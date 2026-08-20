"""Wieviel Ebenenfehler erzeugt das SCHEMA allein?

Unser Pfad zeigt ~9 % relativen L2 je Ebene gegen float. Nie gemessen
wurde, wieviel davon schon die Gewichtsquantisierung erzeugt. Ist es
ebenfalls ~8 %, gibt es keinen Widerspruch zwischen "jede Operation
exakt" und "9 % Ebenenfehler" - dann ist der Ebenenfehler schlicht kein
Mass fuer den Perplexitaetsabstand.
"""
import pickle, sys, torch, numpy as np
from pathlib import Path
REPO = Path("/Users/entity/Desktop/Code/Code/Artificial/Myelith/Repository/INTEGER_LLM")
sys.path.insert(0, str(REPO/"calibrate")); sys.path.insert(0, str(REPO/"eval"))
from src.loader import load_reference_model
from wikitext_common import MODEL_DIR
MAXF, INT16 = 20, 32767
LIN = ("q_proj","k_proj","v_proj","o_proj","gate_proj","up_proj","down_proj")
WEIT = ("embed_tokens","lm_head","input_layernorm","post_attention_layernorm","norm")
def q8(W):
    a = W.abs().clamp(min=1e-9) if W.dim()==1 else W.abs().amax(dim=1,keepdim=True).clamp(min=1e-9)
    sc = torch.pow(2.0, torch.floor(torch.log2(127.0/a)).clamp(0,MAXF))
    return torch.clamp(torch.round(W*sc),-128,127)/sc
toks = [int(x) for x in open("/tmp/tok32.txt").read().split()]

def hidden(quant):
    m,_ = load_reference_model(MODEL_DIR)
    if quant:
        with torch.no_grad():
            for n,mod in m.named_modules():
                if hasattr(mod,"weight") and mod.weight is not None and (n.endswith(LIN) or any(k in n for k in WEIT)):
                    mod.weight.data = q8(mod.weight.data.float()).to(mod.weight.dtype)
    with torch.no_grad():
        o = m(input_ids=torch.tensor([toks]), output_hidden_states=True)
    r = [h[0,-1].float().cpu().numpy().astype(np.float64) for h in o.hidden_states]
    del m
    return r

f = hidden(False)
s = hidden(True)
print(f"{'Ebene':>6} {'Schema vs float':>16}")
for i in (1, 4, 11, 18, 24):
    e = 100*np.linalg.norm(s[i]-f[i])/np.linalg.norm(f[i])
    print(f"{i-1:6d} {e:15.2f}%")
