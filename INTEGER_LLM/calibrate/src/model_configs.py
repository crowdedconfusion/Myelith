"""
Modell-Konfigurationen fuer verschiedene Qwen2.5-Groessen.
Ermoeglicht einfachen Austausch: 0.5B -> 72B.

WICHTIG (Fund aus Fahrplan-Punkt 12.10/12.14): Nur der 0.5B-Eintrag traegt
"num_key_value_heads" und "tie_word_embeddings" - verifiziert gegen die
tatsaechliche models/Qwen2.5-0.5B/config.json. Fuer die uebrigen Varianten
sind diese Werte NICHT geraten worden, sondern fehlen bewusst, bis sie gegen
die jeweils echte HF-config.json der Variante geprueft sind. runtime/src/
loader.rs::ModelDims verlangt beide Felder zwingend (GQA-Gruppierung,
Weight-Tying) - ein Export ohne verifizierte Werte wuerde falsche Attention-
Berechnung oder ein fehlendes lm_head.weight erzeugen, siehe Hinweis zu 12.10
in INTEGER_LLM/README/Fahrplan-v3.md.
"""

MODEL_CONFIGS = {
    "qwen2.5-0.5b-instruct": {
        "family": "qwen2.5",
        "variant": "0.5b-instruct",
        "num_layers": 24,
        "hidden_size": 896,
        "intermediate_size": 4864,
        "num_heads": 14,
        "num_kv_heads": 2,
        "head_dim": 64,
        "vocab_size": 151936,
        "max_context": 2048,
        "tie_word_embeddings": True,
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
    "tie_word_embeddings",
)


def get_export_model_config(name: str) -> dict:
    """
    Wie get_model_config(), aber mit harter Pruefung, dass alle fuer
    model_config.json noetigen Felder vorhanden sind - schlaegt laut und
    fruehzeitig fehl statt ein Artefakt zu exportieren, das der Rust-Loader
    ohnehin ablehnen wuerde (oder schlimmer: mit falschen Annahmen laedt).
    """
    config = get_model_config(name)
    missing = [f for f in _REQUIRED_EXPORT_FIELDS if f not in config]
    if missing:
        raise ValueError(
            f"Modell '{name}' fehlen fuer den Export noetige, verifizierte Felder: {missing}. "
            "Gegen die echte HF-config.json dieser Variante pruefen und in MODEL_CONFIGS "
            "ergaenzen, bevor sie exportiert wird (siehe Modul-Docstring)."
        )
    return config


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
    for model in ["qwen2.5-0.5b-instruct", "qwen2.5-7b-instruct", "qwen2.5-72b-instruct"]:
        for nodes in [2, 4, 8]:
            try:
                print_sharding_plan(model, nodes)
            except ValueError as e:
                print(f"\n{model} auf {nodes} Nodes: {e}")
