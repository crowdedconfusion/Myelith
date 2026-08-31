"""
Sammelt Aktivierungsstatistiken (Min, Max, p99, AbsMean) per Forward-Hook.

Gehookt werden (Konvention fuer die Schluessel in scales.json — die Runtime
verwendet exakt dieselben Namen, siehe runtime/src/loader.rs):
- Ausgaenge aller Projektionen (q/k/v/o/gate/up/down_proj) -> deren
  Ausgangsskalen,
- Ausgaenge der RMSNorm-Module (input_layernorm, post_attention_layernorm,
  model.norm) -> Eingangsskalen der Folgeprojektionen bzw. des LM-Heads,
- Ausgang des self_attn-Moduls -> Eingangsskala von o_proj,
- EINGANG von down_proj (h = silu(gate)*up) -> Eingangsskala von down_proj.

**Fund 20 (2026-08-18): Per-Kanal-Statistik fuer die drei Residualstrom-
Segmente** (`*_layernorm.input`, `model.norm.input`). Qwen2.5-7B zeigt an
Position 0 "Massive Activations" (Sun et al. 2024) in wenigen festen
Kanaelen (absmax ~9600 gegenueber ~10 im Rest) — eine globale (skalare)
Statistik verwischt das voellig. Diese drei Hooks sammeln deshalb
ZUSAETZLICH ein Per-Kanal-AbsMax (letzte Dimension = hidden_size), aus dem
`scales.py` je Kanal einen eigenen Shift ableitet. Alle anderen Hooks
bleiben skalar — nur der Residualstrom selbst zeigt den Ausreisser (siehe
`INTEGER_LLM/kernels/src/rmsnorm.rs`-Modulkopf).
"""

import re
import torch
from collections import defaultdict

# Projektkonvention: p99 wird aus einer begrenzten Stichprobe geschaetzt;
# absmax/absmean laufen inkrementell ueber alle Werte mit.
_MAX_SAMPLES = 20_000
_CHUNK = 10_000



_EXPERTE = re.compile(r"\.mlp\.experts\.\d+\.")


def _sammelschluessel(name: str) -> str:
    """Fuehrt die Projektionen aller Experten einer Layer auf **einen**
    Statistik-Schluessel zurueck.

    `model.layers.7.mlp.experts.42.gate_proj` wird zu
    `model.layers.7.mlp.gate_proj`. Fuer Modelle ohne Experten aendert
    die Funktion nichts.

    ## Warum eine Skala je Layer und nicht je Experte

    **Das ist eine Festlegung mit einem Preis, und der Preis gehoert
    dazugesagt.** Je Experte waere feiner: Jeder Experte hat seine eigene
    Aktivierungsverteilung, und eine gemeinsame Skala wird von dem
    Experten mit der breitesten bestimmt, waehrend schmale an Aufloesung
    verlieren. Das ist genau das Argument aus Fund 20, eine Ebene hoeher.

    **Dagegen steht die Abdeckung.** Bei 128 Experten und Top-8 sieht
    jeder Experte nur `8192 x 8/128 = 512` Kalibriertoken, ein Sechzehntel
    dessen, was eine dichte MLP bekommt. Eine Skala ist ein
    Absmax-Schaetzer, und ein Absmax aus 512 Beobachtungen **unterschaetzt
    den wahren Bereich systematisch**. Eine unterschaetzte Skala klippt im
    Betrieb.

    Die gemeinsame Skala kann das nicht: Das Absmax ueber alle 8192 Token
    ist mindestens so gross wie das jedes einzelnen Experten. Sie kostet
    Aufloesung, sie klippt nicht. **Zu grob ist heilbar, zu knapp nicht**,
    und das ist dieselbe Richtung, in die schon die Slashing-Spannen und
    die S_min-Aufrundung entschieden wurden.

    **Umzustellen, sobald die Kalibrierbasis waechst.** Bei
    CALIB_WIKITEXT_SEQUENCES = 1024 statt 64 saehe jeder Experte dieselben
    8192 Token wie heute eine dichte MLP, und dann ist je Experte die
    bessere Wahl. Der Umbau betrifft diese Funktion, `MoeLayer` im
    Runtime-Modell und den Loader.
    """
    return _EXPERTE.sub(".mlp.", name)


