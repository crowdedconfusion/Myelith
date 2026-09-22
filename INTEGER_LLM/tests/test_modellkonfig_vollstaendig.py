#!/usr/bin/env python3
"""**Stimmt der Modelleintrag mit dem echten `config.json` ueberein?**

## ⛔️ Wozu diese Probe da ist

Am 2026-09-21 haben zwei Angaben Stunden gekostet, und beide hatten
dieselbe Form:

| Angabe | im `config.json` | was ich einsetzte | Folge |
|---|---|---|---|
| `norm_topk_prob` | fehlt | `False` erfunden | Expertengewichte nicht normiert |
| `rope_theta` | `None` oben, **1e7 in `rope_scaling`** | θ_v-Konstante 1e6 | alle Drehwinkel falsch, Kauderwelsch |

📌 **Ein fehlendes Feld ist keine Erlaubnis, einen Vorgabewert zu
erfinden**, und ein Feld, das oben `None` ist, kann weiter unten einen
Wert haben. Beide Male habe ich an einer Stelle nachgesehen und den Rest
angenommen.

⚑ **Diese Probe sucht an allen Stellen.** Sie vergleicht jedes Feld des
Modelleintrags mit dem echten `config.json`, schaut dabei auch in
`text_config` und `rope_scaling`, und meldet zusaetzlich, welche Felder
des Config **niemand gelesen hat**.

⚠️ **Der zweite Teil ist der wichtigere.** Eine Abweichung faellt
irgendwann auf; ein Feld, das niemand liest, nie.

Aufruf:
    python3 INTEGER_LLM/tests/test_modellkonfig_vollstaendig.py [modell]
"""
import json
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
LLM = HIER.parent
REPO = LLM.parent
sys.path.insert(0, str(LLM / "calibrate"))

# Felder des Config, die den Export bewusst nichts angehen.
NICHT_GEBRAUCHT = {
    "architectures", "auto_map", "bos_token_id", "eos_token_id",
    "pad_token_id", "torch_dtype", "dtype", "transformers_version",
    "use_cache", "attention_dropout", "initializer_range",
    "model_type", "_name_or_path", "router_aux_loss_coef",
    "output_router_logits", "tie_word_embeddings",
    # Der Sichtturm und der MTP-Kopf sind ausgeschlossen (Fund 416).
    "vision_config", "image_token_id", "video_token_id",
    "vision_start_token_id", "vision_end_token_id",
    # ⚑ Nachgesehen am 2026-09-21 und fuer gleichgueltig befunden:
    # `mamba_ssm_dtype` ist eine Gleitkomma-Angabe der Vorlage (dieses
    # Projekt rechnet ganzzahlig), `mtp_use_dedicated_embeddings`
    # betrifft nur den ausgeschlossenen MTP-Kopf, und
    # `rope_parameters` ist derselbe Inhalt wie `rope_scaling`, dessen
    # Felder einzeln geprueft werden.
    "mamba_ssm_dtype", "mtp_use_dedicated_embeddings", "rope_parameters",
    # ⚠️ **Der Config zaehlt die MTP-Ebenen des QUELLMODELLS, der Eintrag
    # die des ARTEFAKTS.** Der Kopf ist ausgeschlossen (Fund 416), also
    # 1 gegen 0, und das ist Absicht statt Abweichung.
    "mtp_num_hidden_layers",
    # ⚑ Nachgesehen am 2026-09-21: Bei allen fuenf Modellen ist
    # `sliding_window` None und `use_sliding_window` False, also gibt es
    # kein Fenster; `max_window_layers` zaehlt dann nichts.
    "sliding_window", "use_sliding_window", "max_window_layers",
    # `decoder_sparse_step: 1` heisst „jede Ebene ist ein Gemisch", und
    # genau das drueckt `num_experts > 0` im Eintrag schon aus.
    "decoder_sparse_step",
    # `rope_scaling` selbst ist der Behaelter; seine Felder werden
    # einzeln geprueft (siehe `flach`).
    "rope_scaling",
}

