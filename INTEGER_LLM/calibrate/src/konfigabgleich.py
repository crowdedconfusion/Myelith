"""
**Der Feldabgleich zwischen einem Modelleintrag und dem echten `config.json`.**

Herausgeloest am 2026-10-19 aus `tests/test_modellkonfig_vollstaendig.py`,
damit die Probe und das Bauskript (`tools/modell_bauen.py`) **dieselbe**
Pruefung fahren: Ein Eintrag, den das Skript aus einem fremden `config.json`
erzeugt, wird genau so gegen die Quelle gehalten wie ein von Hand
geschriebener. Zwei Fassungen dieser Regeln liefen auseinander, und die
zweite waere die schlechter gepruefte.

Die Begruendungen zu den einzelnen Tabellen stehen bei den Tabellen.
"""

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
    # ⚑ Nachgesehen am 2026-09-28 (ternaeres 8B, gespeichert mit
    # transformers 4.57): die allgemeinen Vorgaben, die diese Fassung
    # beim Speichern jedes Config ausschreibt. Generierung, Strahlsuche,
    # Encoder-Decoder, Ausgabeschalter, Klassifikationskoepfe; nichts
    # davon betrifft den Export eines reinen Decoders.
    "add_cross_attention", "bad_words_ids", "begin_suppress_tokens",
    "chunk_size_feed_forward", "cross_attention_hidden_size",
    "decoder_start_token_id", "diversity_penalty", "do_sample",
    "early_stopping", "encoder_no_repeat_ngram_size",
    "exponential_decay_length_penalty", "finetuning_task",
    "forced_bos_token_id", "forced_eos_token_id", "id2label",
    "is_decoder", "is_encoder_decoder", "label2id", "length_penalty",
    "max_length", "min_length", "no_repeat_ngram_size", "num_beam_groups",
    "num_beams", "num_return_sequences", "output_attentions",
    "output_hidden_states", "output_scores", "prefix", "problem_type",
    "pruned_heads", "remove_invalid_values", "repetition_penalty",
    "return_dict", "return_dict_in_generate", "sep_token_id",
    "suppress_tokens", "task_specific_params", "temperature",
    "tf_legacy_loss", "tie_encoder_decoder", "tokenizer_class", "top_k",
    "top_p", "torchscript", "typical_p", "use_bfloat16",
    # Ebenso nachgesehen: `layer_types` ist 36-mal „full_attention“,
    # also nichts, was ein dichtes Qwen3 nicht ohnehin ist; `no_bias`
    # ist True und deckt sich mit `attention_bias` False, das geprueft
    # wird.
    "layer_types", "no_bias",
    # ⚑ Nachgesehen am 2026-10-19 (Qwen3.8-Flash-Next):
    # `language_model_only` schaltet den Sichtturm ab und betrifft den
    # Textpfad nicht; `mtp` ist der Block des MTP-Kopfs, der ausgeschlossen ist
    # (Fund 416); `split_ngram_parts` ist die Teilung der N-Gramm-Tabelle im
    # Checkpoint, die der Export wieder zusammensetzt.
    "language_model_only", "mtp", "split_ngram_parts",
    # Seit `rope_parameters` mitgezogen wird (2026-10-19): Die Verschraenkung
    # der drei Positionsachsen ist fuer reinen Text ohne Wirkung, weil alle
    # drei Achsen dieselbe Position tragen (beim 35B wie bei Flash-Next).
    "mrope_interleaved",
    # ⚑ Nachgesehen am 2026-10-19: `number_of_conv_states` zaehlt die
    #   Faltungszustaende im Cache der Vorlage (3 mit N-Gramm-Einbettung), eine
    #   Angabe ihrer Speicherverwaltung und keine des Modells.
    "number_of_conv_states",
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

# Felder, die der Eintrag umgerechnet fuehrt und die oben nachgerechnet
# werden (1b), mit dem Feld des Eintrags.
ABGELEITET = {
    "ple_layer_ids": "ple_ebenen",
}
# Felder, die oben ausdruecklich geprueft werden, auch wenn der Eintrag sie
# nicht fuehrt (`rope_type: default` heisst: kein `rope_art`).
AUSDRUECKLICH_GEPRUEFT = {"rope_type"}