class ActivationStatsCollector:
    def __init__(self):
        self.stats = defaultdict(lambda: {
            "min": float("inf"),
            "max": float("-inf"),
            "abs_sum": 0.0,
            "count": 0,
            "values": [],
            "channel_absmax": None,  # Fund 20: nur fuer Residualstrom-Segmente befuellt
        })
        self._handles = []

    def _merke(self, name, t, per_channel=False):
        """Traegt einen Aktivierungstensor in die Statistik unter `name` ein.

        Herausgeloest am 2026-08-25, damit der MoE-Hook (siehe
        [`_make_moe_hook`]) dieselbe Aufzeichnung benutzt und nicht eine
        zweite, die beim naechsten Formatwechsel auseinanderlaeuft.
        """
        if isinstance(t, tuple):
            t = t[0]
        if not isinstance(t, torch.Tensor):
            return
        t_det = t.detach().float().cpu()
        vals = t_det.flatten()
        s = self.stats[name]
        s["min"] = min(s["min"], vals.min().item())
        s["max"] = max(s["max"], vals.max().item())
        s["abs_sum"] += vals.abs().sum().item()
        s["count"] += vals.numel()
        if len(s["values"]) < _MAX_SAMPLES:
            s["values"].extend(vals.tolist()[:_CHUNK])
        if per_channel:
            # Letzte Dimension = hidden_size (Kanal); alle anderen
            # (Batch, Sequenzposition) werden fuers Kanal-AbsMax
            # ueber Forward-Aufrufe UND Positionen hinweg reduziert -
            # der Ausreisser an Position 0 muss die Skala genauso
            # bestimmen wie im echten Inferenzpfad.
            ch_absmax = t_det.reshape(-1, t_det.shape[-1]).abs().amax(dim=0)
            if s["channel_absmax"] is None:
                s["channel_absmax"] = ch_absmax
            else:
                s["channel_absmax"] = torch.maximum(s["channel_absmax"], ch_absmax)

    def _make_hook(self, name, take_input=False, per_channel=False):
        def hook(module, input, output):
            t = input[0] if take_input else output
            self._merke(name, t, per_channel)
        return hook

    def _make_moe_hook(self, layer_praefix):
        """Statistik fuer ein **verschmolzenes** Mixture-of-Experts-Modell.

        ## Warum es diesen Hook ueberhaupt braucht

        Die `index.json` von Qwen3-30B-A3B fuehrt 128 Experten je Layer
        als je drei eigene Tensoren. **transformers 5.15 laedt sie nicht
        so.** Es stapelt sie zu `experts.gate_up_proj` mit der Form
        `[E, 2*moe_inter, hidden]` und `experts.down_proj` mit
        `[E, hidden, moe_inter]`, und `mlp.experts` ist **ein** Modul,
        kein ModuleList aus 128.

        Damit feuert kein Hook, der auf `gate_proj` endet, und die drei
        Skalen `mlp.gate_proj`, `mlp.up_proj`, `mlp.down_proj.input`
        entstehen nie. Der Rust-Loader meldete am Ende eines mehrstuendigen
        Laufs „Fehlende kalibrierte Aktivierungsskala". Aufgefallen am
        2026-08-25, bevor der Lauf lief.

        ## Wie er es macht, und warum das eine Gegenprobe braucht

        Der Hook bekommt die Eingaben des Moduls
        (`hidden_states`, `top_k_index`, `top_k_weights`) und **rechnet
        die Expertenschleife nach**, um an die Zwischenwerte zu kommen.
        Das ist eine Nachbildung fremder Interna, und die veraltet
        stillschweigend, wenn transformers sie aendert.

        **Deshalb prueft der Hook seine eigene Nachbildung**: Am Ende
        muss sein nachgerechnetes Ergebnis der tatsaechlichen
        Modulausgabe entsprechen, bis auf Akkumulationsrauschen. Tut es
        das nicht, bricht der Lauf ab, statt falsche Skalen zu
        schreiben. Eine Nachbildung ohne diese Pruefung waere eine
        Behauptung ueber fremden Code.

        **Bis auf Rauschen, nicht bitgleich** - die Begruendung steht
        unten bei der Pruefung selbst. Kurz: `index_add_` ueber bf16 ist
        nicht assoziativ, und der erste Entwurf scheiterte an genau
        einer ulp.

        ## Was aufgezeichnet wird, und unter welchem Namen

        Unter den **Layer-Schluesseln**, nicht je Experte:
        `<layer>.mlp.gate_proj`, `.mlp.up_proj`, `.mlp.down_proj.input`.
        Die Begruendung steht bei [`_sammelschluessel`]: Bei 128 Experten
        und Top-8 saehe jeder Experte nur 512 Kalibriertoken, und ein
        Absmax aus 512 Beobachtungen unterschaetzt den wahren Bereich.
        Zu grob ist heilbar, zu knapp nicht.
        """
        def hook(modul, eingaben, ausgabe):
            if len(eingaben) < 3:
                raise RuntimeError(
                    f"{layer_praefix}.mlp.experts: erwartet werden drei Eingaben "
                    f"(hidden_states, top_k_index, top_k_weights), gesehen "
                    f"{len(eingaben)}. Die Signatur von Qwen3MoeExperts.forward "
                    "hat sich geaendert; der Nachbau in _make_moe_hook gehoert "
                    "dann ebenfalls angepasst."
                )
            hidden_states, top_k_index, top_k_weights = eingaben[:3]
            nachbau = torch.zeros_like(hidden_states)
            # Summe der Betraege aller Einzelbeitraege: die Bezugsgroesse
            # der Gegenprobe unten. Nicht die Ausgabe, siehe dort.
            summe_betraege = 0.0

            # Ab hier Zeile fuer Zeile wie Qwen3MoeExperts.forward, damit
            # die Gegenprobe unten ueberhaupt eine Chance hat.
            with torch.no_grad():
                expert_mask = torch.nn.functional.one_hot(
                    top_k_index, num_classes=modul.num_experts)
                expert_mask = expert_mask.permute(2, 1, 0)
                expert_hit = torch.greater(
                    expert_mask.sum(dim=(-1, -2)), 0).nonzero()

                for expert_idx in expert_hit:
                    expert_idx = expert_idx[0]
                    if expert_idx == modul.num_experts:
                        continue
                    top_k_pos, token_idx = torch.where(expert_mask[expert_idx])
                    current_state = hidden_states[token_idx]
                    gate, up = torch.nn.functional.linear(
                        current_state, modul.gate_up_proj[expert_idx]).chunk(2, dim=-1)
                    zwischen = modul.act_fn(gate) * up

                    self._merke(f"{layer_praefix}.mlp.gate_proj", gate)
                    self._merke(f"{layer_praefix}.mlp.up_proj", up)
                    self._merke(f"{layer_praefix}.mlp.down_proj.input", zwischen)

                    beitrag = torch.nn.functional.linear(zwischen, modul.down_proj[expert_idx])
                    beitrag = beitrag * top_k_weights[token_idx, top_k_pos, None]
                    summe_betraege += beitrag.float().abs().max().item()
                    nachbau.index_add_(0, token_idx, beitrag.to(nachbau.dtype))

            # **Die Gegenprobe, und warum sie nicht auf Gleichheit prueft.**
            #
            # Der erste Entwurf verlangte `torch.equal`. Er schlug beim
            # ersten Lauf fehl, mit einem groessten Abstand von
            # **0,00390625 = 2^-8**, also **genau einer ulp in bf16**.
            # Die Ursache ist nicht der Nachbau, sondern `index_add_`:
            # Eine Summe vieler bf16-Beitraege ist nicht assoziativ, und
            # schon eine andere Speicheranordnung waehlt einen anderen
            # Kernel mit anderer Summationsreihenfolge.
            #
            # Auf bitgleich zu bestehen hiesse hier, eine Eigenschaft zu
            # verlangen, die die Gleitkommarechnung nicht hat. Der
            # Ganzzahlpfad hat sie, das ist der ganze Sinn des Projekts,
            # aber dieser Hook laeuft auf der bf16-Seite.
            #
            # Gesucht wird deshalb, wogegen die Pruefung geschrieben ist:
            # eine **Bedeutungsaenderung**. Vertauschte gate- und
            # up-Haelften, ein anderer Aktivierungsterm, eine andere
            # Gewichtung - all das verschiebt das Ergebnis um Prozente
            # oder mehr, nicht um eine ulp.
            #
            # ⚑ **Die Bezugsgroesse sind die Summanden, nicht die Summe.**
            # Der zweite Entwurf nahm ein Prozent des groessten
            # Ausgabebetrags und scheiterte bei Ebene 12: Abstand
            # 0,00390625 gegen Schranke 0,003828125, bei einer Ausgabe
            # von 0,3828125. Das waren vier ulp der Ausgabe, also genau
            # das, was acht Additionsschritte in bf16 anrichten duerfen.
            #
            # Die Schranke folgt jetzt der Fehlerabschaetzung der
            # Summierung statt einer geratenen Prozentzahl: Jeder
            # Additionsschritt rundet um hoechstens eine halbe ulp, und
            # ulp ist in bf16 das 2^-8-fache des Betrags. Vier ulp je
            # Summand lassen dafuer reichlich Luft.
            #
            # **Die Trennschaerfe bleibt.** Vertauschte gate- und
            # up-Haelften oder ein anderer Aktivierungsterm verschieben
            # das Ergebnis in der Groessenordnung der Ausgabe selbst,
            # also um ein Vielfaches dieser Schranke. Gemessen wird der
            # Unterschied zwischen Rauschen und Bedeutung, und der ist
            # mehr als eine Groessenordnung.
            _BF16_ULP = 2.0 ** -8
            abstand = (nachbau.float() - ausgabe.float()).abs().max().item()
            schranke = max(1e-6, 4.0 * _BF16_ULP * summe_betraege)
            if abstand > schranke:
                raise RuntimeError(
                    f"{layer_praefix}.mlp.experts: Der Nachbau in _make_moe_hook "
                    f"weicht von der Modulausgabe ab (groesster Abstand {abstand}, "
                    f"Schranke {schranke}, Summe der Beitragsbetraege "
                    f"{summe_betraege}). "
                    "Das ist mehr als Akkumulationsrauschen: Die Interna von "
                    "Qwen3MoeExperts.forward haben sich geaendert. Der Lauf bricht "
                    "hier ab, weil die aufgezeichneten Skalen sonst zu einer "
                    "Rechnung gehoerten, die das Modell nicht ausfuehrt."
                )
        return hook

    def attach(self, model):
        proj_keys = ("q_proj", "k_proj", "v_proj", "o_proj",
                     "gate_proj", "up_proj", "down_proj")
        norm_keys = ("input_layernorm", "post_attention_layernorm")
        # Qwen3 (theta_v-Erweiterung 2026-08-25). Bei Qwen2.5 existieren
        # diese Module nicht, die Schleife trifft sie dann schlicht nie.
        qk_norm_keys = ("self_attn.q_norm", "self_attn.k_norm")
        # Die Router-Projektion eines Mixture-of-Experts-Modells heisst "mlp.gate"
        # und endet damit NICHT auf "gate_proj". Ohne eigenen Schluessel
        # bekaeme sie keinen Hook und der Loader faende keine Skala.
        router_keys = ("mlp.gate",)
        for name, module in model.named_modules():
            if any(name.endswith(k) for k in proj_keys):
                name = _sammelschluessel(name)
                h = module.register_forward_hook(self._make_hook(name))
                self._handles.append(h)
                if name.endswith("down_proj"):
                    # h = silu(gate)*up liegt nur am down_proj-Eingang an.
                    h_in = module.register_forward_hook(
                        self._make_hook(name + ".input", take_input=True))
                    self._handles.append(h_in)
            elif any(name.endswith(k) for k in norm_keys) or name == "model.norm":
                h = module.register_forward_hook(self._make_hook(name))
                self._handles.append(h)
                # Per-Segment-Skalen des Residualstroms (v0.12.21/spec 0.5.1):
                # Die Norm-EINGAENGE sind die Residual-Stromsegmente. Die
                # Spanne reicht von winzigen Embedding-Werten (~±0,2) bis zu
                # Ausreisser-Spitzen (~±1576, bei 7B ~±9600) — eine globale
                # Skala kann das nicht abdecken. Seit Fund 20 zusaetzlich
                # per_channel=True: eine Skala je Kanal statt je Segment.
                h_in = module.register_forward_hook(
                    self._make_hook(name + ".input", take_input=True, per_channel=True))
                self._handles.append(h_in)
            elif any(name.endswith(k) for k in router_keys):
                h = module.register_forward_hook(self._make_hook(name))
                self._handles.append(h)
            elif name.endswith(".mlp.experts") and hasattr(module, "gate_up_proj"):
                # Verschmolzenes Mixture-of-Experts-Modell (transformers 5.x).
                # `hasattr` statt Namenspruefung allein: Eine Fassung mit
                # einzelnen Expertenmodulen traegt hier kein gate_up_proj,
                # und dann greifen die proj_keys oben wie bei jedem
                # dichten Modell.
                praefix = name[: -len(".mlp.experts")]
                h = module.register_forward_hook(self._make_moe_hook(praefix))
                self._handles.append(h)
            elif any(name.endswith(k) for k in qk_norm_keys):
                # QK-Norm (Qwen3): RMSNorm je Kopf ueber head_dim, VOR RoPE.
                # Nur der AUSGANG wird gehookt; der Eingang ist die Ausgabe
                # von q_proj/k_proj und traegt bereits seine eigene Skala.
                #
                # Keine per_channel-Skala: Der Kanal waere hier eine
                # Position innerhalb des Kopfes, und alle Koepfe teilen
                # sich dasselbe Gamma. Eine Skala je Kopfposition wuerde
                # ueber alle Koepfe gemittelt und traefe damit keinen.
                # Die Massive Activations aus Fund 20 sind eine
                # Eigenschaft des Residualstroms, nicht der projizierten
                # Q/K.
                h = module.register_forward_hook(self._make_hook(name))
                self._handles.append(h)
            elif name.endswith(".self_attn"):
                # Modul-Ausgabe ist ein Tupel; der Hook nimmt Element 0.
                h = module.register_forward_hook(self._make_hook(name))
                self._handles.append(h)

    def detach(self):
        for h in self._handles:
            h.remove()
        self._handles.clear()

    def compute(self):
        result = {}
        for name, s in self.stats.items():
            values = torch.tensor(s["values"])
            entry = {
                "min": s["min"],
                "max": s["max"],
                "absmean": s["abs_sum"] / max(s["count"], 1),
                "absmax": max(abs(s["min"]), abs(s["max"])),
                "p99": torch.quantile(values.abs(), 0.99).item() if len(values) else 0.0,
            }
            if s["channel_absmax"] is not None:
                entry["channel_absmax"] = s["channel_absmax"].tolist()
            result[name] = entry
        return result
