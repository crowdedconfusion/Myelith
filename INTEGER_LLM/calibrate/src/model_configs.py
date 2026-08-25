"""
Modell-Konfigurationen fuer verschiedene Qwen2.5-Groessen.
Ermoeglicht einfachen Austausch: 0.5B -> 72B.

WICHTIG (Fund aus Fahrplan-Punkt 12.10/12.14): Exportfaehig sind nur die
Eintraege, deren Felder gegen die *echte* HF-config.json der jeweiligen
Variante geprueft wurden - erkennbar am Feld "verified". Die uebrigen
Eintraege sind Groessenangaben aus den Modellkarten und tragen bewusst
weder "num_kv_heads" noch "tie_word_embeddings"; get_export_model_config()
weist sie deshalb laut zurueck.

Der Grund: runtime/src/loader.rs::ModelDims verlangt beide Felder zwingend
(GQA-Gruppierung, Weight-Tying). Ein Export mit geratenen Werten wuerde
falsche Attention-Berechnung oder ein fehlendes lm_head.weight erzeugen -
und zwar ohne Fehlermeldung, nur mit schlechteren Zahlen. Siehe Hinweis zu
12.10 in INTEGER_LLM/README/Fahrplan-v3.md.

Verifizierte Varianten:
  qwen2.5-0.5b  models/Qwen2.5-0.5B/config.json (lokaler Snapshot)
  qwen2.5-7b    huggingface.co/Qwen/Qwen2.5-7B/raw/main/config.json
                + model.safetensors.index.json (Bias- und lm_head-Tensoren),
                Revision d149729398750b98c0af14eb82c78cfe92750796,
                Lizenz apache-2.0 (Whitepaper Kap. 10.1 / ETHICS G7)

Nur Basis-Varianten, keine Instruct-Varianten: Referenzmodell des Projekts
ist die Basis-Reihe (Scope-Entscheidung 12.15).
"""