# Wie ein Feld im Modelleintrag heisst, wenn es anders heisst als im Config.
# **Wo Eintrag und Config bewusst verschieden sind**, mit dem Grund.
#
# ⚑ **Eigene Liste und nicht `NICHT_GEBRAUCHT`.** Ein Feld, das niemand
# liest, und ein Feld, das absichtlich anders lautet, sind zwei
# verschiedene Dinge. Beides in einen Topf zu werfen hiesse, die Absicht
# unsichtbar zu machen.
ABSICHTLICH_ANDERS = {
    # Der Config zaehlt die MTP-Ebenen des QUELLMODELLS (1), der Eintrag
    # die des ARTEFAKTS (0): Der Kopf fuer Mehrfachvorhersage ist beim
    # Export ausgeschlossen (Fund 416).
    "mtp_num_hidden_layers": "MTP-Kopf ist ausgeschlossen (Fund 416)",
}

UEBERSETZUNG = {
    "num_attention_heads": "num_heads",
    "num_key_value_heads": "num_kv_heads",
    "num_hidden_layers": "num_layers",
    "max_position_embeddings": "max_context",
}


def flach(config: dict) -> dict:
    """Alle Felder, auch die aus `text_config` und `rope_scaling`.

    ⚑ **`rope_scaling` wird mit hineingezogen**, denn genau dort lag der
    Wert, den zu uebersehen einen Abend gekostet hat.
    """
    aus = {}
    for k, v in config.items():
        if k == "text_config" and isinstance(v, dict):
            aus.update(flach(v))
        elif k == "rope_scaling" and isinstance(v, dict):
            for rk, rv in v.items():
                aus[rk] = rv
        else:
            aus[k] = v
    return aus


