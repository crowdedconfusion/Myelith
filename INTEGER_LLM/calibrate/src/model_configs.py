"""
Modell-Konfigurationen fuer verschiedene Qwen3-Groessen.
Ermoeglicht einfachen Austausch: 0,6B -> 30B.

WICHTIG (Fund aus Punkt 12.10/12.14): Exportfaehig sind nur die
Eintraege, deren Felder gegen die *echte* HF-config.json der jeweiligen
Variante geprueft wurden - erkennbar am Feld "verified". Die uebrigen
Eintraege sind Groessenangaben aus den Modellkarten und tragen bewusst
weder "num_kv_heads" noch "tie_word_embeddings"; get_export_model_config()
weist sie deshalb laut zurueck.

Der Grund: runtime/src/loader.rs::ModelDims verlangt beide Felder zwingend
(GQA-Gruppierung, Weight-Tying). Ein Export mit geratenen Werten wuerde
falsche Attention-Berechnung oder ein fehlendes lm_head.weight erzeugen -
und zwar ohne Fehlermeldung, nur mit schlechteren Zahlen.

Verifizierte Varianten, alle Qwen3 und alle gegen den lokalen Snapshot:
  myelith-0.6b     models/Qwen3-0.6B/config.json@c1899de2
  myelith-4b       models/Qwen3-4B/config.json@1cfa9a72
  myelith-30b-a3b  models/Qwen3-30B-A3B/config.json@ad44e777

⚑ Seit dem 2026-09-11 (Festlegung des Projektinhabers) fuehrt das
Projekt ausschliesslich Qwen3. Qwen2.5-0,5B und Qwen2.5-7B sind
abgeloest; damit misst die Reihe eine Achse (Groesse) statt zweier
(Groesse und Familie).

⛔️ **Und seit dem 2026-09-12 ohne das dichte 14B** (Festlegung des
Projektinhabers): schlechterer Durchsatz als das Gemisch (5,25 gegen
10,1 Token je Sekunde), schlechtere Perplexitaet (11,54 gegen 10,42),
aufwendigeres Training, und 46 GB auf der Platte. ⚠️ Damit bleiben
**zwei** dichte Punkte; ein Groessenvergleich ueber die ganze Reihe
misst wieder einen Architekturunterschied mit.

Nur Basis-Varianten, keine Instruct-Varianten: Referenzmodell des Projekts
ist die Basis-Reihe (Scope-Entscheidung 12.15).
"""

