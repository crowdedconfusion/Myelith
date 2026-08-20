"""Der exp-LUT-Eingang: Attention-Scores auf 1/16 gerastert.

Bisher gemessen wurde die Softmax-AUSGABE (prob_frac_bits=8, +-0 %).
Der LUT-EINGANG ist eine andere Groesse: exp_input_frac_bits=4 rastert
die Scores selbst auf 1/16, BEVOR exponenziert wird. Ein Rasterfehler
von 1/32 im Exponenten wird durch exp() multiplikativ verstaerkt.

Der SDPA-Ersatz wird wie zuvor zuerst gegen das Original validiert.
"""
import json, math, sys, torch
from pathlib import Path
REPO = Path("/Users/entity/Desktop/Code/Code/Artificial/Myelith/Repository/INTEGER_LLM")
sys.path.insert(0, str(REPO/"calibrate")); sys.path.insert(0, str(REPO/"eval"))
from src.loader import load_reference_model
from wikitext_common import MODEL_DIR, MODEL_NAME, select_sequences
MAXF = 20
LINEAR = ("q_proj","k_proj","v_proj","o_proj","gate_proj","up_proj","down_proj")
WEITERE = ("embed_tokens","lm_head","input_layernorm","post_attention_layernorm","norm")
ORIG = torch.nn.functional.scaled_dot_product_attention
Z = {"score_bits": None, "aufrufe": 0}

def q8(W):
    a = W.abs().clamp(min=1e-9) if W.dim()==1 else W.abs().amax(dim=1,keepdim=True).clamp(min=1e-9)
    sc = torch.pow(2.0, torch.floor(torch.log2(127.0/a)).clamp(0,MAXF))
    return torch.clamp(torch.round(W*sc),-128,127)/sc

def ersatz(q,k,v,attn_mask=None,dropout_p=0.0,is_causal=False,scale=None,**kw):
    Z["aufrufe"] += 1
    d = q.shape[-1]; sc = scale if scale is not None else 1.0/math.sqrt(d)
    qs,ks,vs = q.float(),k.float(),v.float()
    if kw.get("enable_gqa") and ks.shape[1]!=qs.shape[1]:
        r = qs.shape[1]//ks.shape[1]
        ks = ks.repeat_interleave(r,1); vs = vs.repeat_interleave(r,1)
    s = (qs @ ks.transpose(-2,-1))*sc
    if is_causal:
        L,S = qs.shape[-2],ks.shape[-2]
        s = s.masked_fill(~torch.ones(L,S,dtype=torch.bool,device=q.device).tril(diagonal=S-L), float("-inf"))
    if attn_mask is not None:
        s = s.masked_fill(~attn_mask, float("-inf")) if attn_mask.dtype==torch.bool else s+attn_mask.float()
    if Z["score_bits"] is not None:
        # Rasterung relativ zum Zeilenmaximum - genau das tut unser
        # Kernel: er bildet (m - z) und indiziert damit die exp-LUT.
        m = s.amax(dim=-1, keepdim=True)
        diff = (m - s).clamp(min=0)
        g = float(1 << Z["score_bits"])
        s = m - torch.round(diff*g)/g
    p = torch.softmax(s, dim=-1)
    return (p @ vs).to(q.dtype)

def ppl(model, seqs):
    s=n=0
    with torch.no_grad():
        for ids in seqs:
            lg = model(input_ids=torch.tensor([ids], device=model.device)).logits[0].float()
            lp = torch.log_softmax(lg[:-1],dim=-1)
            t = lp.gather(1, torch.tensor(ids[1:],device=lp.device).unsqueeze(1)).squeeze(1)
            s+=t.sum().item(); n+=t.numel()
    return math.exp(-s/n)

mess = select_sequences(4,128,verbose=False)
model,_ = load_reference_model(MODEL_DIR)
ref = ppl(model, mess)
torch.nn.functional.scaled_dot_product_attention = ersatz
Z["score_bits"]=None; Z["aufrufe"]=0
nach = ppl(model, mess)
print(f"VALIDIERUNG original {ref:.4f} | Ersatz {nach:.4f} | {Z['aufrufe']} Aufrufe", flush=True)
if Z["aufrufe"]==0 or abs(nach-ref)/ref > 0.005:
    torch.nn.functional.scaled_dot_product_attention = ORIG
    raise SystemExit("ABBRUCH: Ersatz reproduziert sdpa nicht.")
print("-> validiert\n", flush=True)

with torch.no_grad():
    for nme,m in model.named_modules():
        if hasattr(m,"weight") and m.weight is not None and (nme.endswith(LINEAR) or any(k in nme for k in WEITERE)):
            m.weight.data = q8(m.weight.data.float()).to(m.weight.dtype)
Z["score_bits"]=None
grund = ppl(model, mess)
print(f"Grundlage (W8, Scores exakt)   : {grund:.2f}", flush=True)
for b in (4, 6, 8, 12):
    Z["score_bits"]=b
    w = ppl(model, mess)
    marke = "  <- theta_v heute" if b==4 else ""
    print(f"  + Score-Raster 1/{1<<b:<4}      : {w:.2f}   ({100*(w/grund-1):+.2f} %){marke}", flush=True)
torch.nn.functional.scaled_dot_product_attention = ORIG
