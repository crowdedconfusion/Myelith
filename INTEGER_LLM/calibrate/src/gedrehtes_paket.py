"""
Ein ternaeres, gedrehtes Paket (MLX, 2 Bit je Gewicht, Gruppen von 128,
Hadamard-Drehung in die Gewichte gefaltet) als transformers-Modell fuer die
Kalibrierung.

⚑ **Wozu.** Die Kalibrierung braucht ein Modell, das in Gleitkomma
vorwaerts rechnet, damit die Aktivierungsstatistik entsteht. Ein 27B in
BF16 sind 54 GB gegen 24 GiB Arbeitsspeicher, und ein dichtes Modell liest
je Durchgang alle Gewichte: Auslagern hiesse Terabytes je Kalibrierung.
Das Paket selbst ist 8,6 GB. Hier bleibt es gepackt im Speicher, und jede
gedrehte Linearschicht entpackt ihre Gewichte erst im Vorwaertslauf, eine
Matrix zur Zeit.

⚑ **Das Format, wie es hier gelesen wird** (nachvollzogen aus dem Paket,
nichts uebernommen):

- Gewicht: 2-Bit-Code `q` je Gewicht, 16 je `uint32`, das Gewicht `j` in den
  Bits `2*(j % 16)` des Worts `j // 16`. Je Gruppe von 128 eine FP16-Skala
  `s` und ein Achsenabschnitt `b = -s`, also `w = s*q + b = s*(q - 1)`.
- Drehung einer Eingabe: erst mit den Vorzeichen der Eingangsbreite
  multiplizieren, dann je Block von 1024 mit der normierten
  Sylvester-Hadamard-Matrix (`H/32`). Die gepackte Matrix erwartet die
  gedrehte Eingabe.
- Einbettung: umgekehrt gedreht gespeichert, also Zeile nachschlagen, erst
  Hadamard, dann Vorzeichen.
- Faltung der linearen Aufmerksamkeit: MLX-Layout `[Ausgang, Kern, Eingang]`,
  transformers erwartet `[Ausgang, Eingang, Kern]`.
- Normen: der Versatz +1 der nullzentrierten Qwen3.5-Normen ist im Paket
  schon eingerechnet (gemessen: Mittel um 1). transformers rechnet mit
  `1 + w`, also wird hier 1 abgezogen. Die gegatete Norm der linearen
  Aufmerksamkeit rechnet mit `w` und bleibt.

⚠️ **Gleitkomma ist hier erlaubt**: Das ist das Kalibrierwerkzeug, nicht
der Rechenpfad. Gerechnet wird in float32, damit die Referenz nicht an der
BF16-Rundung haengt.
"""
import json
import math
from pathlib import Path

import torch
from torch import nn

BLOCK = 1024


def hadamard(n: int = BLOCK) -> torch.Tensor:
    """Die normierte Sylvester-Hadamard-Matrix der Groesse `n` (symmetrisch)."""
    h = torch.ones(1, 1)
    while h.shape[0] < n:
        h = torch.cat([torch.cat([h, h], 1), torch.cat([h, -h], 1)], 0)
    return h / math.sqrt(n)


_H = None


def _h() -> torch.Tensor:
    global _H
    if _H is None:
        _H = hadamard()
    return _H


