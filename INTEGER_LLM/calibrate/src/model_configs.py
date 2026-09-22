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
  myelith-0.6b     MODELS/llm/Qwen3-0.6B/config.json@c1899de2
  myelith-4b       MODELS/llm/Qwen3-4B/config.json@1cfa9a72
  myelith-8b       MODELS/llm/Qwen3-8B/config.json@b968826d
  myelith-30b-a3b  MODELS/llm/Qwen3-30B-A3B/config.json@ad44e777

⚑ Seit dem 2026-09-11 (Festlegung des Projektinhabers) fuehrt das
Projekt ausschliesslich Qwen3. Qwen2.5-0,5B und Qwen2.5-7B sind
abgeloest; damit misst die Reihe eine Achse (Groesse) statt zweier
(Groesse und Familie).

⛔️ **Und seit dem 2026-09-12 ohne das dichte 14B** (Festlegung des
Projektinhabers): schlechterer Durchsatz als das Gemisch (5,25 gegen
10,1 Token je Sekunde), schlechtere Perplexitaet (11,54 gegen 10,42),
aufwendigeres Training, und 46 GB auf der Platte.

⚑ **Seit dem 2026-09-21 ist die dichte Reihe wieder dreipunktig**
(0,6B, 4B, 8B), nachdem sie mit dem Wegfall des 14B auf zwei gefallen
war. ⚠️ **Das war der benannte Preis dieses Wegfalls:** Ein
Groessenvergleich ueber zwei Punkte misst einen Architekturunterschied
mit, weil die Gerade durch zwei Punkte immer passt.