UEBERSETZUNG = {
    # Die Saat der N-Gramm-Kennungen (Bauart mit vier Stroemen).
    "seed": "ngram_seed",
    "num_attention_heads": "num_heads",
    "num_key_value_heads": "num_kv_heads",
    "num_hidden_layers": "num_layers",
    "max_position_embeddings": "max_context",
    # Die Felder einer RoPE-Skalierung, aus `rope_scaling` flachgezogen.
    "rope_type": "rope_art",
    "factor": "rope_yarn_faktor",
    "original_max_position_embeddings": "rope_yarn_urlaenge",
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
        # ⛔️ **Seit Qwen3.8 heisst der Behaelter `rope_parameters`** (2026-10-19).
        #   Gezogen wurde nur `rope_scaling`; die Basis 1e7, die Teildrehung
        #   und die Abschnitte von Flash-Next prueften sich damit gar nicht, und
        #   die Probe sagte nichts dazu.
        elif k in ("rope_scaling", "rope_parameters") and isinstance(v, dict):
            for rk, rv in v.items():
                aus[rk] = rv
        else:
            aus[k] = v
    return aus




def feldabgleich(eintrag: dict, echt: dict) -> dict:
    """**Jedes Feld des Eintrags gegen den Config, und was niemand liest.**

    `echt` ist der flachgezogene Config ([`flach`]). Zurueck kommt ein
    Woerterbuch mit:
    - `abweichend`: `(feld, config, eintrag)` ohne die absichtlichen;
    - `absichtlich`: `(feld, config, eintrag, grund)`;
    - `abgeleitet`: Meldungen der nachgerechneten Groessen;
    - `ungelesen`: Felder des Config, die weder der Eintrag fuehrt noch
      `NICHT_GEBRAUCHT` mit Grund nennt.
    """
    abweichend, absichtlich, abgeleitet = [], [], []
    for ck, wert in echt.items():
        mk = UEBERSETZUNG.get(ck, ck)
        if mk not in eintrag:
            continue
        meiner = eintrag[mk]
        if ck == "layer_types" and isinstance(wert, list):
            # ⚑ `indexed_attention` ist eine volle Aufmerksamkeit mit
            #   Blockauswahl; die Auswahl steht als Merkmal daneben.
            wert = ["full_attention" if t == "indexed_attention" else t for t in wert]
        if isinstance(wert, float) or isinstance(meiner, float):
            try:
                gleich = abs(float(wert) - float(meiner)) < 1e-9
            except (TypeError, ValueError):
                gleich = False
        else:
            gleich = wert == meiner
        if gleich:
            continue
        if ck in ABSICHTLICH_ANDERS:
            absichtlich.append((ck, wert, meiner, ABSICHTLICH_ANDERS[ck]))
        else:
            abweichend.append((ck, wert, meiner))

    # Abgeleitete Groessen gegen ihre Quelle: eine Zahl, die aus einer
    # anderen folgt, wird nachgerechnet und nicht geglaubt.
    paf = echt.get("partial_rotary_factor")
    if paf is not None and "rotary_dim" in eintrag:
        soll = int(eintrag["head_dim"] * paf)
        if eintrag["rotary_dim"] != soll:
            abgeleitet.append(f"rotary_dim: {eintrag['rotary_dim']}, aber head_dim "
                              f"{eintrag['head_dim']} * {paf} = {soll}")
    if "rope_type" in echt:
        soll_art = None if echt["rope_type"] == "default" else echt["rope_type"]
        if eintrag.get("rope_art") != soll_art:
            abgeleitet.append(f"rope_art: {eintrag.get('rope_art')!r}, aber rope_type {echt['rope_type']!r}")
    if "ple_layer_ids" in echt and "ple_ebenen" in eintrag:
        soll = [i - 1 for i in echt["ple_layer_ids"]]
        if eintrag["ple_ebenen"] != soll:
            abgeleitet.append(f"ple_ebenen: {eintrag['ple_ebenen']}, aber ple_layer_ids "
                              f"{echt['ple_layer_ids']} (ab eins) ergibt {soll}")

    ungelesen = sorted(
        ck for ck in echt
        if UEBERSETZUNG.get(ck, ck) not in eintrag and ck not in NICHT_GEBRAUCHT
        and not (ck in ABGELEITET and ABGELEITET[ck] in eintrag)
        and ck not in AUSDRUECKLICH_GEPRUEFT
    )
    return {"abweichend": abweichend, "absichtlich": absichtlich,
            "abgeleitet": abgeleitet, "ungelesen": ungelesen}