def drehen(x: torch.Tensor, vorzeichen: torch.Tensor) -> torch.Tensor:
    """Eingabe einer gedrehten Linearschicht: Vorzeichen, dann Hadamard je Block."""
    form = x.shape
    y = (x.float() * vorzeichen).reshape(-1, form[-1] // BLOCK, BLOCK) @ _h()
    return y.reshape(form)


def rueckdrehen(x: torch.Tensor, vorzeichen: torch.Tensor) -> torch.Tensor:
    """Die Umkehrung fuer die Einbettung: Hadamard je Block, dann Vorzeichen."""
    form = x.shape
    y = (x.float().reshape(-1, form[-1] // BLOCK, BLOCK) @ _h()).reshape(form)
    return y * vorzeichen


def codes_entpacken(woerter: torch.Tensor) -> torch.Tensor:
    """`uint32 [z, n/16]` zu Codes `uint8 [z, n]` (0 bis 3)."""
    w = woerter.to(torch.int64)
    verschiebung = torch.arange(0, 32, 2, dtype=torch.int64)
    codes = (w.unsqueeze(-1) >> verschiebung) & 3
    return codes.reshape(woerter.shape[0], -1).to(torch.uint8)


def entpacken(woerter, skalen, zeilen=None) -> torch.Tensor:
    """Die Gewichte `s * (q - 1)` in float32, auf Wunsch nur ausgewaehlte Zeilen."""
    if zeilen is not None:
        woerter, skalen = woerter[zeilen], skalen[zeilen]
    q = codes_entpacken(woerter).float() - 1.0
    s = skalen.float().repeat_interleave(128, dim=-1)
    return q * s


class Drehung(nn.Module):
    """Gibt die gedrehte Eingabe zurueck; ein eigenes Modul, damit die
    Statistik sie unter `<projektion>.drehung` messen kann."""

    def __init__(self, vorzeichen: torch.Tensor):
        super().__init__()
        self.register_buffer("vorzeichen", vorzeichen, persistent=False)

    def forward(self, x):
        return drehen(x, self.vorzeichen)


class GepackteLinear(nn.Module):
    """Eine gedrehte, ternaer gepackte Linearschicht; entpackt je Aufruf.

    ⚑ **In Zeilenbloecken**, damit auch der Kopf (248 320 x 5 120) nie ganz
    entpackt im Speicher liegt.
    """

    ZEILEN_JE_BLOCK = 8192

    def __init__(self, woerter, skalen, vorzeichen):
        super().__init__()
        self.register_buffer("woerter", woerter, persistent=False)
        self.register_buffer("skalen", skalen, persistent=False)
        self.out_features = woerter.shape[0]
        self.in_features = woerter.shape[1] * 16
        self.drehung = Drehung(vorzeichen)

    def forward(self, x):
        xd = self.drehung(x).float()
        teile = []
        for a in range(0, self.out_features, self.ZEILEN_JE_BLOCK):
            e = min(a + self.ZEILEN_JE_BLOCK, self.out_features)
            w = entpacken(self.woerter[a:e], self.skalen[a:e])
            teile.append(xd @ w.T)
        return torch.cat(teile, dim=-1)


def _vorzeichen(hadamard_json: dict) -> dict:
    breiten = hadamard_json["prism.hadamard.sign_widths"]
    werte = hadamard_json["prism.hadamard.sign_values"]
    aus, a = {}, 0
    for b in breiten:
        aus[b] = torch.tensor(werte[a:a + b], dtype=torch.float32)
        a += b
    if a != len(werte):
        raise ValueError("hadamard.json: Vorzeichen passen nicht zu den Breiten")
    return aus


# Nullzentrierte Normen (transformers rechnet `1 + w`); die gegatete Norm
# der linearen Aufmerksamkeit (`linear_attn.norm`) gehoert nicht dazu.
_NULLZENTRIERT = ("input_layernorm.weight", "post_attention_layernorm.weight",
                  "self_attn.q_norm.weight", "self_attn.k_norm.weight")


def paket_laden(ordner, praefix="language_model."):
    """Laedt das Paket als `Qwen3_5ForCausalLM` in float32, die gedrehten
    Linearschichten gepackt. Gibt `(modell, tokenizer)` zurueck."""
    from safetensors import safe_open
    from transformers import AutoTokenizer
    from transformers.models.qwen3_5 import Qwen3_5ForCausalLM, Qwen3_5TextConfig
    from transformers.models.qwen3_5.modeling_qwen3_5 import Qwen3_5TextRotaryEmbedding

    ordner = Path(ordner)
    konfig = json.loads((ordner / "config.json").read_text())
    dreh = json.loads((ordner / "hadamard.json").read_text())
    if dreh.get("prism.hadamard.block_size") != BLOCK:
        raise ValueError("unerwartete Blockgroesse der Drehung")
    if dreh.get("prism.hadamard.transform") != "normalized-sylvester-walsh-hadamard":
        raise ValueError("unerwartete Drehung")
    vorzeichen = _vorzeichen(dreh)
    gedreht = set(dreh["prism.hadamard.weight_names"])
    umgekehrt = set(dreh["prism.hadamard.inverse_weight_names"])

    cfg = Qwen3_5TextConfig(**konfig["text_config"])
    cfg.torch_dtype = torch.float32
    with torch.device("meta"):
        modell = Qwen3_5ForCausalLM(cfg)

    def setzen(pfad: str, wert: torch.Tensor):
        teile = pfad.split(".")
        ziel = modell
        for t in teile[:-1]:
            ziel = getattr(ziel, t)
        setattr(ziel, teile[-1], nn.Parameter(wert, requires_grad=False))

    gesehen = set()
    with safe_open(str(ordner / "model.safetensors"), "pt") as f:
        namen = [n for n in f.keys() if n.startswith(praefix)]
        staemme = sorted({n[: -len(".weight")] for n in namen if n.endswith(".weight")
                          and (n[: -len(".weight")] + ".scales") in namen})
        for stamm in staemme:
            hf = stamm[len(praefix):]
            woerter = f.get_tensor(stamm + ".weight")
            skalen = f.get_tensor(stamm + ".scales")
            achsen = f.get_tensor(stamm + ".biases")
            if not torch.equal(achsen, -skalen):
                raise ValueError(f"{stamm}: Achsenabschnitt ist nicht -Skala")
            vz = f.get_tensor(stamm + ".signs").float()
            gesehen |= {stamm + s for s in (".weight", ".scales", ".biases", ".signs")}
            if stamm + ".weight" in umgekehrt:
                # Die Einbettung: einmal ganz entpacken und zurueckdrehen,
                # in Bloecken, damit float32 nicht auf einmal entsteht.
                zeilen = woerter.shape[0]
                plain = torch.empty(zeilen, woerter.shape[1] * 16, dtype=torch.float32)
                for a in range(0, zeilen, 8192):
                    e = min(a + 8192, zeilen)
                    plain[a:e] = rueckdrehen(entpacken(woerter[a:e], skalen[a:e]), vz)
                setzen(hf + ".weight", plain)
            elif stamm + ".weight" in gedreht:
                teile = hf.split(".")
                eltern = modell
                for t in teile[:-1]:
                    eltern = getattr(eltern, t)
                setattr(eltern, teile[-1], GepackteLinear(woerter, skalen, vz))
            else:
                raise ValueError(f"{stamm}: gepackt, aber weder gedreht noch umgekehrt")
        for n in namen:
            if n in gesehen:
                continue
            hf = n[len(praefix):]
            t = f.get_tensor(n).float()
            if hf.endswith(_NULLZENTRIERT) or hf == "model.norm.weight":
                t = t - 1.0
            if hf.endswith("linear_attn.conv1d.weight") and t.dim() == 3:
                # MLX legt Faltungen als [Ausgang, Kern, Eingang] ab,
                # transformers als [Ausgang, Eingang, Kern].
                t = t.permute(0, 2, 1).contiguous()
            setzen(hf, t)

    # Puffer, die auf dem meta-Geraet entstanden sind (Drehtabellen der
    # Positionen), neu auf der CPU anlegen.
    modell.model.rotary_emb = Qwen3_5TextRotaryEmbedding(cfg)
    fehlend = [n for n, p in modell.named_parameters() if p.device.type == "meta"]
    fehlend += [n for n, b in modell.named_buffers() if b.device.type == "meta"]
    if fehlend:
        raise RuntimeError(f"{len(fehlend)} Tensoren ohne Daten, z. B. {fehlend[:5]}")
    modell.eval()
    tokenizer = AutoTokenizer.from_pretrained(str(ordner))
    return modell, tokenizer