MODEL_CONFIGS = {
    "qwen2.5-0.5b": {
        "family": "qwen2.5",
        "variant": "0.5b",
        "num_layers": 24,
        "hidden_size": 896,
        "intermediate_size": 4864,
        "num_heads": 14,
        "num_kv_heads": 2,
        "head_dim": 64,
        "vocab_size": 151936,
        "max_context": 2048,
        "tie_word_embeddings": True,
        # Qwen2.5 besitzt Biases an q/k/v_proj (verifiziert gegen die echte
        # models/Qwen2.5-0.5B/config.json, Feld "attention_bias"); die
        # Runtime verlangt bei true die zugehoerigen Bias-Tensoren im
        # Artefakt (ausserplanmaessiger Patch v0.12.19).
        "attention_bias": True,
        # Qwen2.5 kennt kein QK-Norm; erst Qwen3 normiert Q und K je Kopf
        # vor RoPE. Ausdrueckliches False statt Weglassen, aus demselben
        # Grund wie bei attention_bias: lautes Scheitern statt stiller
        # Abweichung vom Referenzmodell.
        "qk_norm": False,
        # Dichtes Modell, kein Expertengemisch.
        "num_experts": 0,
        "verified": "models/Qwen2.5-0.5B/config.json",
        "hf_model_id": "Qwen/Qwen2.5-0.5B",
    },
    # Verifiziert gegen Qwen/Qwen2.5-7B (Basis), Revision
    # d149729398750b98c0af14eb82c78cfe92750796. Drei Unterschiede zur
    # 0.5B-Variante, die den Exportpfad tatsaechlich beruehren:
    #   num_kv_heads   2 -> 4    (GQA-Gruppierung: 28 Query- auf 4 KV-Heads)
    #   tie_word_embeddings True -> False  (eigenstaendiges lm_head.weight;
    #                            die Weight-Tying-Ausnahme aus v0.12.25
    #                            entfaellt hier, der LM-Head ist ohnehin
    #                            ein eigener Tensor)
    #   head_dim       64 -> 128 (RoPE-LUTs werden [max_context, 64] statt
    #                            [max_context, 32] - LUT-Groesse verdoppelt)
    # attention_bias True gilt weiter: die index.json der Variante fuehrt
    # q_proj.bias/k_proj.bias/v_proj.bias je Layer.
    "qwen2.5-7b": {
        "family": "qwen2.5",
        "variant": "7b",
        "num_layers": 28,
        "hidden_size": 3584,
        "intermediate_size": 18944,
        "num_heads": 28,
        "num_kv_heads": 4,
        "head_dim": 128,
        "vocab_size": 152064,
        # Bewusst 2048 wie bei 0.5B, nicht die 131072 der Modellkarte: der
        # max_context bestimmt die Zeilenzahl der RoPE-LUTs und damit die
        # Artefaktgroesse. 2048 haelt die Messung mit dem 0.5B-Lauf
        # vergleichbar; eine Erhoehung ist eine eigene Entscheidung.
        "max_context": 2048,
        "tie_word_embeddings": False,
        "attention_bias": True,
        "qk_norm": False,
        # Dichtes Modell, kein Expertengemisch.
        "num_experts": 0,
        "verified": "huggingface.co/Qwen/Qwen2.5-7B@d1497293",
        "hf_model_id": "Qwen/Qwen2.5-7B",
    },
    # Verifiziert gegen den lokalen Snapshot models/Qwen3-4B/config.json,
    # Revision 1cfa9a7208912126459214e8b04321603b3df60c, Lizenz Apache-2.0
    # (LICENSE liegt neben den Gewichten, ETHICS G7).
    #
    # **Drei Unterschiede zu Qwen2.5, und nur einer davon war bekannt:**
    #
    #   qk_norm         False -> True   Q und K werden je Kopf ueber
    #                                   head_dim RMS-normiert, VOR RoPE.
    #                                   Der einzige Unterschied, den die
    #                                   Modellagnostik-Analyse benannt
    #                                   hatte.
    #
    #   attention_bias  True -> False   Qwen3 hat ueberhaupt keine
    #                                   Bias-Tensoren; die index.json der
    #                                   Variante fuehrt keinen einzigen.
    #
    #   head_dim        entkoppelt      2560 / 32 = 80, head_dim ist aber
    #                                   128. Bei 0,5B (896 = 14x64) und 7B
    #                                   (3584 = 28x128) fielen beide Zahlen
    #                                   zusammen, und der Rust-Loader hatte
    #                                   die Gleichheit als harte Bedingung
    #                                   eingebaut. Siehe Fund 59.
    #
    # tie_word_embeddings bleibt True wie bei 0,5B: kein eigenes
    # lm_head.weight im Export.
    "qwen3-4b": {
        "family": "qwen3",
        "variant": "4b",
        "num_layers": 36,
        "hidden_size": 2560,
        "intermediate_size": 9728,
        "num_heads": 32,
        "num_kv_heads": 8,
        "head_dim": 128,
        "vocab_size": 151936,
        # Wie bei 0,5B und 7B bewusst 2048 statt der 40960 der Modellkarte:
        # max_context bestimmt die Zeilenzahl der RoPE-LUTs und damit die
        # Artefaktgroesse, und 2048 haelt die Messung vergleichbar.
        "max_context": 2048,
        "tie_word_embeddings": True,
        "attention_bias": False,
        "qk_norm": True,
        # Dichtes Modell, kein Expertengemisch.
        "num_experts": 0,
        "verified": "models/Qwen3-4B/config.json@1cfa9a72",
        "hf_model_id": "Qwen/Qwen3-4B",
    },
    # Verifiziert gegen den lokalen Snapshot
    # models/Qwen3-30B-A3B/config.json, Revision
    # ad44e777bcd18fa416d9da3bd8f70d33ebb85d39, Lizenz Apache-2.0.
    # Tensornamen zusaetzlich gegen die echte
    # model.safetensors.index.json gehalten: je Layer mlp.gate.weight
    # plus 384 Experten-Tensoren (128 x gate/up/down), 18 867 insgesamt.
    #
    # **Das erste Mixture-of-Experts-Modell des Projekts.** Alle 48 Layer
    # sind MoE (mlp_only_layers ist leer, decoder_sparse_step 1).
    #
    # tie_word_embeddings ist hier False, anders als bei Qwen3-4B: Das
    # Modell traegt ein eigenes lm_head.weight.
    #
    # intermediate_size 6144 steht in der config, wird aber bei
    # decoder_sparse_step 1 von keiner Layer benutzt. Es bleibt
    # eingetragen, weil es in der echten config steht; die Rechenarbeit
    # bemisst sich an moe_intermediate_size mal num_experts_per_tok.
    "qwen3-30b-a3b": {
        "family": "qwen3-moe",
        "variant": "30b-a3b",
        "num_layers": 48,
        "hidden_size": 2048,
        "intermediate_size": 6144,
        "num_heads": 32,
        "num_kv_heads": 4,
        "head_dim": 128,
        "vocab_size": 151936,
        "max_context": 2048,
        "tie_word_embeddings": False,
        "attention_bias": False,
        "qk_norm": True,
        "num_experts": 128,
        "num_experts_per_tok": 8,
        "moe_intermediate_size": 768,
        "norm_topk_prob": True,
        "mlp_only_layers": [],
        "verified": "models/Qwen3-30B-A3B/config.json@ad44e777",
        "hf_model_id": "Qwen/Qwen3-30B-A3B",
    },
    "qwen2.5-1.5b-instruct": {
        "num_layers": 28,
        "hidden_size": 1536,
        "intermediate_size": 8960,
        "num_heads": 12,
        "head_dim": 128,
        "vocab_size": 151936,
        "max_context": 32768,
    },
    "qwen2.5-3b-instruct": {
        "num_layers": 36,
        "hidden_size": 2048,
        "intermediate_size": 11008,
        "num_heads": 16,
        "head_dim": 128,
        "vocab_size": 151936,
        "max_context": 32768,
    },
    "qwen2.5-7b-instruct": {
        "num_layers": 28,
        "hidden_size": 3584,
        "intermediate_size": 18944,
        "num_heads": 28,
        "head_dim": 128,
        "vocab_size": 152064,
        "max_context": 32768,
    },
    "qwen2.5-14b-instruct": {
        "num_layers": 48,
        "hidden_size": 5120,
        "intermediate_size": 13824,
        "num_heads": 40,
        "head_dim": 128,
        "vocab_size": 152064,
        "max_context": 32768,
    },
    "qwen2.5-32b-instruct": {
        "num_layers": 64,
        "hidden_size": 5120,
        "intermediate_size": 27648,
        "num_heads": 40,
        "head_dim": 128,
        "vocab_size": 152064,
        "max_context": 32768,
    },
    "qwen2.5-72b-instruct": {
        "num_layers": 80,
        "hidden_size": 8192,
        "intermediate_size": 29568,
        "num_heads": 64,
        "head_dim": 128,
        "vocab_size": 152064,
        "max_context": 32768,
    },
}