Nur Basis-Varianten, keine Instruct-Varianten: Referenzmodell des Projekts
ist die Basis-Reihe (Scope-Entscheidung 12.15).
"""

MODEL_CONFIGS = {
    # Verifiziert gegen den lokalen Snapshot MODELS/llm/Qwen3-4B/config.json,
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
    # MODELS/llm/Qwen3-0.6B/config.json, Revision
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
        # ⚑ **Ausdruecklich statt vorgegeben.** θ_v fuehrt 1e6 als
        # Vorgabe, und fuer die ganze Qwen3-Reihe stimmt sie. Sie steht
        # seit dem 2026-09-21 trotzdem in jedem Eintrag: Das
        # Qwen3.6-35B-A3B dreht mit 1e7, und weil die Vorgabe unsichtbar
        # war, hat das einen Abend gekostet. 📌 **Eine Vorgabe, die man
        # nicht sieht, prueft man auch nicht.**
        "rope_theta": 1_000_000.0,
        "rms_norm_eps": 1e-6,
        "hidden_act": "silu",
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
        "verified": "MODELS/llm/Qwen3-0.6B/config.json@c1899de2",
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
        # ⚑ **Ausdruecklich statt vorgegeben.** θ_v fuehrt 1e6 als
        # Vorgabe, und fuer die ganze Qwen3-Reihe stimmt sie. Sie steht
        # seit dem 2026-09-21 trotzdem in jedem Eintrag: Das
        # Qwen3.6-35B-A3B dreht mit 1e7, und weil die Vorgabe unsichtbar
        # war, hat das einen Abend gekostet. 📌 **Eine Vorgabe, die man
        # nicht sieht, prueft man auch nicht.**
        "rope_theta": 1_000_000.0,
        "rms_norm_eps": 1e-6,
        "hidden_act": "silu",
        # Wie beim 0,6B: 40960, die Zeilen der Tabellen kommen aus der Spec.
        "max_context": 40960,
        "tie_word_embeddings": True,
        "attention_bias": False,
        "qk_norm": True,
        # Dichtes Modell, kein Mixture-of-Experts-Modell.
        "num_experts": 0,
        "verified": "MODELS/llm/Qwen3-4B/config.json@1cfa9a72",
        "hf_model_id": "Qwen/Qwen3-4B",
    },
    # Verifiziert gegen den lokalen Snapshot
    # MODELS/llm/Qwen3-8B/config.json, Revision
    # b968826d9c46dd6066d109eabc6255188de91218, Lizenz Apache-2.0
    # (LICENSE liegt neben den Gewichten, ETHICS G7).
    #
    # ⚑ **Ein reiner Groessenwechsel gegenueber dem 4B**, und das ist
    # gegen die echte `model.safetensors.index.json` geprueft und nicht
    # angenommen: 399 Tensoren, dasselbe Muster wie beim 4B, **36 q_norm
    # und 36 k_norm** (also QK-Norm), **null Bias-Tensoren**.
    #
    # ⚠️ **tie_word_embeddings ist hier False, anders als beim 0,6B und
    # 4B.** Das Modell traegt ein eigenes `lm_head.weight`; der Export
    # schreibt es also mit, und Fund 390 (die doppelt abgelegte Matrix)
    # greift hier nicht. 📌 **Genau dieses Feld war der Anlass fuer die
    # Regel, dass ein Eintrag ohne `verified` nicht exportiert wird:** Es
    # laesst sich aus der Groesse nicht erraten, und ein falscher Wert
    # erzeugt keine Fehlermeldung, sondern ein Artefakt ohne LM-Kopf.
    #
    # ⚑ **Es macht die dichte Reihe wieder dreipunktig.** Seit dem
    # Wegfall des 14B am 2026-09-12 hatte sie nur zwei Punkte, und ein
    # Groessenvergleich ueber zwei Punkte misst einen
    # Architekturunterschied mit.
    "myelith-8b": {
        "family": "qwen3",
        "variant": "8b",
        "num_layers": 36,
        "hidden_size": 4096,
        "intermediate_size": 12288,
        "num_heads": 32,
        "num_kv_heads": 8,
        "head_dim": 128,
        "vocab_size": 151936,
        # ⚑ **Ausdruecklich statt vorgegeben.** θ_v fuehrt 1e6 als
        # Vorgabe, und fuer die ganze Qwen3-Reihe stimmt sie. Sie steht
        # seit dem 2026-09-21 trotzdem in jedem Eintrag: Das
        # Qwen3.6-35B-A3B dreht mit 1e7, und weil die Vorgabe unsichtbar
        # war, hat das einen Abend gekostet. 📌 **Eine Vorgabe, die man
        # nicht sieht, prueft man auch nicht.**
        "rope_theta": 1_000_000.0,
        "rms_norm_eps": 1e-6,
        "hidden_act": "silu",
        # Wie beim 0,6B und 4B: 40960; die Zeilen der RoPE-Tabellen
        # kommen aus rope.max_seq_len in theta_v/spec.json, nicht hier.
        "max_context": 40960,
        "tie_word_embeddings": False,
        "attention_bias": False,
        "qk_norm": True,
        # Dichtes Modell, kein Mixture-of-Experts-Modell.
        "num_experts": 0,
        "verified": "MODELS/llm/Qwen3-8B/config.json@b968826d",
        "hf_model_id": "Qwen/Qwen3-8B",
    },
    # Verifiziert gegen den lokalen Snapshot
    # MODELS/llm/Qwen3-30B-A3B/config.json, Revision
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
    # MODELS/llm/Qwen3-14B/config.json, Revision
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
        # ⚑ **Ausdruecklich statt vorgegeben.** θ_v fuehrt 1e6 als
        # Vorgabe, und fuer die ganze Qwen3-Reihe stimmt sie. Sie steht
        # seit dem 2026-09-21 trotzdem in jedem Eintrag: Das
        # Qwen3.6-35B-A3B dreht mit 1e7, und weil die Vorgabe unsichtbar
        # war, hat das einen Abend gekostet. 📌 **Eine Vorgabe, die man
        # nicht sieht, prueft man auch nicht.**
        "rope_theta": 1_000_000.0,
        "rms_norm_eps": 1e-6,
        "hidden_act": "silu",
        "max_context": 40960,
        "tie_word_embeddings": False,
        "attention_bias": False,
        "qk_norm": True,
        "num_experts": 128,
        "num_experts_per_tok": 8,
        "moe_intermediate_size": 768,
        "norm_topk_prob": True,
        "mlp_only_layers": [],
        "verified": "MODELS/llm/Qwen3-30B-A3B/config.json@ad44e777",
        "hf_model_id": "Qwen/Qwen3-30B-A3B",
    },
    # ================================================================
    # Die erste hybride Bauart des Projekts (2026-09-21)
    # ================================================================
    #
    # Verifiziert gegen den lokalen Snapshot
    # MODELS/llm/Qwen3.6-35B-A3B/config.json, Revision
    # 995ad96eacd98c81ed38be0c5b274b04031597b0, Lizenz Apache-2.0.
    # Jedes Feld aus `text_config` gelesen, nicht aus der Modellkarte.
    #
    # ⛔️ **Dieses Modell rechnet heute NICHT.** Der Eintrag ermoeglicht
    # den **Export**, nicht den Betrieb: 30 der 40 Ebenen sind rekurrente
    # Zustandsschichten, und der Rechenpfad hat sie noch nicht. Das
    # Artefakt deklariert `zustandsschicht`, und der Lader **weist es
    # ab** (`ModelDims::pruefe_merkmale`).
    #
    # ⚑ **Genau das ist der Zweck dieser Reihenfolge.** Der Export laesst
    # sich am echten Gegenstand pruefen, bevor der Kernel existiert, und
    # niemand kann versehentlich damit rechnen. **Ein Artefakt, das
    # entsteht und abgelehnt wird, ist ein Pruefmittel; eines, das
    # entsteht und stillschweigend falsch rechnet, ist ein Konsensbruch.**
    #
    # **Drei Felder verlangen eine Entscheidung, und sie stehen hier
    # statt in einer Fussnote:**
    #
    # 1. `intermediate_size` **gibt es in der echten Konfiguration
    #    nicht**, denn alle 40 Ebenen sind Gemisch; eine dichte
    #    FFN-Breite existiert nicht. Hier steht **0**, also „es gibt
    #    keine". ⚠️ Eine erfundene Zahl waere schlimmer als eine Null:
    #    Sie saehe aus wie eine Angabe.
    # 2. `mtp_num_hidden_layers` steht in der Quelle auf **1** und hier
    #    auf **0**. ⚑ **Dieses Feld beschreibt das Artefakt und nicht das
    #    Quellmodell**, und der Export nimmt den MTP-Kopf nicht mit. Wer
    #    hier 1 schriebe, liesse einen Lader nach Tensoren suchen, die
    #    nicht da sind.
    # 3. `experts_packed` steht auf **False**, obwohl die Quelle die
    #    Experten als zwei gepackte 3D-Tensoren je Ebene fuehrt.
    #    ⚑ **Auch dies beschreibt das Artefakt:** `entschmelze_experten`
    #    zerlegt sie beim Export in die konventionellen Namen je Experte.
    #    **Das Layout der Quelle ueberlebt den Export nicht, und das ist
    #    Absicht.**
    #
    # ⚠️ **Der Sichtturm ist nicht abgebildet und nicht vergessen.** Er
    # rechnet nur, wenn Bildmarken im Strom stehen, die der Textpfad
    # nicht hat; der Client hat fuer Bilder eine eigene Kiste. Die
    # Ausschlussliste in `quantize.py` nennt ihn.
    "myelith-35b-a3b": {
        # ⛔️ **Fund 423: die RMSNorm dieses Modells rechnet `1 + weight`.**
        #   `Qwen3_5MoeRMSNorm` legt ihr Gewicht mit `torch.zeros` an;
        #   ohne den Versatz normiert man mit einer Zahl um null.
        #   ⚠️ Gilt NICHT fuer `linear_attn.norm`, die ist auf eins
        #   angelegt und rechnet ohne Versatz.
        "rms_norm_offset": 1.0,
        "family": "qwen3_5-moe-hybrid",
        "variant": "35b-a3b",
        "num_layers": 40,
        "hidden_size": 2048,
        # Siehe Punkt 1 oben: es gibt keine dichte FFN-Breite.
        "intermediate_size": 0,
        "num_heads": 16,
        "num_kv_heads": 2,
        "head_dim": 256,
        "vocab_size": 248320,
        "max_context": 262144,
        "tie_word_embeddings": False,
        "attention_bias": False,
        "qk_norm": True,
        # --- Gemisch: 256 Experten, top-8, dazu ein geteilter.
        "num_experts": 256,
        "num_experts_per_tok": 8,
        "moe_intermediate_size": 512,
        # In der echten Konfiguration nicht vorhanden, also die Vorgabe.
        # ⛔️ **Hier stand bis zum 2026-09-21 `False`, und das war
        # erfunden.** Im `config.json` fehlt das Feld, und ich habe einen
        # Vorgabewert eingesetzt statt in der Umsetzung nachzusehen. Die
        # Vorlage normiert die Top-k-Gewichte **immer**, ohne jede
        # Abfrage:
        #
        #     router_top_value /= router_top_value.sum(dim=-1, keepdim=True)
        #
        # Ohne Normierung summieren sich die acht Gewichte nicht auf eins,
        # und der Gemischbeitrag wird kleingerechnet.
        #
        # 📌 **Ein fehlendes Feld ist keine Erlaubnis, einen Vorgabewert
        # zu erfinden.**
        "norm_topk_prob": True,
        "mlp_only_layers": [],
        "shared_expert_intermediate_size": 512,
        # Siehe Punkt 3 oben.
        "experts_packed": False,
        # --- Die hybride Ebenenfolge: 30 rekurrent, 10 Softmax.
        #     ⚑ Ausgeschrieben und nicht aus einem Abstand gerechnet:
        #     `full_attention_interval` ist 4, aber die Liste ist die
        #     Wahrheit, und eine gerechnete Folge waere eine zweite.
        "layer_types": (
            ["linear_attention"] * 3 + ["full_attention"]
        ) * 10,
        "linear_num_key_heads": 16,
        "linear_num_value_heads": 32,
        "linear_key_head_dim": 128,
        "linear_value_head_dim": 128,
        "linear_conv_kernel_dim": 4,
        # `attn_output_gate: true` in der Quelle; die naechste Generation
        # nennt an derselben Stelle einen Namen, und die Referenz rechnet
        # `ausgang * sigmoid(tor)`.
        "output_gate_type": "sigmoid",
        # Siehe Punkt 2 oben.
        # ⚠️ **Der Config sagt 1, hier steht 0, und das ist Absicht.** Der
        # MTP-Kopf ist beim Export ausgeschlossen (Fund 416); dieses Feld
        # sagt, wie viele MTP-Ebenen das ARTEFAKT traegt, nicht das
        # Quellmodell. 📌 Ein Feld, das anders heisst als das, was es
        # zaehlt, ist eine Falle; es heisst deshalb hier so und traegt
        # diesen Satz.
        "mtp_num_hidden_layers": 0,
        # head_dim 256 mal partial_rotary_factor 0,25. ⚑ Als Anzahl und
        # nicht als Verhaeltnis, denn dieses Schema traegt kein
        # Gleitkomma.
        # ⛔️ **rope_theta steht in `rope_scaling`, nicht daneben.**
        #
        # Im `config.json` ist `rope_theta` auf oberster Ebene **None**;
        # der echte Wert liegt in
        # `rope_scaling = {"mrope_interleaved": true,
        #                  "mrope_section": [11, 11, 10],
        #                  "partial_rotary_factor": 0.25,
        #                  "rope_theta": 10000000,
        #                  "rope_type": "default"}`.
        #
        # 📌 **Gefunden am 2026-09-21, nach Stunden Fehlersuche.** θ_v
        # fuehrte `rope_theta` als FORMATkonstante mit 1e6, und die gilt
        # fuer die ganze Qwen3-Reihe. Dieses Modell dreht zehnmal
        # langsamer. Mit der falschen Basis dreht Paar 16 um den Faktor
        # 3,2 zu schnell und Paar 31 um 9,3; die Positionsinformation
        # aller zehn Achtsamkeitsebenen ist damit zerstoert, und das
        # Modell erzeugte Kauderwelsch.
        #
        # ⚠️ **Ein Modellparameter gehoert nicht in θ_v.** Er steht
        # seither hier und landet ueber `artifact_model_config` in
        # `model_config.json`; θ_v behaelt ihn nur als Vorgabe fuer
        # Modelle ohne eigene Angabe (die ganze Qwen3-Reihe: 1e6).
        "rope_theta": 10_000_000.0,
        # ⚑ **Steht ausdruecklich im Config und wurde trotzdem muehsam
        # hergeleitet.** Am 2026-09-21 habe ich das Achtsamkeitstor ueber
        # eine fehlgeschlagene Formpruefung gefunden („shape [8192, 2048]
        # statt [4096, 2048]"), nachdem ich die Vorlage gelesen hatte.
        # `attn_output_gate: true` steht seit jeher im `config.json`.
        # 📌 **Wer ein Feld sucht, findet ein Feld; wer die Datei liest,
        # findet das Modell.**
        "attn_output_gate": True,
        # Die Abfolge der Ebenenarten: je drei rekurrente, dann eine
        # volle. ⚑ **Daraus wird `layer_types` abgeleitet**, statt sie in
        # vierzig Eintraegen zu tippen.
        "full_attention_interval": 4,
        "rms_norm_eps": 1e-6,
        "hidden_act": "silu",
        # ⚑ **Der Anteil steht im Config, die Stellenzahl wird daraus
        # gerechnet.** `256 * 0,25 = 64`. Beide stehen hier, damit die
        # Konfigprobe sie gegeneinander halten kann; wer nur `rotary_dim`
        # fuehrte, koennte es unbemerkt am Anteil vorbei tippen.
        "partial_rotary_factor": 0.25,
        "rotary_dim": 64,
        # Die drei Abschnitte der mehrdimensionalen Drehung. ⚠️ **Fuer
        # reinen Text ohne Wirkung**, weil dann alle drei Positionsachsen
        # gleich sind; sie steht hier, damit der naechste Leser sie nicht
        # fuer vergessen haelt, wenn ein Bildmodell dazukommt.
        "mrope_section": [11, 11, 10],
        "verified": "MODELS/llm/Qwen3.6-35B-A3B/config.json@995ad96e",
        "hf_model_id": "Qwen/Qwen3.6-35B-A3B",
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
_ARTIFACT_EXCLUDED_FIELDS = (
    "verified",
    "hf_model_id",
    # ⛔️ **Der Normversatz gehoert NICHT ins Artefakt** (Fund 423). Er
    #   ist eine Umrechnung beim Export und steckt danach in den
    #   Gewichten. Stuende er zusaetzlich im Artefakt, koennte ihn
    #   jemand ein zweites Mal anwenden, und das faellt nicht auf: Die
    #   Ausgabe behaelt ihre Groessenordnung, nur eben die einer anderen
    #   Funktion. 📌 Genau so ist Fund 423 entstanden.
    "rms_norm_offset",
)


# ⚑ **Die Abbildung von Konfigurationsfeldern auf Merkmalsnamen**
# (2026-09-21). Je Eintrag: der Name, und eine Bedingung an die Config.
#
# ⛔️ **Warum abgeleitet und nicht getippt.** Das Merkmal ist die Zusage
# „ohne dies rechnest du etwas anderes". Waere die Liste von Hand
# gepflegt, waere das erste vergessene Merkmal genau der Fall, gegen den
# sie gebaut ist: Ein Lader ohne diese Faehigkeit haette geladen und
# still falsch gerechnet.
#
# ⚠️ **Hier stehen auch Merkmale, die der heutige Rechenpfad NICHT kann.**
# Das ist Absicht: Erst wenn der Export sie benennt, kann der Lader sie
# ablehnen. Ein Merkmal, das niemand deklariert, wird auch nicht
# abgelehnt.
_MERKMALE = (
    ("moe", lambda c: c.get("num_experts", 0) > 0),
    ("dicht", lambda c: c.get("num_experts", 0) == 0),
    ("gebundene_einbettung", lambda c: bool(c.get("tie_word_embeddings"))),
    ("qk_norm", lambda c: bool(c.get("qk_norm"))),
    ("attention_bias", lambda c: bool(c.get("attention_bias"))),
    # --- Ab hier: Bauarten, die der Rechenpfad noch nicht kann. Sie
    #     stehen bewusst schon da, damit ein Artefakt damit **abgelehnt**
    #     wird statt stillschweigend falsch zu laufen.
    ("zustandsschicht", lambda c: "linear_attention" in (c.get("layer_types") or [])),
    ("gepackte_experten", lambda c: bool(c.get("experts_packed"))),
    ("geteilter_experte", lambda c: c.get("shared_expert_intermediate_size", 0) > 0),
    ("teildrehung", lambda c: c.get("rotary_dim", 0) > 0),
    ("mehrfachvorhersage", lambda c: c.get("mtp_num_hidden_layers", 0) > 0),
    ("ausgangstor", lambda c: bool(c.get("output_gate_type"))),
)


def merkmale_ableiten(config: dict) -> list:
    """Die Merkmalsliste eines Modells, aus seiner Konfiguration gerechnet.

    ⚑ **Sortiert**, damit zwei Laeufe dieselbe Datei erzeugen. Eine
    Reihenfolge, die an der Iteration eines Wortverzeichnisses haengt,
    macht aus einem bitgleichen Bau einen fast bitgleichen.
    """
    return sorted(name for name, trifft in _MERKMALE if trifft(config))


def artifact_model_config(name: str) -> dict:
    """Die Felder aus get_export_model_config(), die in model_config.json gehoeren."""
    config = get_export_model_config(name)
    aus = {k: v for k, v in config.items() if k not in _ARTIFACT_EXCLUDED_FIELDS}
    aus["merkmale"] = merkmale_ableiten(config)
    return aus


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