def main() -> int:
    from src.model_configs import get_export_model_config, MODEL_CONFIGS as MODELS

    modell = sys.argv[1] if len(sys.argv) > 1 else None
    namen = [modell] if modell else sorted(MODELS)
    fehler = 0

    for name in namen:
        # ⚑ **Nur exportfaehige Eintraege.** Ein Eintrag ohne die
        #   verifizierten Pflichtfelder ist ein Vermerk und kein Modell;
        #   ihn hier zu pruefen hiesse, eine Absicht gegen einen Config
        #   zu halten.
        try:
            eintrag = get_export_model_config(name)
        except ValueError:
            continue
        quelle = REPO / "MODELS" / "llm" / eintrag["hf_model_id"].split("/")[-1]
        pfad = quelle / "config.json"
        if not pfad.is_file():
            print(f"[konfigprobe] {name}: uebersprungen, {pfad} fehlt")
            continue

        echt = flach(json.loads(pfad.read_text(encoding="utf-8")))
        print(f"\n[konfigprobe] {name} gegen {pfad.relative_to(REPO)}")

        # 1. Jedes Feld des Eintrags gegen den Config.
        abweichend = []
        for ck, wert in echt.items():
            mk = UEBERSETZUNG.get(ck, ck)
            if mk not in eintrag:
                continue
            meiner = eintrag[mk]
            if isinstance(wert, float) or isinstance(meiner, float):
                gleich = abs(float(wert) - float(meiner)) < 1e-9
            else:
                gleich = wert == meiner
            if not gleich:
                abweichend.append((ck, wert, meiner))
        echte_abweichung = []
        for ck, soll, ist in abweichend:
            if ck in ABSICHTLICH_ANDERS:
                print(f"  ⚑ {ck}: {soll!r} gegen {ist!r}, absichtlich "
                      f"({ABSICHTLICH_ANDERS[ck]})")
                continue
            print(f"  ⛔️ {ck}: config sagt {soll!r}, Eintrag sagt {ist!r}")
            echte_abweichung.append(ck)
        fehler += len(echte_abweichung)
        abweichend = echte_abweichung

        # 1b. Abgeleitete Groessen gegen ihre Quelle.
        #
        # ⚑ **Eine Zahl, die aus einer anderen folgt, wird nachgerechnet
        # und nicht geglaubt.** `rotary_dim` habe ich am 2026-09-21 von
        # Hand getippt; es stimmte zufaellig.
        paf = echt.get("partial_rotary_factor")
        if paf is not None and "rotary_dim" in eintrag:
            soll = int(eintrag["head_dim"] * paf)
            if eintrag["rotary_dim"] != soll:
                print(f"  ⛔️ rotary_dim: {eintrag['rotary_dim']}, aber "
                      f"head_dim {eintrag['head_dim']} * {paf} = {soll}")
                fehler += 1

        # 1c. Die Tabellen des Artefakts gegen die Masse.
        #
        # ⛔️ **Der Fund vom 2026-09-22.** Die Drehtabellen des
        # Qwen3.6-35B-A3B hatten 1 310 720 Eintraege, also 40 960 mal 32,
        # waehrend das Artefakt 262 144 Positionen zusagte. Die Zahl stand
        # in `luts.json`, seit das erste Artefakt gebaut wurde, und war
        # mehrfach ausgegeben worden, **ohne sie gegen `max_context` zu
        # halten**.
        #
        # 📌 **Eine Zahl, die man ausgibt, hat man noch nicht geprueft.**
        artefakt = LLM / "artifacts" / name
        luts = artefakt / "luts.json"
        if luts.is_file():
            meta = json.loads(luts.read_text(encoding="utf-8"))
            paare = (eintrag.get("rotary_dim") or eintrag["head_dim"]) // 2
            soll = eintrag["max_context"] * paare
            for tabelle in ("cos", "sin"):
                if tabelle not in meta:
                    print(f"  ⛔️ {tabelle}.lut fehlt im Artefakt")
                    fehler += 1
                    continue
                ist = meta[tabelle]["length"]
                if ist != soll:
                    print(f"  ⛔️ {tabelle}.lut: {ist} Eintraege, aber "
                          f"max_context {eintrag['max_context']} x {paare} "
                          f"Paare = {soll}")
                    fehler += 1
            if all(meta.get(x, {}).get("length") == soll for x in ("cos", "sin")):
                print(f"  ⚑ Drehtabellen passen zu {eintrag['max_context']} "
                      f"Positionen x {paare} Paaren")
        else:
            print(f"  ⚠️ kein Artefakt unter {artefakt.name}, Tabellen "
                  f"nicht geprueft")

        # 2. Was niemand gelesen hat.
        gelesen = {UEBERSETZUNG.get(k, k) for k in echt}
        ungelesen = sorted(
            ck for ck in echt
            if UEBERSETZUNG.get(ck, ck) not in eintrag and ck not in NICHT_GEBRAUCHT
        )
        if ungelesen:
            print(f"  ⚠️ {len(ungelesen)} Felder des Config liest niemand:")
            for ck in ungelesen:
                print(f"       {ck} = {echt[ck]!r}")
            print("     ⚑ Jedes davon ist entweder gleichgueltig oder ein")
            print("       Fehler, der noch nicht aufgefallen ist. Wer es")
            print("       fuer gleichgueltig haelt, traegt es in")
            print("       NICHT_GEBRAUCHT ein und sagt damit, dass er")
            print("       nachgesehen hat.")
            fehler += len(ungelesen)
        elif not abweichend:
            print("  ⚑ alle Felder gelesen und in Uebereinstimmung")

    print()
    if fehler:
        print(f"[konfigprobe] ⛔️ FEHLGESCHLAGEN: {fehler} Punkte")
        return 1
    print("[konfigprobe] ⚑ BESTANDEN")
    return 0


if __name__ == "__main__":
    sys.exit(main())