def get_model_config(name: str) -> dict:
    if name not in MODEL_CONFIGS:
        raise ValueError(f"Unbekanntes Modell: {name}. Verfuegbar: {list(MODEL_CONFIGS.keys())}")
    return MODEL_CONFIGS[name]


# Felder, die runtime/src/loader.rs::ModelDims fuer einen erfolgreichen Export
# zwingend braucht (siehe Modul-Docstring: nur fuer 0.5B verifiziert).
_REQUIRED_EXPORT_FIELDS = (
    "family", "variant", "num_layers", "hidden_size", "intermediate_size",
    "num_heads", "num_kv_heads", "head_dim", "vocab_size", "max_context",
    "tie_word_embeddings", "attention_bias", "qk_norm", "num_experts",
    "verified", "hf_model_id",
)

# Felder, die zusaetzlich Pflicht werden, sobald "num_experts" > 0 ist.
# Sie stehen nicht in _REQUIRED_EXPORT_FIELDS, weil ein dichtes Modell
# sie nicht hat und ein leeres Feld dort nichts aussagt; als bedingte
# Pflicht sagen sie etwas.
_REQUIRED_MOE_FIELDS = (
    "num_experts_per_tok", "moe_intermediate_size", "norm_topk_prob",
    "mlp_only_layers",
)