MODEL_CONFIGS = {
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
    # Verifiziert gegen den lokalen Schnappschuss
    # models/Qwen3-0.6B/config.json, Revision
    # c1899de289a04d12100db370d81485cdf75e47ca, Lizenz Apache-2.0.
    #
    # ⚑ **Der neue Anker des Projekts** (2026-09-11, Festlegung des
    # Projektinhabers): Er loest Qwen2.5-0,5B ab. Damit liegt die
    # kleinste Groesse in derselben Familie wie 4B und 14B, und die
    # Reihe misst eine Achse (Groesse) statt zweier (Groesse und
    # Familie).
    #
    # Drei Unterschiede zum abgeloesten 0,5B, alle aus der Qwen3-Linie:
    #   qk_norm        False -> True   (Q und K je Kopf normiert, vor RoPE)
    #   attention_bias True  -> False  (keine Bias-Tensoren an q/k/v)
    #   head_dim       64    -> 128    (bei hidden_size/num_heads = 64,
    #                                   also wie bei 4B ausdruecklich
    #                                   gesetzt und nicht gerechnet,
    #                                   siehe Fund 59)
    #
    # ⚠️ `lm_head.weight` liegt bei diesem Schnappschuss **mit in den
    # Gewichten**, obwohl `tie_word_embeddings` True ist. Das ist kein
    # Widerspruch: Beim Binden ist der Kopf eine Sicht auf die
    # Einbettung, und HF legt ihn trotzdem ab. Massgeblich ist das Feld
    # aus der config, nicht das Vorhandensein des Tensors.
    "myelith-0.6b": {
        "family": "qwen3",
        "variant": "0.6b",
        "num_layers": 28,
        "hidden_size": 1024,
        "intermediate_size": 3072,
        "num_heads": 16,
        "num_kv_heads": 8,
        "head_dim": 128,
        "vocab_size": 151936,
        # theta_v 0.20.0 (Fund 368): die max_position_embeddings der
        # Modellkarte. Die Zeilen der RoPE-Tabellen kommen aus
        # rope.max_seq_len in theta_v/spec.json und nicht von hier; hier
        # stand bis dahin das Gegenteil. Der Lader nimmt das Kleinere.
        "max_context": 40960,
        "tie_word_embeddings": True,
        "attention_bias": False,
        "qk_norm": True,
        # Dichtes Modell, kein Mixture-of-Experts-Modell.
        "num_experts": 0,
        "verified": "models/Qwen3-0.6B/config.json@c1899de2",
        "hf_model_id": "Qwen/Qwen3-0.6B",
    },
    "myelith-4b": {
        "family": "qwen3",
        "variant": "4b",
        "num_layers": 36,
        "hidden_size": 2560,
        "intermediate_size": 9728,
        "num_heads": 32,
        "num_kv_heads": 8,
        "head_dim": 128,
        "vocab_size": 151936,
        # Wie beim 0,6B: 40960, die Zeilen der Tabellen kommen aus der Spec.
        "max_context": 40960,
        "tie_word_embeddings": True,
        "attention_bias": False,
        "qk_norm": True,
        # Dichtes Modell, kein Mixture-of-Experts-Modell.
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
    # Verifiziert gegen den lokalen Schnappschuss
    # models/Qwen3-14B/config.json, Revision
    # 40c069824f4251a91eefaf281ebe4c544efd3e18, Lizenz Apache-2.0.
    # Tensornamen zusaetzlich gegen die echte
    # model.safetensors.index.json gehalten: 443 Tensoren, darunter
    # `model.layers.<i>.self_attn.q_norm.weight`.
    #
    # ⛔️ **Hier stand `myelith-14b`, entfernt am 2026-09-12**
    # (Festlegung des Projektinhabers). Es war das groesste **dichte**
    # Modell des Projekts.
    #
    # **Begruendung:** schlechterer Durchsatz als das Gemisch (5,25
    # gegen 10,1 Token je Sekunde), schlechtere Perplexitaet (11,54
    # gegen 10,42), aufwendigeres Training, und es kostet 46 GB auf der
    # Platte ohne Mehrwert.
    #
    # ⚑ **Und ein Befund aus dem Messen stuetzt das:** Auf einer
    # Maschine mit 24 GiB ist das dichte Modell **unhandlicher als das
    # groessere Gemisch**. Eine Perplexitaetsmessung kostete dort 19 bis
    # 102 Minuten gegen zehn beim Gemisch. Ein dichtes Modell fasst bei
    # jedem Vorwaertspass alle Gewichte an, ein Gemisch je Token acht
    # von 128 Experten.
    #
    # ⚠️ **Was dabei verlorengeht, ist benannt:** Die dichte Reihe faellt
    # von drei Punkten auf zwei, und ein Groessenvergleich misst damit
    # wieder einen Architekturunterschied mit. Der Projektinhaber hat
    # das gegen den Plattenbedarf abgewogen.
    #
    "myelith-30b-a3b": {
        "family": "qwen3-moe",
        "variant": "30b-a3b",
        "num_layers": 48,
        "hidden_size": 2048,
        "intermediate_size": 6144,
        "num_heads": 32,
        "num_kv_heads": 4,
        "head_dim": 128,
        "vocab_size": 151936,
        "max_context": 40960,
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
    for model in ["myelith-0.6b", "myelith-30b-a3b", "qwen2.5-72b-instruct"]:
        for nodes in [2, 4, 8]:
            try:
                print_sharding_plan(model, nodes)
            except ValueError as e:
                print(f"\n{model} auf {nodes} Nodes: {e}")