def get_export_model_config(name: str) -> dict:
    """
    Wie get_model_config(), aber mit harter Pruefung, dass alle fuer
    model_config.json noetigen Felder vorhanden sind - schlaegt laut und
    fruehzeitig fehl statt ein Artefakt zu exportieren, das der Rust-Loader
    ohnehin ablehnen wuerde (oder schlimmer: mit falschen Annahmen laedt).

    "attention_bias" steht mit in der Liste, seit der Loader die Q/K/V-Biases
    verarbeitet (v0.12.19): fehlt das Feld, exportiert der Lauf ein Artefakt,
    das die Runtime beim Laden ablehnt. "verified" erzwingt, dass jemand die
    Werte gegen die echte config.json der Variante gehalten hat - dieses Feld
    ist der Unterschied zwischen einer geprueften und einer abgeschriebenen
    Konfiguration.
    """
    config = get_model_config(name)
    missing = [f for f in _REQUIRED_EXPORT_FIELDS if f not in config]
    if not missing and config.get("num_experts", 0) > 0:
        # Bedingte Pflicht: Ein MoE-Eintrag ohne top_k oder ohne
        # Expertenbreite ist nicht exportfaehig, und der Rust-Loader
        # koennte den Fehler nicht bemerken - er saehe nur eine Null und
        # baute eine Layer ohne Experten.
        missing = [f for f in _REQUIRED_MOE_FIELDS if f not in config]
    if missing:
        raise ValueError(
            f"Modell '{name}' fehlen fuer den Export noetige, verifizierte Felder: {missing}. "
            "Gegen die echte HF-config.json dieser Variante pruefen und in MODEL_CONFIGS "
            "ergaenzen, bevor sie exportiert wird (siehe Modul-Docstring). "
            f"Exportfaehig sind derzeit: {export_ready()}."
        )
    return config


def export_ready() -> list:
    """Namen aller Varianten, die den Export-Gate von get_export_model_config() bestehen."""
    return [
        name for name, cfg in MODEL_CONFIGS.items()
        if all(f in cfg for f in _REQUIRED_EXPORT_FIELDS)
    ]


# Felder, die als model_config.json ins Artefakt geschrieben werden. "verified"
# und "hf_model_id" sind Herkunftsnachweise fuer den Menschen, keine
# Modellparameter - sie gehoeren nicht in die vom Loader gepruefte Struktur.
_ARTIFACT_EXCLUDED_FIELDS = ("verified", "hf_model_id")


def artifact_model_config(name: str) -> dict:
    """Die Felder aus get_export_model_config(), die in model_config.json gehoeren."""
    config = get_export_model_config(name)
    return {k: v for k, v in config.items() if k not in _ARTIFACT_EXCLUDED_FIELDS}


def suggest_sharding(num_layers: int, num_nodes: int) -> list:
    """
    Schlaegt eine gleichmaessige Layer-Aufteilung vor.
    Returns: Liste von (start, end) Tupeln pro Node.
    """
    if num_layers % num_nodes != 0:
        raise ValueError(f"{num_layers} Layer lassen sich nicht gleichmaessig auf {num_nodes} Nodes aufteilen.")
    
    layers_per_node = num_layers // num_nodes
    shards = []
    for i in range(num_nodes):
        start = i * layers_per_node
        end = start + layers_per_node
        shards.append((start, end))
    return shards


def print_sharding_plan(model_name: str, num_nodes: int):
    config = get_model_config(model_name)
    shards = suggest_sharding(config["num_layers"], num_nodes)
    
    print(f"\nModell: {model_name}")
    print(f"  Layer: {config['num_layers']}, Hidden: {config['hidden_size']}, Heads: {config['num_heads']}")
    print(f"  Shard-Plan fuer {num_nodes} Nodes:")
    for i, (start, end) in enumerate(shards):
        num_layers = end - start
        has_emb = "[EMB]" if i == 0 else ""
        has_head = "[HEAD+SAMPLING]" if i == num_nodes - 1 else ""
        print(f"    Node {i}: Layer {start:2d}-{end:2d} ({num_layers} Layer) {has_emb} {has_head}")


if __name__ == "__main__":
    # Beispiel: Sharding-Plaene fuer verschiedene Konfigurationen
    for model in ["qwen2.5-0.5b", "qwen2.5-7b-instruct", "qwen2.5-72b-instruct"]:
        for nodes in [2, 4, 8]:
            try:
                print_sharding_plan(model, nodes)
            except ValueError as e:
                print(f"\n{model} auf {nodes} Nodes: {e}")
