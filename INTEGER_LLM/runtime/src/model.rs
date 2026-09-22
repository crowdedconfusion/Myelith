//! Kompletter Transformer-Graph fuer Qwen2.5-0.5B
//! 
//! Embedding -> [Layer x 24] -> Final RMSNorm -> LM Head
//! Jeder Layer: RMSNorm -> Attention -> ResAdd -> RMSNorm -> MLP -> ResAdd
// Die Schleifenvariable `row` ist hier die Ausgabe-Zeile der
// Gewichtsmatrix und wird als solche auch fuer die Per-Channel-Shifts
// gebraucht — ein Iterator ueber `logits` allein wuerde den Bezug
// verlieren. Der Heisspfad wird zudem gegen Golden Vectors geprueft;
// eine rein stilistische Umschreibung waere hier reines Risiko.
#![allow(clippy::needless_range_loop)]
// `Vec<(Vec<i8>, Vec<u8>)>`-Rueckgaben spiegeln das Artefaktformat
// (Gewichte + Per-Channel-Shifts). Ein Typalias wuerde die Struktur
// verstecken, die beim Lesen des Loaders gebraucht wird.
#![allow(clippy::type_complexity)]

use integer_llm_kernels::fixed_point::{clamp_i16, clamp_i16_from_i64, inv_sqrt_q15, rescale, rescale_i64};
use integer_llm_kernels::moe::{mische_experten, route_top_k};
use integer_llm_kernels::rmsnorm::{qk_norm_heads, rmsnorm_i16};
use integer_llm_kernels::linear::{
    add_bias_i16, linear_w8a16, linear_w8a16_pc, linear_w8a16_pc_stapel, linear_w8a16_stapel,
};
use integer_llm_kernels::fadenpool::rechnen_breit;
use integer_llm_kernels::rope::rotate_half_split_i16;
use integer_llm_kernels::attention::aufmerksamkeit_einer_abfrage;
use integer_llm_kernels::mlp::{mlp_int_experten, mlp_int_mit_spur, Expertenteil, Mlpspur};
use integer_llm_kernels::sampling::{argmax_int, sample_integer_cdf};
use crate::kv_cache::KVCache;
use crate::loader::{ThetaV, LoadedScales};

/// Die Bytes eines quantisierten Tensors: im Heap oder als Abbild der
/// Artefaktdatei.
///
/// ## ⚑ Fund 62 (2026-08-25): Ein 29-GiB-Artefakt gehört nicht in den Heap
///
/// Der Loader las jede Gewichtsdatei mit `std::fs::read` in einen `Vec`.
/// Für Qwen2.5-0,5B (0,74 GB) und 7B (8,1 GB) trägt das. Das Artefakt von
/// Qwen3-30B-A3B ist **29 GiB** gegen 24 GiB Arbeitsspeicher, und der
/// Versuch zeigte das schlechteste denkbare Verhalten: Während der
/// Prozess las, schrieb das Betriebssystem seinen Heap in die
/// Auslagerung, RSS fiel von 8,9 auf 1,9 GiB, während der Swap von 2,9
/// auf 4,1 GB wuchs. **Von der Platte lesen und sofort wieder auf die
/// Platte schreiben**, und zwar mit echten Schreibzugriffen auf die SSD.
///
/// **Der Unterschied ist die Art der Seite, nicht ihre Zahl.** Eine
/// anonyme Heap-Seite muss ausgelagert, also geschrieben werden. Eine
/// dateigestützte Seite ist sauber: Das System verwirft sie und liest
/// sie bei Bedarf neu. Dieselbe Datenmenge, aber ohne Schreiblast und
/// ohne Auslagerungsdruck.
///
/// **Für ein Mixture-of-Experts-Modell ist es mehr als eine Notlösung.** Bei
/// Top-8 von 128 rührt ein Token nur ein Sechzehntel der
/// Expertengewichte an. Die übrigen bleiben ungelesen auf der Platte,
/// statt Speicher zu belegen, den sie nie brauchen. Der Zuschnitt
/// „jeder Knoten hält alle Experten seiner Layer" wird damit erst
/// bezahlbar.
pub enum Gewichtsdaten {
    /// Im Heap, wie bisher. Für kleine Tensoren und für Tests.
    Speicher(Vec<i8>),
    /// Abbild der Artefaktdatei.
    Abbild(memmap2::Mmap),
}

impl std::ops::Deref for Gewichtsdaten {
    type Target = [i8];

    fn deref(&self) -> &[i8] {
        match self {
            Gewichtsdaten::Speicher(v) => v,
            Gewichtsdaten::Abbild(abbild) => {
                // SICHERHEIT: `i8` und `u8` haben dieselbe Größe und
                // dieselbe Ausrichtung, und jedes Bitmuster ist für
                // beide gültig. Der Zeiger stammt aus einem gültigen
                // Abbild, das mindestens so lange lebt wie die
                // zurückgegebene Referenz (beide hängen an `self`).
                // Geschrieben wird nie: Das Abbild ist nur lesend
                // geöffnet.
                unsafe {
                    std::slice::from_raw_parts(
                        abbild.as_ptr() as *const i8,
                        abbild.len(),
                    )
                }
            }
        }
    }
}

impl Gewichtsdaten {
    /// **Sagt dem System, dass diese Bytes gleich gebraucht werden.**
    ///
    /// ## ⚑ Fund 331 (2026-09-11): Nicht die Bandbreite fehlte, sondern die Buendelung
    ///
    /// Ein Abbild holt seine Seiten **beim ersten Zugriff, eine nach der
    /// anderen**. Fuer Qwen3-30B-A3B heisst das je Token 384
    /// Expertenbesuche zu je drei Matrizen zu je 1,5 MB, und jede
    /// dieser Matrizen zerfaellt in 96 Seiten, die der Kern einzeln und
    /// **synchron** von der Platte holt. Gemessen auf kalten Ebenen:
    ///
    /// | Zugriffsart                          | Durchsatz  | je Ebene |
    /// |--------------------------------------|------------|----------|
    /// | Abbild, Seite fuer Seite (vorher)    | 0,44 GB/s  | 85,5 ms  |
    /// | Abbild mit `MADV_WILLNEED` (jetzt)   | 3,41 GB/s  | 11,1 ms  |
    /// | `pread` in einen Puffer              | 5,24 GB/s  |  7,2 ms  |
    ///
    /// **Die Platte war nie das Problem.** Sie liefert 5 GB/s; die
    /// Einzelseitenfehler holten 0,44 davon ab. Ein einziger Rat an den
    /// Kern, bevor der erste Experte rechnet, buendelt die 2 304 Seiten
    /// einer Ebene zu einem Lauf und bringt das Siebeneinhalbfache.
    ///
    /// ⚑ **`pread` waere noch schneller und kommt trotzdem nicht in
    /// Frage.** Es braeuchte einen Puffer im Heap, und genau den hat
    /// Fund 62 abgeschafft: Eine anonyme Seite muss ausgelagert werden,
    /// eine dateigestuetzte wird verworfen. Der Unterschied von 3,4 zu
    /// 5,2 GB/s ist den Rueckfall in Auslagerungsdruck nicht wert.
    ///
    /// ⚑ **An der Rechnung aendert das nichts.** Der Rat sagt, *wann*
    /// Bytes ankommen, nicht *welche*. Die Gegenprobe ist trotzdem
    /// gelaufen: `decode_digest` unveraendert, Konformitaet 44/44.
    ///
    /// Ein Fehlschlag wird verschluckt. Ein Rat, den das System
    /// ausschlaegt, ist kein Fehler, sondern der Zustand von vorher.
    ///
    /// 📌 **Gemessen und verworfen (2026-09-14): ein Zeitfenster je Tensor.**
    /// Im Decode des 30B liegen rund 28 % des Hauptfadens hier, weil jeder
    /// Token acht Experten je Ebene neu ankuendigt. Ganz ohne Rat wurde der
    /// Decode langsamer (10,3 bis 10,9 gegen 12,7 Token/s), mit einem Rat
    /// hoechstens alle zwei Sekunden je Tensor zuerst 5 % schneller. Nach
    /// der gruppierten Vorbereitung (Fund 371) blieben im Wechsel gemessen
    /// 13,9 gegen 13,6 Token/s, innerhalb der Streuung; das Fenster ist
    /// deshalb nicht gebaut.
    pub fn vorbereiten(&self) {
        if let Gewichtsdaten::Abbild(abbild) = self {
            vorrat_ankuendigen(abbild);
        }
    }
}

/// **Der Rat an den Kern, und warum er eine eigene Funktion ist.**
///
/// ⛔️ **`memmap2::Advice` gibt es nur unter Unix** (`#[cfg(unix)]` in
/// `lib.rs`), und der Windows-Bau der CI ist daran zerbrochen. Ein
/// `#[cfg]` an zwei Aufrufstellen waere zweimal dieselbe Entscheidung;
/// hier steht sie einmal.
///
/// ⚑ **Unter Windows bleibt es beim Zustand von vorher**, also bei
/// Seitenfehlern, die einzeln bedient werden. Das Gegenstueck dort
/// hiesse `PrefetchVirtualMemory`, und `memmap2` bietet es nicht an.
/// **Eine eigene Anbindung waere unsicherer Code fuer eine Plattform,
/// auf der niemand gemessen hat**, und ungemessene Optimierung ist in
/// diesem Projekt keine.
///
/// ⚠️ **An der Rechnung aendert der Rat nichts**, deshalb ist sein
/// Fehlen kein Unterschied im Ergebnis, sondern nur einer in der Zeit:
/// Die Digests sind auf beiden Plattformen dieselben.
#[cfg(unix)]
fn vorrat_ankuendigen(abbild: &memmap2::Mmap) {
    let _ = abbild.advise(memmap2::Advice::WillNeed);
}

#[cfg(not(unix))]
fn vorrat_ankuendigen(_abbild: &memmap2::Mmap) {}

/// Dasselbe fuer den Lader, der sein Abbild noch nicht eingepackt hat.
pub fn abbild_vorbereiten(abbild: &memmap2::Mmap) {
    vorrat_ankuendigen(abbild);
}

impl From<Vec<i8>> for Gewichtsdaten {
    fn from(v: Vec<i8>) -> Self {
        Gewichtsdaten::Speicher(v)
    }
}

impl std::fmt::Debug for Gewichtsdaten {
    /// Zeigt Herkunft und Länge, **nicht die Bytes**. Ein Tensor mit
    /// drei Millionen Einträgen im Fehlertext ist keine Hilfe.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let art = match self {
            Gewichtsdaten::Speicher(_) => "Speicher",
            Gewichtsdaten::Abbild(_) => "Abbild",
        };
        write!(f, "Gewichtsdaten::{art}({} Byte)", self.len())
    }
}

/// Quantisierungs-Metadaten fuer einen Tensor.
#[derive(Debug, Clone)]
pub struct QTensor {
    /// Hinter `Arc`, damit `QTensor` klonbar bleibt: Ein Abbild lässt
    /// sich nicht kopieren, und der Loader reicht denselben Tensor an
    /// mehrere Stellen weiter. Der `Arc` kostet einen Zähler, nicht die
    /// Daten.
    pub data: std::sync::Arc<Gewichtsdaten>,      // flat, row-major
    pub shape: Vec<usize>,
    /// Zweierpotenz-Shift je Zeile (theta_v 0.7.0: Per-Channel-Skalen;
    /// bei 1D-Tensoren wie Biases/Gammas je Element). Ältere Artefakte mit
    /// Per-Tensor-Skala werden vom Loader als replizierter Shift geladen.
    pub shifts: Vec<u8>,    // Rechts-Shifts fuer Reskalierung, len == shape[0]
}

impl QTensor {
    /// Aus Bytes im Heap. Für Tests und für kleine Tensoren.
    pub fn aus_speicher(data: Vec<i8>, shape: Vec<usize>, shifts: Vec<u8>) -> Self {
        QTensor {
            data: std::sync::Arc::new(Gewichtsdaten::Speicher(data)),
            shape,
            shifts,
        }
    }

    /// Reicht [`Gewichtsdaten::vorbereiten`] durch. Die Shifts liegen
    /// im Heap und sind ohnehin da.
    pub fn vorbereiten(&self) {
        self.data.vorbereiten();
    }

    pub fn n_elements(&self) -> usize {
        self.shape.iter().product()
    }

    pub fn rows(&self) -> usize {
        self.shape[0]
    }

    pub fn cols(&self) -> usize {
        self.shape[1]
    }

    pub fn row(&self, idx: usize) -> Vec<i8> {
        let cols = self.cols();
        self.data[idx * cols .. (idx + 1) * cols].to_vec()
    }
}

/// Kalibrierte Per-Layer-Aktivierungsskalen (Zweierpotenzen, aus
/// `scales.json`; Schluessel-Konvention identisch zu
/// `calibrate/src/stats.py`). Seit dem Numerik-Realitaetsabgleich (v0.12.20)
/// tragen Aktivierungen int16 mit diesen Skalen; der Loader validiert die
/// Vollstaendigkeit beim Modellbau.
#[derive(Debug, Clone)]
pub struct LayerScales {
    /// Ausgang von input_layernorm = Eingang des Mischers.
    pub norm_attn_frac: u8,
    /// **Die Skalen der vollen Achtsamkeit**, oder `None` bei einer
    /// rekurrenten Ebene.
    ///
    /// ⛔️ **Warum `Option` und nicht einfach Zahlen darin.** Eine
    /// Zustandsebene hat **kein** `q_proj`: Ihre Projektion ist
    /// verschmolzen (`in_proj_qkv`), also gibt es genau eine Skala und
    /// nicht drei. Sie in drei Felder zu schreiben hiesse, **dieselbe
    /// Zahl an drei Orten** zu fuehren, und `attn_out_frac` waere fuer
    /// eine Ebene ohne Achtsamkeit ein Name, der luegt.
    ///
    /// ⚠️ **Eine erfundene Skala ist eine stille falsche Zahl**, und
    /// genau die sucht dieses Projekt am laengsten.
    pub achtsamkeit: Option<Achtsamkeitsskalen>,
    /// Ausgang von post_attention_layernorm = Eingang von gate/up_proj.
    pub norm_mlp_frac: u8,
    /// Ausgaenge von gate-/up_proj.
    pub gate_frac: u8,
    pub up_frac: u8,
    /// h = silu(gate)*up = Eingang von down_proj.
    pub down_in_frac: u8,
    /// Residualstrom-Segment am Eingang dieses Layers
    /// (Eingang von input_layernorm). Per-Segment-Skalen seit spec 0.5.1:
    /// die Spanne des Stroms reicht von winzigen Embedding-Werten bis zu
    /// Ausreisser-Spitzen — eine globale Skala wuerde einen der beiden
    /// Bereiche zerstoeren. Seit Fund 20 (theta_v 0.11.0) zusaetzlich eine
    /// Skala JE KANAL: bei Qwen2.5-7B tragen 3-4 feste Kanaele an
    /// Position 0 "Massive Activations" (absmax ~9600 gegenueber ~10 im
    /// Rest, Sun et al. 2024) - eine gemeinsame Skala fuers ganze Segment
    /// wuerde jeden anderen Kanal auf Schrittweite 0,5 zwingen.
    pub residual_in_frac: Vec<u8>,
    /// Mittleres Residualstrom-Segment zwischen erstem Residual-Add und
    /// post_attention_layernorm. Ebenfalls Per-Kanal (Fund 20).
    pub residual_mid_frac: Vec<u8>,
}

/// Die Aktivierungsskalen der vollen Achtsamkeit.
#[derive(Debug, Clone)]
pub struct Achtsamkeitsskalen {
    /// Ausgaenge der q/k/v-Projektionen (Q/K/V-Skala, beeinflusst
    /// score_shift und KV-Cache-Reskalierung).
    pub q_frac: u8,
    pub k_frac: u8,
    pub v_frac: u8,
    /// Ausgang des Attention-Moduls = Eingang von o_proj.
    pub attn_out_frac: u8,
}

/// Die Aktivierungsskalen einer rekurrenten Zustandsschicht.
///
/// ⚑ **Je Stelle eine, und keine davon hat ein Gegenstueck in der
/// Achtsamkeit.** Die Projektion ist verschmolzen, die Faltung sitzt
/// dazwischen, und `z`, `a` und `b` gibt es dort gar nicht.
#[derive(Debug, Clone)]
pub struct Zustandsskalen {
    /// Ausgang von `in_proj_qkv` = Eingang der Faltung.
    pub qkv_frac: u8,
    /// Ausgang der Faltung, **gemeinsam kalibriert**.
    ///
    /// ⚠️ Nur noch fuer die Meldung; gerechnet wird mit den drei
    /// Skalen darunter (Fund 421).
    pub konv_frac: u8,
    /// Die Skala je Kanal am Faltungsausgang, drei Laeufe: erst `q`,
    /// dann `k`, dann `v`.
    ///
    /// # ⛔️ Fund 421: eine Skala fuer drei Groessen, die es nicht sind
    ///
    /// `in_proj_qkv` liefert Abfrage, Schluessel und Wert in **einem**
    /// Tensor, und der Export gab ihnen **eine** Schranke: den groessten
    /// Zeilenbetrag ueber alle Kanaele mal dem Eingangsgroesstwert. Die
    /// Schranke richtet sich damit nach `v`, und gemessen am
    /// Qwen3.6-35B-A3B liegt `v` rund zwanzigmal ueber `q` und `k`.
    /// Ergebnis bei `frac = 9`: `v` bekam 38 Zaehler Effektivwert, `q`
    /// 12 und `k` **9**. Das sind drei bis vier Bit.
    ///
    /// ⚑ **Und `q` und `k` werden gleich darauf auf Einheitslaenge
    /// gebracht.** Ihre Groesse ist danach weg; was bleibt, ist ihre
    /// **Richtung**, und die haengt allein an der Aufloesung. Eine Skala,
    /// die sich nach dem groessten der drei richtet, verschenkt sie
    /// genau dort, wo sie zaehlt.
    ///
    /// 📌 **Ein Tensor ist noch keine gemeinsame Groesse.** Was in einer
    /// Matrixmultiplikation zusammen herauskommt, muss nicht zusammen
    /// skaliert werden; die Frage ist nicht, wie es gerechnet wurde,
    /// sondern was damit geschieht.
    ///
    /// ⚑ **Die Schranke je Kanal steht im Artefakt**, es braucht keine
    /// neue Kalibrierung: `Summe_j |w[c][j]| * absmax(in_proj_qkv)`, und
    /// `silu` vergroessert den Betrag nicht.
    pub konv_fracs: Vec<u8>,
    /// Ausgang von `in_proj_z`, dem Ausgangstor.
    pub z_frac: u8,
    /// Ausgang von `in_proj_a`, woraus der Zerfall entsteht.
    pub a_frac: u8,
    /// Ausgang von `in_proj_b`, woraus `beta` entsteht.
    pub b_frac: u8,
    /// Ausgang der torgesteuerten Norm = Eingang von `out_proj`.
    pub norm_aus_frac: u8,
}

impl LayerScales {
    /// Die Achtsamkeitsskalen dieser Ebene.
    ///
    /// ⚠️ **Bricht ab bei einer rekurrenten Ebene**, aus demselben Grund
    /// wie [`TransformerLayer::achtsamkeit`]: Ein Ersatzwert waere eine
    /// falsche Zahl ohne Meldung.
    pub fn achtsamkeit(&self) -> &Achtsamkeitsskalen {
        self.achtsamkeit.as_ref().expect(
            "diese Ebene mischt rekurrent und hat keine Achtsamkeitsskalen; \
             der Aufrufer prueft die Ebenenart nicht",
        )
    }
}

/// Ein Transformer-Layer.
/// Was der Router einer MoE-Layer für **eine** Position entschieden hat.
///
/// Nur für Messungen erhoben, nie im Regelbetrieb: `forward_layer` nimmt
/// den Sammler als `Option` entgegen und rührt ihn sonst nicht an.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routingbefund {
    pub layer: usize,
    /// Die gewählten Experten, in Auswahlreihenfolge.
    pub experten: Vec<u16>,
    /// Wie viele **nicht** gewählte Experten denselben Logit tragen wie
    /// der zuletzt gewählte. Null heißt: Die Auswahl war eindeutig, ganz
    /// ohne Tie-Break. Siehe `kernels::moe::randgleichstaende`.
    pub randgleichstaende: usize,
    /// Dasselbe, aber über die **Wahrscheinlichkeiten** statt über die
    /// Logits gezählt.
    ///
    /// Die Vergleichszahl zur Festlegung in `kernels::moe`: Ausgewählt
    /// wird über die Logits, weil der Weg über die exp-Tabelle
    /// Gleichstände erzeugt, die es vorher nicht gab. Wie viele, sagt
    /// erst diese Zahl.
    pub randgleichstaende_wahrscheinlichkeit: usize,
    /// Die Mischgewichte der gewählten Experten, auf `prob_frac_bits`.
    ///
    /// # ⚑ Warum sie hier stehen (2026-09-05)
    ///
    /// Wegen **Fund 79**, und die Frage ist nicht akademisch, sobald das
    /// Primärmodell ein Expertengemisch wird. Der Ganzzahl-Softmax
    /// sättigt, und Sättigung ist für den Router ein **absorbierender
    /// Zustand**: Trägt der Gewinner exakt `1 << prob_frac_bits`, ist
    /// der Gradient **jedes** Routerlogits exakt null, aus
    /// Rechengründen und nicht durch Rundung. Ein Router, der einmal
    /// sicher genug war, bleibt es für immer.
    ///
    /// ⚑ **Ob das ein reales oder ein rechnerisches Problem ist,
    /// entscheidet eine Messung, und die braucht diese Zahlen.** Ohne
    /// sie liesse sich nur sagen, dass der Zustand existiert, nicht wie
    /// oft er eintritt.
    pub gewichte: Vec<i32>,
    /// Der grösste Abstand zwischen zwei Routerlogits an dieser Stelle.
    ///
    /// ⚑ Die Grösse, aus der die Sättigung folgt: Ab einem gewissen
    /// Abstand liefert die exp-Tabelle für den Verlierer null.
    pub logit_spanne: i32,
}

/// Der Feedforward-Teil einer Layer.
///
/// **Warum ein Enum und nicht drei Tensoren plus ein optionales
/// Mixture-of-Experts-Modell.** Ein Modell kann beides mischen: Qwen3 kennt das
/// Feld `mlp_only_layers`, mit dem einzelne Layer dicht bleiben, während
/// der Rest Experten hat. Bei Qwen3-30B-A3B ist die Liste leer, aber sie
/// existiert, und ein Typ, der den Mischfall nicht ausdrücken kann,
/// müsste beim ersten Modell mit gemischten Layern umgebaut werden.
///
/// Vor allem: Zwei `Option`-Felder ließen den Zustand „beides" und den
/// Zustand „keines" zu. Beides wäre ein Ladefehler, den erst der
/// Forward-Pass bemerkt.
pub enum Feedforward {
    /// Gate, Up, Down wie in Qwen2.5 und in jeder dichten Layer.
    Dense(DenseMlp),
    /// Ein Router und `num_experts` Experten, von denen je Token genau
    /// `top_k` rechnen.
    Moe(MoeLayer),
}

/// Die drei Matrizen einer Feedforward-Einheit.
///
/// Ein **Experte** eines MoE ist strukturell dasselbe wie eine dichte
/// MLP, nur schmaler (`moe_intermediate_size` statt
/// `intermediate_size`). Deshalb derselbe Typ: Eine eigene Struktur mit
/// denselben drei Feldern wäre eine zweite Wahrheit, die beim nächsten
/// Formatwechsel auseinanderliefe.
pub struct DenseMlp {
    pub gate_proj: QTensor,
    pub up_proj: QTensor,
    pub down_proj: QTensor,
}

impl DenseMlp {
    /// **Alle drei Matrizen auf einmal ankuendigen.**
    ///
    /// ⚑ **Drei Rate und nicht einer je Matrix, wenn sie gebraucht
    /// wird.** Wer vor `gate` raet, vor `up` raet und vor `down` raet,
    /// wartet drei Mal; wer vorher alle drei ankuendigt, wartet einmal.
    /// Siehe [`Gewichtsdaten::vorbereiten`].
    pub fn vorbereiten(&self) {
        self.gate_proj.vorbereiten();
        self.up_proj.vorbereiten();
        self.down_proj.vorbereiten();
    }
}

/// Ein Mixture-of-Experts-Modell: Router plus Experten.
/// **Der geteilte Experte eines Gemischs.**
///
/// ⚑ **Er feuert bei JEDEM Token**, anders als die gerouteten. Die
/// Vorlage rechnet `aus = experten + sigmoid(tor(x)) * geteilt(x)`.
///
/// ⚠️ **Das Tor liefert genau einen Wert je Token**, nicht einen je
/// Kanal: `Linear(hidden_size, 1)`. Ein Tor je Kanal waere eine andere
/// Schicht.
pub struct GeteilterExperte {
    /// Dieselben drei Matrizen wie eine dichte MLP, nur schmaler.
    pub mlp: DenseMlp,
    /// Die Torprojektion, `[1, hidden_size]`.
    pub tor: QTensor,
    /// Kalibrierte Ausgangsskala der Torprojektion.
    pub tor_frac: u8,
    /// Zwischengroesse des geteilten Experten.
    pub zwischen: usize,
    /// Kalibrierte Skalen seiner drei Matrizen.
    pub gate_frac: u8,
    pub up_frac: u8,
    pub down_in_frac: u8,
}

pub struct MoeLayer {
    /// Router-Projektion, `[num_experts, hidden_size]`. Liefert je
    /// Experte einen Logit.
    pub router: QTensor,
    /// Kalibrierte Ausgangsskala der Router-Projektion.
    pub router_frac: u8,
    /// Alle Experten dieser Layer. **Alle liegen vor, es rechnen `top_k`.**
    /// Das ist der Zuschnitt, der die Pod-Kette unverändert lässt: Ein
    /// Knoten hält alle Experten seiner Layer, und weil je Layer und
    /// Token exakt `top_k` feuern, ist seine Arbeitsmenge eine Konstante
    /// aus der Modellkonfiguration statt einer Größe je Anfrage.
    pub experts: Vec<DenseMlp>,
    /// **Der geteilte Experte**, falls das Modell einen hat.
    ///
    /// ⚑ `None` bei jedem Gemisch vor dem `Qwen3.6-35B-A3B`.
    pub geteilter_experte: Option<GeteilterExperte>,
    /// Wie viele Experten je Token feuern (`num_experts_per_tok`).
    pub top_k: usize,
    /// Ob die Gewichte der gewählten Experten auf eins normiert werden
    /// (`norm_topk_prob`).
    pub norm_topk_prob: bool,
}

pub struct TransformerLayer {
    pub layer_idx: usize,
    /// Gamma der input_layernorm als QTensor: `data` + eigener kalibrierter
    /// Shift (vor v0.12.20 wurde der Shift verworfen, siehe Fund 1).
    pub input_layernorm_gamma: QTensor,
    pub post_attention_layernorm_gamma: QTensor,
    /// **Womit diese Ebene ueber die Folge mischt**: Achtsamkeit oder eine
    /// rekurrente Zustandsschicht.
    pub mischer: Mischer,
    /// Der Feedforward-Teil: eine dichte MLP oder ein Mixture-of-Experts-Modell.
    pub ffn: Feedforward,
    /// Kalibrierte Per-Layer-Aktivierungsskalen.
    pub scales: LayerScales,
}

/// **Womit eine Ebene ueber die Folge mischt.**
///
/// # ⚑ Warum ein Aufzaehlungstyp und nicht zwei `Option`-Felder
///
/// Dasselbe Argument wie bei [`Feedforward`], und hier noch zwingender:
/// Eine Zustandsebene hat **kein** `q_proj`. Ein zweites `Option`-Feld
/// neben den Achtsamkeitstensoren ginge also gar nicht, ohne auch diese
/// optional zu machen, und dann waeren „beides" und „keines" darstellbar.
///
/// ⛔️ **Beides waere ein Ladefehler, den erst der Vorwaertspass
/// bemerkt**, und zwar als eine falsche Zahl ohne Meldung.
///
/// ⚠️ **Das grosse Modell mischt beide Arten in einem Modell**: 30
/// rekurrente Ebenen und 10 mit voller Achtsamkeit, im Wechsel drei zu
/// eins. Ein Typ, der den Mischfall nicht ausdruecken kann, muesste beim
/// ersten solchen Modell umgebaut werden.
pub enum Mischer {
    /// Volle Achtsamkeit ueber den Schluessel-Wert-Speicher.
    Achtsamkeit(Achtsamkeit),
    /// Eine rekurrente Zustandsschicht (Gated DeltaNet).
    Zustand(Zustandsschicht),
}

/// Die Tensoren der vollen Achtsamkeit.
pub struct Achtsamkeit {
    pub q_proj: QTensor,
    pub k_proj: QTensor,
    pub v_proj: QTensor,
    pub o_proj: QTensor,
    /// Q/K/V-Attention-Biases (Qwen2.5 besitzt sie an q/k/v_proj). `None`
    /// bei Modellen ohne Attention-Biases (`attention_bias: false` in
    /// model_config.json); sonst je ein Bias-Tensor mit eigener Skala,
    /// der nach der Projektion addiert wird (siehe `add_bias_i16`).
    pub q_bias: Option<crate::loader::BiasTensor>,
    pub k_bias: Option<crate::loader::BiasTensor>,
    pub v_bias: Option<crate::loader::BiasTensor>,
    /// QK-Norm (Qwen3). `None` bei Modellen ohne (`qk_norm: false`).
    pub qk_norm: Option<QkNorm>,
}

/// **Die Masse und Tabellen der rekurrenten Zustandsschichten**, einmal
/// je Modell.
///
/// ⚑ **`Option` am Modell und nicht je Ebene.** Die Masse sind fuer alle
/// Zustandsebenen dieselben; sie je Ebene zu fuehren hiesse, dieselbe
/// Angabe dreissigmal abzulegen. Ein Modell ohne Zustandsebenen traegt
/// `None`.
pub struct Zustandsmasse {
    /// Wie viele Schluesselkoepfe, wie viele Wertkoepfe.
    ///
    /// ⚠️ **Sie sind verschieden**: Beim grossen Modell 16 gegen 32, und
    /// jeder Schluesselkopf bedient zwei Wertkoepfe.
    pub schluessel_koepfe: usize,
    pub wert_koepfe: usize,
    pub schluessel_dim: usize,
    pub wert_dim: usize,
    /// Die Kanalzahl der Faltung: `2 * schluessel_dim * schluessel_koepfe
    /// + wert_dim * wert_koepfe`.
    pub kanaele: usize,

    /// Der feine Teil des Softplus, `log(1 + exp(-|x|))`.
    pub softplus_rest: Vec<i32>,
    pub softplus_ein_frac: u8,
    pub softplus_aus_frac: u8,
    /// Der grobe Teil des Zerfalls, `exp(-d_grob)`.
    pub zerfall_exp: Vec<i32>,
    pub zerfall_raster_frac: u8,
    pub zerfall_aus_frac: u8,
}

impl Zustandsmasse {
    /// Wie viele Wertkoepfe auf einen Schluesselkopf kommen.
    ///
    /// ⚑ **Die Auffaecherung ist reine Indexierung**, kein Kopieren: Der
    /// Wertkopf `h` liest den Schluesselkopf `h / auffaecherung()`.
    pub fn auffaecherung(&self) -> usize {
        self.wert_koepfe / self.schluessel_koepfe
    }
}

/// Die Tensoren einer rekurrenten Zustandsschicht.
///
/// ⚑ **Neun Stueck, und sie heissen wie in den Gewichten**, damit beim
/// Nachsehen im Artefakt kein Uebersetzen noetig ist.
pub struct Zustandsschicht {
    /// Eine Projektion fuer `q`, `k` und `v` zusammen.
    pub in_proj_qkv: QTensor,
    /// Das Ausgangstor `z`.
    pub in_proj_z: QTensor,
    /// Woraus `beta` entsteht (ueber Sigmoid).
    pub in_proj_b: QTensor,
    /// Woraus der Zerfall entsteht (ueber Softplus und `exp_A`).
    pub in_proj_a: QTensor,
    /// Die tiefenweise Faltung, `kanaele * KERN` Gewichte.
    pub conv1d: QTensor,
    /// `exp(A_log)`, **zur Exportzeit gerechnet**, je Wertkopf einer.
    pub exp_a: crate::loader::BiasTensor,
    /// Der Versatz vor dem Softplus, je Wertkopf einer.
    pub dt_bias: crate::loader::BiasTensor,
    /// Gamma der torgesteuerten Norm.
    pub norm_gamma: QTensor,
    pub out_proj: QTensor,
    /// ⚑ **Die Skalen wohnen bei den Tensoren, zu denen sie gehoeren.**
    pub skalen: Zustandsskalen,
}

impl TransformerLayer {
    /// Die Achtsamkeitstensoren dieser Ebene.
    ///
    /// ⚠️ **Bricht ab, wenn die Ebene rekurrent mischt.** Das ist
    /// Absicht: Ein Aufrufer, der hier landet, hat die Ebenenart nicht
    /// geprueft, und eine stillschweigende Ersatzantwort waere eine
    /// falsche Zahl ohne Meldung.
    pub fn achtsamkeit(&self) -> &Achtsamkeit {
        match &self.mischer {
            Mischer::Achtsamkeit(a) => a,
            Mischer::Zustand(_) => panic!(
                "Ebene {} mischt rekurrent und hat keine Achtsamkeitstensoren; \
                 der Aufrufer prueft die Ebenenart nicht",
                self.layer_idx
            ),
        }
    }

    /// Die Zustandstensoren dieser Ebene, oder `None` bei Achtsamkeit.
    ///
    /// ⚑ **Hier `Option` und oben ein Abbruch**, mit Absicht: Wer die
    /// Zustandsschicht sucht, fragt danach; wer die Achtsamkeit nimmt,
    /// setzt sie voraus, und diese Voraussetzung soll knallen.
    pub fn zustandsschicht(&self) -> Option<&Zustandsschicht> {
        match &self.mischer {
            Mischer::Zustand(z) => Some(z),
            Mischer::Achtsamkeit(_) => None,
        }
    }

    /// Mischt diese Ebene rekurrent?
    pub fn ist_rekurrent(&self) -> bool {
        matches!(self.mischer, Mischer::Zustand(_))
    }
}

/// QK-Norm einer Layer: die beiden Gammas und ihre Ausgangsskalen.
///
/// **Warum ein eigener Typ und nicht vier Felder.** Vier `Option`-Felder
/// koennten halb besetzt sein: Gamma ohne Skala, oder Q ohne K. Beides
/// waere ein Ladefehler, den erst der Forward-Pass bemerkt, und dort
/// bemerkt er ihn als falsche Zahlen und nicht als Fehler. Als ein Typ
/// gibt es entweder alles oder nichts, und der Loader muss die
/// Vollstaendigkeit nicht pruefen, weil sie sich nicht ausdruecken laesst.
/// Dieselbe Ueberlegung wie bei `RebuildAnlass` in `myl-pod`.
pub struct QkNorm {
    /// Gamma fuer die Query-Normierung, Laenge `head_dim`. Alle Koepfe
    /// teilen sich dasselbe Gamma.
    pub q_gamma: QTensor,
    /// Gamma fuer die Key-Normierung, Laenge `head_dim`.
    pub k_gamma: QTensor,
    /// Ausgangsskala der Query-Normierung. Sie loest `q_frac` als
    /// Eingangsskala von RoPE und Attention ab.
    pub q_out_frac: u8,
    /// Ausgangsskala der Key-Normierung.
    pub k_out_frac: u8,
}

/// Das komplette Modell.
pub struct IntegerModel {
    pub theta_v: ThetaV,
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub num_layers: usize,
    pub num_heads: usize,
    /// Anzahl der Key/Value-Heads bei Grouped-Query-Attention (GQA).
    /// `num_heads` muss ein Vielfaches von `num_kv_heads` sein; je
    /// `num_heads / num_kv_heads` aufeinanderfolgende Query-Heads teilen sich
    /// einen KV-Head (Qwen2.5-0.5B: 14 Query-Heads, 2 KV-Heads).
    pub num_kv_heads: usize,
    pub head_dim: usize,
    /// **Wie viele Stellen eines Kopfvektors gedreht werden.**
    ///
    /// ⚑ **Meist `head_dim`, aber nicht immer.** Das `Qwen3.6-35B-A3B`
    /// dreht nur 64 von 256 Stellen (`partial_rotary_factor` 0,25); der
    /// Rest geht unveraendert durch.
    ///
    /// ⛔️ **Eine Teildrehung ist nicht „dieselbe Drehung, nur
    /// kuerzer".** Die Frequenzen haengen an dieser Breite:
    /// `theta_j = 1 / base^(j / (drehbreite/2))`. Mit `head_dim/2` im
    /// Nenner waeren sie schlicht falsch, und das faellt nur an der
    /// Qualitaet auf, nie an einer Meldung.
    pub drehbreite: usize,
    pub max_context: usize,
    /// Das Epsilon der RMSNorm als `round(eps * 2^40)` (Fund 419).
    ///
    /// ⚑ Es traegt **nur** in der torgesteuerten Norm des rekurrenten
    /// Zweigs; ueberall sonst ist `mean(x^2)` von der Groessenordnung
    /// eins, und dort aendert es nichts.
    pub norm_eps_q40: i64,
    pub embedding_table: QTensor,   // [vocab_size, hidden_size]
    pub lm_head: QTensor,           // [vocab_size, hidden_size] (oder mit embedding_table getied)
    /// INT16-LM-Head mit Per-Channel-Skalen (benannte spec-Ausnahme 0.6.0,
    /// Eskalation nach dem Entscheidungspunkt 12.21). Falls vorhanden, wird
    /// er für die Logits verwendet; `lm_head` (int8, getied) dient dann nur
    /// noch als Fallback-Pfad für ältere Artefakte.
    pub lm_head_int16: Option<crate::loader::LmHead>,
    /// Gamma der finalen RMSNorm als QTensor (data + kalibrierter Shift).
    pub final_norm_gamma: QTensor,
    /// Kalibrierte Skala des finalen Norm-Ausgangs = Eingang des LM-Heads.
    pub final_norm_frac: u8,
    /// Kalibrierte Skala des letzten Residualstrom-Segments (Eingang von
    /// model.norm; Per-Segment-Skalen seit spec 0.5.1, Per-Kanal seit
    /// Fund 20 / theta_v 0.11.0).
    pub final_residual_frac: Vec<u8>,
    pub layers: Vec<TransformerLayer>,
    pub cos_lut: Vec<i16>,
    pub sin_lut: Vec<i16>,
    pub exp_lut: Vec<i16>,
    pub silu_lut: Vec<i16>,
    /// rsqrt-LUT (spec: rsqrt.method = "lut"), konsumiert von `rmsnorm_i16`
    /// mit dynamischem geradem Index-Shift.
    pub rsqrt_lut: Vec<i16>,
    /// Reziproken-Konstante 2^20/hidden_size fuer den divisionsfreien
    /// Mittelwert in `rmsnorm_i16` (einmalige Initialisierung).
    pub inv_n_q20: i64,
    /// Kalibrierte Aktivierungsskalen aus scales.json (vollstaendig
    /// validiert und in den Forward-Pass verdrahtet, v0.12.20).
    pub activation_scales: LoadedScales,
    pub config: ModelConfig,
    /// **Die Masse und Tabellen der rekurrenten Ebenen**, oder `None`
    /// bei einem Modell ohne solche.
    pub zustandsmasse: Option<Zustandsmasse>,
    /// **Sigmoid**, fuer jedes Tor im Modell.
    ///
    /// ⚑ **Am Modell und nicht bei der Zustandsschicht**, denn sie wird
    /// an drei Stellen gebraucht: der Schreibstaerke `beta`, dem Tor am
    /// Achtsamkeitsausgang und dem Tor des geteilten Experten. Sie lag
    /// bis zum 2026-09-21 in der Zustandsmasse, und ein Modell mit
    /// torgesteuerter Achtsamkeit **ohne** rekurrente Ebenen haette sie
    /// dort nicht gefunden.
    pub sigmoid_lut: Vec<i16>,
    pub sigmoid_versatz: i16,
    pub sigmoid_ein_frac: u8,
    pub sigmoid_aus_frac: u8,
    /// **Traegt die volle Achtsamkeit ein Tor am Ausgang?**
    ///
    /// ⛔️ Beim `Qwen3.6-35B-A3B` liefert `q_proj` die doppelte
    /// Kopfbreite, und der Ausgang wird vor `o_proj` mit `sigmoid(tor)`
    /// multipliziert.
    pub achtsamkeit_mit_tor: bool,
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Skala des KV-Cache (spec: kv_cache, frac 8). K/V werden beim
    /// Schreiben/Lesen zwischen ihrer Per-Layer-Skala und dieser Skala
    /// umgerechnet.
    /// Historisch: bis theta_v 0.11.0 wurde der KV-Cache auf diese
    /// GLOBALE Skala umgerechnet und beim Lesen zurueck. Seit Fund 22
    /// (theta_v 0.12.0) haelt der Cache K/V in der nativen Per-Layer-Skala
    /// der erzeugenden Projektion — die Umrechnung war ein reiner Verlust
    /// (Quelle und Ziel identisch), kostete 2-4 Bit auf fast jeder Ebene
    /// und clippte bei Qwen2.5-7B in Ebene 0 um Faktor 3,28.
    ///
    /// Das Feld bleibt erhalten, weil es im Artefaktformat steht und von
    /// Diagnosen ausgegeben wird; der Inferenzpfad liest es nicht mehr.
    pub kv_cache_frac_bits: u8,
    /// Skala der Q*K-Scores NACH dem Rescale, bevor sie in die exp-LUT
    /// indizieren. Muss mit dem Kalibrierungsbereich der LUT
    /// uebereinstimmen (spec: softmax.exp_lut_frac_bits).
    pub score_frac_bits: u8,
    /// Eingangsskala der exp-LUT (spec: softmax.exp_input_frac_bits):
    /// Index i der LUT steht fuer den Score-Differenz-Realwert i * 2^-Wert.
    /// Der lut_shift der Attention ist score_frac_bits - exp_input_frac_bits.
    pub exp_input_frac: u8,
    pub prob_frac_bits: u8,
    pub rope_frac_bits: u8,
    /// Feste Eingangsskala der SiLU-LUT (spec: silu.input_frac_bits);
    /// Gate-Werte werden vor dem Lookup dorthin reskaliert.
    pub silu_in_frac: u8,
    /// Index-Offset der SiLU-LUT = -input_min (spec: silu.input_range).
    pub silu_lut_offset: i16,
    /// Ausgangsskala der SiLU-LUT (spec: silu.output_frac_bits).
    pub silu_out_frac: u8,
    /// Parameter der rsqrt-LUT (spec: rsqrt.input_shift / output_frac_bits).
    pub rsqrt_input_shift: u8,
    pub rsqrt_output_frac: u8,
    /// Skala der Logits (nur fuer Sampling/Argmax; gemeinsame Skala reicht,
    /// da beide skaleninvariant sind).
    pub logit_frac_bits: u8,
}

impl Default for ModelConfig {
    fn default() -> Self {
        // Fallback-Konstanten fuer Tests ohne spec-Parsing; der reale
        // Modellbau (build_model) liest die Werte aus der eingebetteten
        // theta_v/spec.json.
        ModelConfig {
            kv_cache_frac_bits: 8,
            score_frac_bits: 8,
            exp_input_frac: 4,
            prob_frac_bits: 8,
            rope_frac_bits: 8,
            silu_in_frac: 3,
            silu_lut_offset: 1024,
            silu_out_frac: 6,
            rsqrt_input_shift: 8,
            rsqrt_output_frac: 8,
            logit_frac_bits: 6,
        }
    }
}

impl IntegerModel {
    /// Wie [`IntegerModel::forward_token`], aber sammelt je MoE-Layer die
    /// Routing-Entscheidung.
    ///
    /// **Für Messungen, nicht für den Betrieb.** Sie rechnet dasselbe wie
    /// `forward_token`, kostet aber je MoE-Layer einen zweiten Durchgang
    /// über die Router-Logits, um die Gleichstände an der Auswahlgrenze
    /// zu zählen. Bei einem dichten Modell bleibt die Liste leer.
    ///
    /// Der Grund für den eigenen Einstieg statt eines Schalters am
    /// Modell: Ein Zähler im Modell müsste `Sync` sein, weil `myl-pod`
    /// die Shards als `Arc` über Threads teilt (Fund A17). Ein
    /// durchgereichter Sammler braucht das nicht.
    pub fn forward_token_mit_routing(
        &self,
        token_id: usize,
        pos: usize,
        cache: &mut KVCache,
    ) -> (Vec<i32>, Vec<Routingbefund>) {
        let first_residual_frac = &self.layers[0].scales.residual_in_frac;
        let emb = self.embedding_table.row(token_id);
        let emb_shift = self.embedding_table.shifts[token_id];
        let mut hidden: Vec<i16> = emb
            .iter()
            .enumerate()
            .map(|(i, v)| clamp_i16(rescale(*v as i32, emb_shift, first_residual_frac[i])))
            .collect();

        let mut befunde = Vec::new();
        for (i, layer) in self.layers.iter().enumerate() {
            let out_frac: &[u8] = if i + 1 < self.layers.len() {
                &self.layers[i + 1].scales.residual_in_frac
            } else {
                &self.final_residual_frac
            };
            hidden = self.forward_layer(
                layer, &hidden, pos, cache, out_frac, Some(&mut befunde), None,
            );
        }

        // ⚑ **`head_logits` normiert selbst** (Fund 176). Bis zum
        // 2026-09-04 stand hier eine eigene Normierung davor, und der
        // Kopf bekam einen bereits normierten Strom, den er ein zweites
        // Mal normierte, dazu auf der falschen Eingangsskala. Der
        // Rechenpfad selbst war nie betroffen: `forward_token` schreibt
        // Normierung und Kopf aus, und der Shard ruft `head_logits` mit
        // dem rohen Residualstrom. **Betroffen war der Messpfad**, also
        // die Zahlen, die eine Routing-Untersuchung liefert.
        (self.head_logits(&hidden), befunde)
    }

    /// **Wie viele Positionen dieses Modell rechnen kann**: das Kleinere aus
    /// `max_context` und den Zeilen der RoPE-Tabelle.
    ///
    /// # ⛔️ Fund 368 (2026-09-14): hinter der Tabelle begann sie von vorn
    ///
    /// `koepfe_drehen` las die Tabellenzeile `pos % Zeilen`. Die Tabelle
    /// hat 2 048 Zeilen, und Position 2 048 bekam den Winkel von Position
    /// null: **Die Aufmerksamkeit hielt ein Token hinter der Grenze fuer
    /// eines vom Anfang**, ohne Meldung. Gemessen am 4B mit einem Prompt von
    /// 4 393 Token: Die Ausgabe war zusammenhangloser Text.
    ///
    /// ⚑ **Jetzt gilt die Grenze und wird geprueft**: Die Erzeugung haelt an
    /// ihr an, ein Prompt darueber ist ein Fehler des Aufrufers, und ein
    /// Shard lehnt eine solche Position ab, bevor er rechnet.
    pub fn kontextgrenze(&self) -> usize {
        // ⛔️ **Hier stand bis zum 2026-09-22 `head_dim / 2`**, und das
        // ist die falsche Breite, sobald eine Ebene nur teilweise dreht.
        // Beim Qwen3.6-35B-A3B ist `head_dim` 256 und die Drehbreite 64;
        // die Tabellenzeile ist also 32 breit, nicht 128, und diese
        // Rechnung meldete 10 240 statt 40 960 Positionen.
        //
        // ⚠️ **Sie unterschaetzte, griff also nicht ueber den Rand.**
        // Genau das machte sie harmlos und damit unsichtbar.
        //
        // 📌 **Zwei Orte, dieselbe Groesse, verschiedene Antwort.**
        // `koepfe_drehen` schneidet die Zeile mit `drehbreite / 2`; wer
        // hier anders rechnet, widerspricht ihr.
        let half = (self.drehbreite / 2).max(1);
        let zeilen = (self.cos_lut.len() / half).min(self.sin_lut.len() / half);
        zeilen.min(self.max_context)
    }

    /// Einzelner Forward-Schritt fuer ein Token an Position `pos`.
    /// KV-Cache wird gelesen und geschrieben.
    pub fn forward_token(
        &self,
        token_id: usize,
        pos: usize,
        cache: &mut KVCache,
    ) -> Vec<i32> {
        let cfg = &self.config;
        let hidden = self.durch_die_ebenen(token_id, pos, cache);

        // 3. Final RMSNorm (int16 -> int16 auf der kalibrierten
        //    final-norm-Skala; LUT-gestuetzt, divisionsfrei).
        let normed = rmsnorm_i16(
            &hidden,
            &self.final_residual_frac,
            &self.final_norm_gamma.data,
            &self.final_norm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            self.final_norm_frac,
        );

        // 4. LM-Kopf, siehe `logits_aus_normiertem`.
        self.logits_aus_normiertem(&normed, cfg.logit_frac_bits)
    }

    /// **Der MLP-Teil fuer mehrere Token zugleich**, dicht oder als
    /// Expertengemisch.
    fn ebene_mlp_stapel(
        &self,
        layer: &TransformerLayer,
        normen: &[&[i16]],
        acc_mlp: &[u8],
    ) -> Vec<Vec<i16>> {
        let sc = &layer.scales;
        let cfg = &self.config;
        let mlp = match &layer.ffn {
            Feedforward::Dense(mlp) => mlp,
            Feedforward::Moe(moe) => return self.moe_stapel(moe, normen, sc, cfg, acc_mlp),
        };
        integer_llm_kernels::mlp::mlp_int_stapel(
            normen,
            &mlp.gate_proj.data,
            &mlp.up_proj.data,
            &mlp.down_proj.data,
            mlp.gate_proj.cols(),
            mlp.down_proj.cols(),
            &mlp.gate_proj.shifts,
            &mlp.up_proj.shifts,
            &mlp.down_proj.shifts,
            &self.silu_lut,
            sc.norm_mlp_frac,
            sc.gate_frac,
            sc.up_frac,
            sc.down_in_frac,
            cfg.silu_in_frac,
            cfg.silu_lut_offset,
            cfg.silu_out_frac,
            acc_mlp,
        )
    }

    /// **Eine Gemischebene fuer viele Token: die Token je Experte
    /// gruppiert.**
    ///
    /// # 📌 Fund 371 (2026-09-14): die Experten liefen Token fuer Token
    ///
    /// Fund 335 hatte diese Gruppierung am 2026-09-11 gebaut, gemessen und
    /// verworfen: auf der CPU kein Gewinn, **der Posten sei die
    /// Aufmerksamkeit**. Das stimmte an jenem Tag. Seit die
    /// Aufmerksamkeitshaelfte gebuendelt und verteilt rechnet (Fund 366),
    /// **liegen beim 30B rund neun Zehntel des Hauptfadens in den
    /// Experten**, gemessen an 992 Token mit `metal`. Und auf der GPU ist die
    /// Gruppierung nicht neutral, sondern die Voraussetzung: Erst sie macht
    /// aus einer Eingabe je Aufruf ein Buendel.
    ///
    /// ⚑ **Dieselbe Rechnung wie `moe_vorwaerts` je Token**, nur in anderer
    /// Reihenfolge: derselbe Router, dieselbe Auswahl, fuer jedes gewaehlte
    /// Paar aus Token und Experte dasselbe gate, up, SiLU-Produkt und down,
    /// dieselbe Mischung in der Reihenfolge der Auswahl. Geprueft gegen den
    /// tokenweisen Weg in `die_gebuendelte_vorbereitung_rechnet_dasselbe`
    /// (Fixture mit Gemisch und Zufallsgewichten) und ueber die Stufe
    /// `rechenwege` am echten Modell.
    ///
    /// ⚑ **Alle Expertenmatrizen einer Runde in einem Auftrag**
    /// (`linear_w8a16_stapel_viele`): auf der GPU ein Befehlspuffer, auf der
    /// CPU eine Poolrunde. Einzeln waeren es 384 Aufrufe je Ebene.
    fn moe_stapel(
        &self,
        moe: &MoeLayer,
        normen: &[&[i16]],
        sc: &LayerScales,
        cfg: &ModelConfig,
        acc: &[u8],
    ) -> Vec<Vec<i16>> {
        use integer_llm_kernels::linear::{linear_w8a16_stapel_viele, Ausgangsskala, Stapelauftrag};
        let n = normen.len();
        if n == 0 {
            return Vec::new();
        }
        let faeden = vorbereitungsfaeden(n);

        // 1. Der Router fuer alle Token, dann die Auswahl je Token; dieselbe
        //    Rechnung wie `moe_routing`.
        let logits = linear_fuer_alle(
            normen,
            &moe.router.data,
            moe.router.cols(),
            &moe.router.shifts,
            sc.norm_mlp_frac,
            moe.router_frac,
        );
        let exp_lut_shift = moe.router_frac.saturating_sub(cfg.exp_input_frac);
        let auswahl: Vec<integer_llm_kernels::moe::Routing> = logits
            .iter()
            .map(|l| {
                let l32: Vec<i32> = l.iter().map(|v| *v as i32).collect();
                route_top_k(&l32, moe.top_k, &self.exp_lut, exp_lut_shift, cfg.prob_frac_bits, moe.norm_topk_prob)
            })
            .collect();

        // 2. Je Experte seine Token, als (Token, Platz in dessen Auswahl).
        let mut je_experte: Vec<Vec<(usize, usize)>> = vec![Vec::new(); moe.experts.len()];
        for (b, r) in auswahl.iter().enumerate() {
            for (platz, e) in r.experten.iter().enumerate() {
                je_experte[*e as usize].push((b, platz));
            }
        }
        let benutzt: Vec<usize> = (0..moe.experts.len()).filter(|e| !je_experte[*e].is_empty()).collect();
        if benutzt.is_empty() {
            return vec![vec![0i16; acc.len()]; n];
        }
        // ⚑ **Einmal je Experte und Ebene angekuendigt**, nicht einmal je
        // Token (Fund 331), **und parallel**. Siehe
        // [`ANKUENDIGUNGSFAEDEN`]: Die Vorbereitung eines Gemischs auf
        // 24 GiB wartet sonst mehr auf die Platte, als sie rechnet.
        let je = benutzt.len().div_ceil(ANKUENDIGUNGSFAEDEN);
        std::thread::scope(|s| {
            for teil in benutzt.chunks(je) {
                s.spawn(move || {
                    for e in teil {
                        moe.experts[*e].vorbereiten();
                    }
                });
            }
        });
        let eingaben: Vec<Vec<&[i16]>> = benutzt
            .iter()
            .map(|e| je_experte[*e].iter().map(|(b, _)| normen[*b]).collect())
            .collect();

        // 3. gate und up aller benutzten Experten in einem Auftrag.
        let mut vorne: Vec<Stapelauftrag<'_>> = Vec::with_capacity(2 * benutzt.len());
        for (i, e) in benutzt.iter().enumerate() {
            let ex = &moe.experts[*e];
            vorne.push(Stapelauftrag {
                xs: &eingaben[i],
                w: &ex.gate_proj.data,
                in_features: ex.gate_proj.cols(),
                w_shifts: &ex.gate_proj.shifts,
                act_frac_bits: sc.norm_mlp_frac,
                aus: Ausgangsskala::Eine(sc.gate_frac),
            });
            vorne.push(Stapelauftrag {
                xs: &eingaben[i],
                w: &ex.up_proj.data,
                in_features: ex.up_proj.cols(),
                w_shifts: &ex.up_proj.shifts,
                act_frac_bits: sc.norm_mlp_frac,
                aus: Ausgangsskala::Eine(sc.up_frac),
            });
        }
        let vorne_aus = linear_w8a16_stapel_viele(&vorne);
        drop(vorne);

        // 4. Das SiLU-Produkt je Paar aus Experte und Token, verteilt.
        let breite = moe.experts[benutzt[0]].down_proj.cols();
        let paare: Vec<(usize, usize)> = (0..benutzt.len())
            .flat_map(|i| (0..eingaben[i].len()).map(move |j| (i, j)))
            .collect();
        let produkte = rechnen_breit(paare.len(), breite, faeden, |p, ziel| {
            let (i, j) = paare[p];
            ziel.copy_from_slice(&integer_llm_kernels::mlp::silu_produkt(
                &vorne_aus[2 * i][j],
                &vorne_aus[2 * i + 1][j],
                &self.silu_lut,
                sc.gate_frac,
                sc.up_frac,
                sc.down_in_frac,
                cfg.silu_in_frac,
                cfg.silu_lut_offset,
                cfg.silu_out_frac,
            ));
        });
        drop(vorne_aus);
        let mut hs: Vec<Vec<&[i16]>> = benutzt.iter().map(|_| Vec::new()).collect();
        for (p, (i, _)) in paare.iter().enumerate() {
            hs[*i].push(&produkte[p * breite..(p + 1) * breite]);
        }

        // 5. down aller benutzten Experten in einem Auftrag.
        let hinten: Vec<Stapelauftrag<'_>> = benutzt
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let ex = &moe.experts[*e];
                Stapelauftrag {
                    xs: &hs[i],
                    w: &ex.down_proj.data,
                    in_features: ex.down_proj.cols(),
                    w_shifts: &ex.down_proj.shifts,
                    act_frac_bits: sc.down_in_frac,
                    aus: Ausgangsskala::JeZeile(acc),
                }
            })
            .collect();
        let hinten_aus = linear_w8a16_stapel_viele(&hinten);

        // 6. Je Token die Ausgaben in der Reihenfolge seiner Auswahl, dann
        //    die Mischung wie `mische_experten`.
        let mut plaetze: Vec<Vec<Option<(usize, usize)>>> =
            auswahl.iter().map(|r| vec![None; r.experten.len()]).collect();
        for (i, e) in benutzt.iter().enumerate() {
            for (j, (b, platz)) in je_experte[*e].iter().enumerate() {
                plaetze[*b][*platz] = Some((i, j));
            }
        }
        let hidden = acc.len();
        rechnen_breit(n, hidden, faeden, |b, ziel| {
            let ausgaben: Vec<Vec<i16>> = plaetze[b]
                .iter()
                .map(|p| {
                    let (i, j) = p.expect("jeder Platz einer Auswahl ist einem Experten zugeordnet");
                    hinten_aus[i][j].clone()
                })
                .collect();
            ziel.copy_from_slice(&mische_experten(&ausgaben, &auswahl[b].gewichte, cfg.prob_frac_bits));
        })
        .chunks_exact(hidden)
        .map(<[i16]>::to_vec)
        .collect::<Vec<Vec<i16>>>()
        .into_iter()
        .zip(normen.iter())
        // ⛔️ **Der geteilte Experte, dieselbe Rechnung wie einzeln**
        //   (Fund 427). Bis zum 2026-09-22 endete der gebuendelte Weg
        //   eine Zeile frueher, und der geteilte Experte fehlte auf
        //   **jeder** Gemischebene und bei **jedem** Token.
        .map(|(gemischt, x)| self.geteilten_experten_addieren(moe, x, gemischt, sc, cfg, acc))
        .collect()
    }

    /// **Die Vorbereitung ebenenweise statt tokenweise.**
    ///
    /// Zurueck kommt der Residualstrom jedes Tokens nach der letzten Ebene,
    /// also dasselbe, was [`Model::durch_die_ebenen`] je Token liefert.
    /// 📌 **Er wird fuer die Pruefungen gebraucht** (Fund 366): Die
    /// Aufmerksamkeit der vorbereiteten Token wirkt erst in der naechsten
    /// Ebene, und ein Testmodell mit einer Ebene zeigt einen Fehler dort
    /// weder im KV-Speicher noch in den Logits.
    ///
    /// # ⚑ Dieselbe Rechnung, andere Reihenfolge
    ///
    /// Tokenweise laeuft Token 0 durch alle 36 Ebenen, dann Token 1, und
    /// so fort. Ebenenweise laufen **alle** Token durch Ebene 0, dann
    /// alle durch Ebene 1. **Jede einzelne Rechnung ist dieselbe**, und
    /// zwar in demselben Zustand:
    ///
    /// - Der KV-Speicher wird in **derselben Reihenfolge** gefuellt:
    ///   Ebene `l` bekommt die Token 0..B nacheinander, genau wie
    ///   vorher.
    /// - Die Aufmerksamkeit von Token `b` in Ebene `l` liest die
    ///   Positionen 0..pos+b **derselben** Ebene, und die stehen dort,
    ///   weil die Token 0..b-1 diese Ebene gerade durchlaufen haben.
    /// - Der Residualstrom jedes Tokens haengt nur an ihm selbst.
    ///
    /// ⚑ **Und `forward_layer` bleibt unberuehrt.** Es gibt weiterhin
    /// **eine** Umsetzung des Vorwaertspasses; hier steht nur eine
    /// andere Schleifenreihenfolge darum. Eine zweite Umsetzung waere
    /// eine zweite Wahrheit ueber den Rechenpfad.
    ///
    /// # ⚠️ Warum das ueberhaupt etwas bringen kann
    ///
    /// Die Zahl der gelesenen Gewichtsbytes aendert sich **nicht**.
    /// Was sich aendert, ist die Naehe: Ebenenweise liegen die B
    /// Zugriffe auf dieselbe Matrix unmittelbar hintereinander, und was
    /// in den Zwischenspeicher passt, wird beim zweiten Token nicht
    /// wieder aus dem Hauptspeicher geholt.
    ///
    /// ⚠️ **Ob das traegt, ist eine Messfrage und keine Herleitung.**
    /// Gemessen am 2026-09-11 gegen `myelith-4b` mit 169 Token: allein
    /// die Umstellung der Reihenfolge brachte rund 4 %, die Buendelung
    /// des MLP darauf noch einmal 5 %. **Der grosse Posten lag
    /// woanders**, naemlich im KV-Verlauf, der je Kopf und je Token
    /// kopiert wurde.
    pub fn vorbereiten_stapel(
        &self,
        token_ids: &[usize],
        pos_start: usize,
        cache: &mut KVCache,
    ) -> Vec<Vec<i16>> {
        if token_ids.is_empty() {
            return Vec::new();
        }
        let mut zustaende: Vec<Vec<i16>> =
            token_ids.iter().map(|&t| self.embed_token(t)).collect();
        for (i, layer) in self.layers.iter().enumerate() {
            // ⛔️ **Eine Rekurrenz laesst sich nicht buendeln.**
            //
            // Der gebuendelte Vorlauf rechnet viele Token auf einmal und
            // liest die Gewichte je Ebene einmal statt einmal je Token.
            // Das traegt, weil die Achtsamkeit jedes Token unabhaengig
            // projiziert. **Der Zustand bei Token t haengt am Zustand bei
            // t-1**, dort gibt es nichts zu buendeln.
            //
            // ⚑ **Deshalb laeuft eine Zustandsebene hier Token fuer Token
            // durch DENSELBEN Weg wie beim Dekodieren.** Kein zweiter
            // Rumpf: Was gebuendelt nicht geht, wird nicht nachgebaut,
            // sondern in der Schleife gerechnet. Genau hier entstuenden
            // sonst zwei Fassungen derselben Rechnung.
            //
            // ⚠️ **Der Vorlauf verliert fuer diese Ebenen seinen
            // Vorteil.** Beim grossen Modell sind das 30 von 40 Ebenen;
            // was das kostet, ist noch nicht gemessen.
            if layer.ist_rekurrent() {
                let out_frac: &[u8] = if i + 1 < self.layers.len() {
                    &self.layers[i + 1].scales.residual_in_frac
                } else {
                    &self.final_residual_frac
                };
                for b in 0..zustaende.len() {
                    zustaende[b] = self.forward_layer(
                        layer,
                        &zustaende[b],
                        pos_start + b,
                        cache,
                        out_frac,
                        None,
                        None,
                    );
                }
                continue;
            }
            let out_frac: &[u8] = if i + 1 < self.layers.len() {
                &self.layers[i + 1].scales.residual_in_frac
            } else {
                &self.final_residual_frac
            };
            let sc = &layer.scales;
            let acc_mlp: Vec<u8> = sc
                .residual_mid_frac
                .iter()
                .zip(out_frac.iter())
                .map(|(&a, &b)| a.min(b))
                .collect();

            // ⚑ **Erst der Aufmerksamkeitsteil, in fuenf Schritten.** Was
            // nur an einer Position haengt, laeuft fuer alle Token zugleich:
            // Normen, Koepfe und Residuen verteilt ueber die Faeden, die
            // Projektionen q, k, v und o als Buendel (Fund 366). **Der
            // KV-Speicher wird in der Reihenfolge der Positionen gefuellt**,
            // und erst danach lesen alle Token gleichzeitig: Token `b` liest
            // nur die Positionen bis zu seiner eigenen, und die stehen dann
            // genau so da wie tokenweise. `ebene_bis_mlp` ruft dieselben
            // Schritte mit einer Eingabe.
            //
            // ⚠️ **Keiner der verteilten Schritte darf selbst den
            // Fadenpool rufen**: Er nimmt eine Runde zur Zeit, und eine
            // Runde in einer Runde wartete auf sich selbst. Die Matrizen
            // laufen deshalb ausserhalb, zwischen den verteilten Schritten.
            let n = zustaende.len();
            let hs = self.hidden_size;
            let (nh, nkv, hd) = (self.num_heads, self.num_kv_heads, self.head_dim);
            let faeden = vorbereitungsfaeden(n);

            let normen = rechnen_breit(n, hs, faeden, |b, ziel| {
                ziel.copy_from_slice(&self.norm_vor_aufmerksamkeit(layer, &zustaende[b]));
            });
            let normen: Vec<&[i16]> = normen.chunks_exact(hs).collect();
            let qkv = self.projektionen_qkv(layer, &normen);

            // ⛔️ **Fund 426: der gebuendelte Weg kannte das Ausgangstor
            //   nicht.**
            //
            // `q_proj` liefert bei dieser Bauart die **doppelte**
            // Kopfbreite, Abfrage und Tor hintereinander. Der Einzelweg
            // trennt sie und multipliziert am Ende mit `sigmoid(tor)`;
            // hier stand beides nicht. Der rohe Ausgang ging als
            // Abfrage in die Drehung, und das Tor fiel weg.
            //
            // ⚠️ **Es brach nichts.** Die Breite passte zufaellig zur
            // Schleife, die Koepfe wurden nur falsch belegt. Gemessen
            // schlug es erst in der Perplexitaet durch: 10^6 auf dem
            // gebuendelten Weg gegen richtige Logits auf dem einzelnen.
            //
            // 📌 **Die Warnung stand eine Ebene hoeher im selben
            // Rumpf**: „Was gebuendelt nicht geht, wird nicht nachgebaut
            // ... Genau hier entstuenden sonst zwei Fassungen derselben
            // Rechnung." Fuer die Zustandsebenen wurde sie befolgt, fuer
            // die Achtsamkeit nicht, und die zweite Fassung ist dann
            // beim naechsten Modell auseinandergelaufen.
            let (qkv, tore): (Vec<[Vec<i16>; 3]>, Vec<Option<Vec<i16>>>) =
                if self.achtsamkeit_mit_tor {
                    let mut ohne_tor = Vec::with_capacity(qkv.len());
                    let mut tore = Vec::with_capacity(qkv.len());
                    for [q_roh, k, v] in qkv {
                        let (q, g) =
                            abfrage_und_tor_trennen(&q_roh, self.num_heads, self.head_dim);
                        ohne_tor.push([q, k, v]);
                        tore.push(Some(g));
                    }
                    (ohne_tor, tore)
                } else {
                    let leer = vec![None; qkv.len()];
                    (qkv, leer)
                };

            // Je Token: q-Koepfe, k-Koepfe, v-Koepfe hintereinander.
            let breite = (nh + 2 * nkv) * hd;
            let koepfe = rechnen_breit(n, breite, faeden, |b, ziel| {
                let [q, k, v] = &qkv[b];
                let (q_heads, k_heads, v_heads) = self.koepfe_drehen(layer, q, k, v, pos_start + b);
                for (teil, kopf) in ziel
                    .chunks_exact_mut(hd)
                    .zip(q_heads.iter().chain(k_heads.iter()).chain(v_heads.iter()))
                {
                    teil.copy_from_slice(kopf);
                }
            });
            drop(qkv);
            let zeile = |b: usize| -> Vec<Vec<i16>> {
                koepfe[b * breite..(b + 1) * breite].chunks_exact(hd).map(<[i16]>::to_vec).collect()
            };
            for b in 0..n {
                let k_und_v = zeile(b);
                self.kv_schreiben(layer, &k_und_v[nh..nh + nkv], &k_und_v[nh + nkv..], pos_start + b, cache);
            }
            let lesend: &KVCache = cache;
            let attn_aus = rechnen_breit(n, nh * hd, faeden, |b, ziel| {
                let mut q_heads = zeile(b);
                q_heads.truncate(nh);
                ziel.copy_from_slice(&self.aufmerksamkeit_lesen(layer, &q_heads, pos_start + b, lesend, None));
            });
            drop(koepfe);

            let acc_attn = akkumulationsskala_mischer(sc);
            // ⚑ **Das Tor wirkt VOR `o_proj`**, wortgleich zum
            //   Einzelweg (Fund 426).
            let getort: Option<Vec<Vec<i16>>> = if self.achtsamkeit_mit_tor {
                Some(
                    attn_aus
                        .chunks_exact(nh * hd)
                        .zip(tore.iter())
                        .map(|(aus, tor)| {
                            let g = tor.as_ref().expect("mit Tor: je Token eines");
                            self.tor_anwenden(aus, g, layer)
                        })
                        .collect(),
                )
            } else {
                None
            };
            let attn_scheiben: Vec<&[i16]> = match &getort {
                Some(g) => g.iter().map(Vec::as_slice).collect(),
                None => attn_aus.chunks_exact(nh * hd).collect(),
            };
            let o_aus = self.projektion_o(layer, &attn_scheiben, &acc_attn);
            // Je Token: Residualstrom, dann seine Norm.
            let zwischen = rechnen_breit(n, 2 * hs, faeden, |b, ziel| {
                let (residual, norm) = self.residual_eins_und_norm(layer, &zustaende[b], &o_aus[b], &acc_attn, None);
                ziel[..hs].copy_from_slice(&residual);
                ziel[hs..].copy_from_slice(&norm);
            });
            let residuen: Vec<&[i16]> = zwischen.chunks_exact(2 * hs).map(|z| &z[..hs]).collect();
            let normen: Vec<&[i16]> = zwischen.chunks_exact(2 * hs).map(|z| &z[hs..]).collect();

            // ⚑ **Dann der MLP fuer alle zugleich**, dicht oder als
            // Gemisch mit den Token je Experte gruppiert (Fund 371).
            let mlp_aus = self.ebene_mlp_stapel(layer, &normen, &acc_mlp);
            zustaende = rechnen_breit(n, hs, faeden, |b, ziel| {
                ziel.copy_from_slice(&self.residual_zwei(residuen[b], &mlp_aus[b], sc, &acc_mlp, out_frac));
            })
            .chunks_exact(hs)
            .map(<[i16]>::to_vec)
            .collect();
        }
        zustaende
    }

    /// **Ein ganzer Prompt ab Position null**: alle Positionen ausser der
    /// letzten gebuendelt, die letzte mit Kopf. Zurueck kommen die Logits
    /// der letzten Position, bei einem leeren Prompt lauter Nullen.
    ///
    /// # 📌 Fund 366 (2026-09-14): der gemessene Weg war nicht der benutzte
    ///
    /// [`Model::vorbereiten_stapel`] war seit dem 2026-09-11 gemessen und
    /// gegen den tokenweisen Weg geprueft. **Aufgerufen hat ihn nur das
    /// Messprogramm.** Die Erzeugung im Klienten, der Konformitaetslauf
    /// und der Testclient bereiteten Token fuer Token vor. Gemessen am
    /// 2026-09-14 mit 219 Token unter `cpu-simd`: 0,6B 2,83 gegen 3,95 s,
    /// 4B 7,45 gegen 9,69 s. **Die Buendelung, die in den Messungen stand,
    /// erreichte niemanden, der ein Modell benutzt.**
    ///
    /// ⚑ **Eine Stelle fuer alle Aufrufer**, damit das nicht wieder
    /// auseinanderlaeuft: `generate_beobachtet` und
    /// `dekodieren_mit_digest` rufen diese Methode, und ueber den zweiten
    /// laufen die E2E-Vektoren mit mehr als einem Prompt-Token durch die
    /// Buendelung.
    ///
    /// ⚑ **In Fenstern von [`VORBEREITUNGSFENSTER`] Token**, siehe dort.
    pub fn prompt_vorbereiten(&self, token_ids: &[usize], cache: &mut KVCache) -> Vec<i32> {
        self.prompt_vorbereiten_ab(token_ids, 0, cache)
    }

    /// **Die Logits JEDER Position einer Folge, gebuendelt.**
    ///
    /// # ⛔️ Warum es sie gibt (2026-09-16)
    ///
    /// **Die Messwerkzeuge dieses Projekts rechneten Token fuer Token.**
    /// `perplexity_probe` und `entscheidungsprobe` liefen in einer
    /// Schleife ueber [`Model::forward_token`], und damit erreichten sie
    /// weder die gebuendelte Vorbereitung noch die GPU: Die rechnet erst
    /// ab sechzehn Eingaben je Buendel, und ein einzelnes Token kommt
    /// dort nie an. **Die Messung lief also auf dem einen Pfad, den die
    /// Optimierung nicht beruehrt**, und beim 30B kostete das Stunden.
    ///
    /// ⚑ **Dieselbe Rechnung, andere Reihenfolge.** Die Ebenen laufen
    /// ueber [`Model::vorbereiten_stapel`], dessen Kopf ausfuehrt, warum
    /// jede einzelne Rechnung dieselbe bleibt. **Der LM-Kopf bleibt
    /// tokenweise**, Zeichen fuer Zeichen derselbe Aufruf wie in
    /// [`Model::forward_token`]: Er ist damit nicht schneller, aber auch
    /// nicht anders, und das ist hier mehr wert. Wer ihn buendelt, tut
    /// es als eigenen Schritt mit eigener Gegenprobe.
    ///
    /// ⚑ **In Fenstern von [`VORBEREITUNGSFENSTER`]**, wie
    /// [`Model::prompt_vorbereiten_ab`]: Eine Folge von zehntausend
    /// Token haette sonst zehntausend Zustaende gleichzeitig im
    /// Speicher.
    ///
    /// `pos_start` ist die Position des ersten Tokens; der Speicher wird
    /// dabei gefuellt wie beim tokenweisen Weg.
    pub fn logits_stapel(
        &self,
        token_ids: &[usize],
        pos_start: usize,
        cache: &mut KVCache,
    ) -> Vec<Vec<i32>> {
        let cfg = &self.config;
        let mut aus = Vec::with_capacity(token_ids.len());
        for (i, fenster) in token_ids.chunks(VORBEREITUNGSFENSTER).enumerate() {
            let zustaende =
                self.vorbereiten_stapel(fenster, pos_start + i * VORBEREITUNGSFENSTER, cache);
            for hidden in &zustaende {
                // Wortgleich zu `forward_token`, Schritt 3 und 4.
                let normed = rmsnorm_i16(
                    hidden,
                    &self.final_residual_frac,
                    &self.final_norm_gamma.data,
                    &self.final_norm_gamma.shifts,
                    &self.rsqrt_lut,
                    cfg.rsqrt_input_shift,
                    cfg.rsqrt_output_frac,
                    self.inv_n_q20,
                    self.final_norm_frac,
                );
                aus.push(self.logits_aus_normiertem(&normed, cfg.logit_frac_bits));
            }
        }
        aus
    }

    /// **Wie [`Model::prompt_vorbereiten`], aber die ersten `ab` Token
    /// stehen schon im KV-Speicher** und werden nicht noch einmal
    /// gerechnet.
    ///
    /// ⚑ **Bitgleich zur Vorbereitung ab null**: Der Eintrag an Position
    /// `p` haengt nur an den Token `0..=p`, und die sind dieselben. Die
    /// Fenster beginnen hier bei `ab` statt bei null; an keiner Zahl
    /// aendert das etwas, siehe [`VORBEREITUNGSFENSTER`].
    ///
    /// ⚠️ **Mindestens das letzte Token wird immer gerechnet**, denn nur so
    /// entstehen die Logits: `ab` darf hoechstens `token_ids.len() - 1`
    /// sein.
    pub fn prompt_vorbereiten_ab(&self, token_ids: &[usize], ab: usize, cache: &mut KVCache) -> Vec<i32> {
        let Some((&letzte, davor)) = token_ids.split_last() else {
            return vec![0i32; self.vocab_size];
        };
        assert!(
            token_ids.len() <= self.kontextgrenze(),
            "Der Prompt hat {} Token, die Kontextgrenze des Modells ist {}",
            token_ids.len(),
            self.kontextgrenze()
        );
        assert!(ab <= davor.len(), "prompt_vorbereiten_ab: {ab} Token wiederverwendet, aber nur {} vor dem letzten", davor.len());
        for (i, fenster) in davor[ab..].chunks(VORBEREITUNGSFENSTER).enumerate() {
            self.vorbereiten_stapel(fenster, ab + i * VORBEREITUNGSFENSTER, cache);
        }
        self.forward_token(letzte, davor.len(), cache)
    }

    /// **Embedding und alle Ebenen, ohne Schlussnorm und ohne Kopf.**
    ///
    /// # ⚑ Genau das, was ein Prompt-Token beitraegt
    ///
    /// Waehrend der Vorbereitung fuellt jedes Prompt-Token den
    /// KV-Speicher, und **seine Logits liest niemand**: Gefragt wird
    /// erst nach der letzten Position. Bis zum 2026-09-11 rechnete die
    /// Vorbereitung den Kopf trotzdem, fuer jedes Token einzeln.
    ///
    /// 📌 **Gemessen beim 4B-Modell:** Der Kopf ist eine int16-Matrix
    /// ueber 151 936 Zeilen, also 0,78 GB von 4,41 GB je Token,
    /// **17,6 %**. Bei einem Prompt von 588 Token sind das 458 GB
    /// gelesene Gewichte ohne Gegenwert.
    ///
    /// ⚑ **Ein Ausschnitt und kein zweiter Pfad.** `forward_token` ruft
    /// dieselbe Funktion und setzt Norm und Kopf darauf; es gibt
    /// weiterhin **eine** Umsetzung des Vorwaertspasses, und die ist
    /// ueber dreissig Konformitaetsvektoren belegt.
    pub fn durch_die_ebenen(
        &self,
        token_id: usize,
        pos: usize,
        cache: &mut KVCache,
    ) -> Vec<i16> {

        // 1. Embedding Lookup: Gewicht int8 mit Per-Channel-Skala der
        //    Token-Zeile (theta_v 0.7.0) -> erstes Residualstrom-Segment.
        //    Seit Fund 20 (theta_v 0.11.0) traegt das Segment eine Skala
        //    je Kanal.
        let first_residual_frac = &self.layers[0].scales.residual_in_frac;
        let emb = self.embedding_table.row(token_id);
        let emb_shift = self.embedding_table.shifts[token_id];
        let mut hidden: Vec<i16> = emb
            .iter()
            .enumerate()
            .map(|(i, v)| clamp_i16(rescale(*v as i32, emb_shift, first_residual_frac[i])))
            .collect();

        // 2. Transformer Layers: jeder Layer gibt den Strom auf der Skala
        //    des Folge-Segments aus (Eingangsskala des naechsten Layers bzw.
        //    des finalen Norm-Eingangs).
        for (i, layer) in self.layers.iter().enumerate() {
            let out_frac: &[u8] = if i + 1 < self.layers.len() {
                &self.layers[i + 1].scales.residual_in_frac
            } else {
                &self.final_residual_frac
            };
            hidden = self.forward_layer(layer, &hidden, pos, cache, out_frac, None, None);
        }

        hidden
    }

    // === Pipeline-Stage-API (Phase 12.56–12.59) =======================
    // Die drei Methoden zerlegen `forward_token` in Stage-Bausteine:
    // Embedding (Stage 0), Layer-Block (alle Stages), Norm+LM-Head
    // (letzte Stage). Zusammengesetzt mit denselben Skalen und
    // Rundungen wie der Einzelknoten-Pfad — eine Stage-Pipeline rechnet
    // dadurch dieselben Werte (plus dokumentierte Boundary-Reskalierung
    // zwischen den Stages, siehe `integer-llm-pipeline`).

    /// Embedding-Lookup: liefert das erste Residualstrom-Segment auf der
    /// Eingangsskala von Layer 0 (`layers[0].scales.residual_in_frac`).
    pub fn embed_token(&self, token_id: usize) -> Vec<i16> {
        let first_residual_frac = &self.layers[0].scales.residual_in_frac;
        let emb = self.embedding_table.row(token_id);
        let emb_shift = self.embedding_table.shifts[token_id];
        emb.iter()
            .enumerate()
            .map(|(i, v)| clamp_i16(rescale(*v as i32, emb_shift, first_residual_frac[i])))
            .collect()
    }

    /// Führt die Layer `[layer_start, layer_end)` auf dem Residualstrom
    /// aus. Eingang auf der Skala von
    /// `layers[layer_start].scales.residual_in_frac`, Ausgang auf der
    /// Skala von `layers[layer_end].scales.residual_in_frac` (bzw.
    /// `final_residual_frac` für `layer_end == num_layers`).
    /// KV-Cache wird gelesen und geschrieben (absolute Layer-Indizes).
    pub fn run_layers(
        &self,
        hidden: Vec<i16>,
        pos: usize,
        cache: &mut KVCache,
        layer_start: usize,
        layer_end: usize,
    ) -> Vec<i16> {
        self.run_layers_intern(hidden, pos, cache, layer_start, layer_end, None)
    }

    /// Dasselbe, aber mit Mitschnitt für den Rückwärtspass (TRAINING V).
    ///
    /// ⚑ **Ein zweiter Eingang, kein zweiter Pfad.** Beide Wege laufen
    /// durch dieselbe Schleife und dieselbe `forward_layer`; das ist die
    /// ganze Absicht. Genau dieser Vorwärtspass ist über dreissig
    /// Konformitätsvektoren als bitgleich belegt, und eine zweite
    /// Umsetzung wäre eine zweite Wahrheit über den Rechenpfad.
    #[allow(clippy::too_many_arguments)]
    pub fn run_layers_mit_mitschnitt(
        &self,
        hidden: Vec<i16>,
        pos: usize,
        cache: &mut KVCache,
        layer_start: usize,
        layer_end: usize,
        mitschnitt: &mut crate::mitschnitt::Zwischenwerte,
    ) -> Vec<i16> {
        self.run_layers_intern(hidden, pos, cache, layer_start, layer_end, Some(mitschnitt))
    }

    #[allow(clippy::too_many_arguments)]
    fn run_layers_intern(
        &self,
        mut hidden: Vec<i16>,
        pos: usize,
        cache: &mut KVCache,
        layer_start: usize,
        layer_end: usize,
        mut mitschnitt: Option<&mut crate::mitschnitt::Zwischenwerte>,
    ) -> Vec<i16> {
        for i in layer_start..layer_end {
            let out_frac: &[u8] = if i + 1 < self.layers.len() {
                &self.layers[i + 1].scales.residual_in_frac
            } else {
                &self.final_residual_frac
            };
            hidden = self.forward_layer(
                &self.layers[i],
                &hidden,
                pos,
                cache,
                out_frac,
                None,
                // ⚑ `as_deref_mut`, sonst wandert die Ausleihe in der
                // ersten Runde hinein und fehlt in der zweiten.
                mitschnitt.as_deref_mut(),
            );
        }
        hidden
    }

    /// Finale RMSNorm + LM-Head: Eingang auf `final_residual_frac`,
    /// Ausgabe die Logits auf `config.logit_frac_bits`.
    pub fn head_logits(&self, hidden: &[i16]) -> Vec<i32> {
        self.head_logits_mit_spur(hidden, self.config.logit_frac_bits, None)
    }

    /// Dasselbe, aber die Spur der Abschlussnormierung fällt mit ab.
    ///
    /// ⚑ **Zweiter Eingang und kein zweiter Pfad** (TRAINING V). Der
    /// Rückwärtspass der Normierung braucht das `r`, das der
    /// Vorwärtspass nachgeschlagen hat; ein zweiter Nachschlag könnte
    /// einen anderen Eintrag treffen. **Und eine zweite Umsetzung des
    /// Kopfes wäre eine zweite Wahrheit über den Rechenpfad**, deren
    /// Bitgleichheit niemand belegt.
    /// Der LM-Kopf über einem **bereits normierten** Strom.
    ///
    /// # ⚑ Eine Umsetzung, drei Aufrufer (Fund 178)
    ///
    /// Bis zum 2026-09-04 stand dieser Block **dreimal wortgleich** im
    /// Modul: in [`Self::forward_token`], in
    /// [`Self::head_logits_mit_spur`] und im Aktivierungsabzug. Über
    /// [`Self::head_logits_mit_spur`] stand dabei der Satz, eine zweite
    /// Umsetzung des Kopfes wäre „eine zweite Wahrheit über den
    /// Rechenpfad", und es waren schon drei.
    ///
    /// ⚑ **Das ist nicht bloss Wiederholung, es hat schon einmal
    /// gekostet.** Fund 176 war genau dieser Fehler eine Ebene höher:
    /// `forward_token_mit_routing` normierte selbst und reichte den
    /// Strom dann an einen Kopf, der noch einmal normierte. Wer eine der
    /// drei Kopien anfasst, ändert den Rechenpfad für ein Drittel der
    /// Aufrufer.
    ///
    /// **Pfad A** (Ausnahme in theta_v): INT16-LM-Kopf mit
    /// Per-Kanal-Skalen, i64-Akkumulator (896 · 32767 · 32767 > i32) und
    /// Zeilenumskalierung auf die gemeinsame Logitskala, denn jede Zeile
    /// hat ihren eigenen Zweierpotenzversatz. **Pfad B** (Rückfall,
    /// ältere Artefakte mit gebundenen Einbettungen): INT8 × INT16,
    /// i64-Akkumulator, dieselbe Zeilenumskalierung (theta_v 0.7.0).
    ///
    /// `logit_frac` ist die Skala der Ausgabe, siehe
    /// [`Self::head_logits_mit_spur`].
    fn logits_aus_normiertem(&self, normed: &[i16], logit_frac: u8) -> Vec<i32> {
        // 📌 **Fund 370, zweiter Teil (2026-09-14): der Kopf rechnete
        // einkernig**, beim 30B 29 % eines Decode-Schritts: 151 936 Zeilen
        // mal 2 048 Spalten auf einem Faden, während die übrigen warteten.
        // Jede Zeile ist ihr eigenes Skalarprodukt und schreibt in ihr
        // eigenes Feld; verteilt ändert sich also keine Zahl, dieselbe
        // Eigenschaft wie bei `linear.rs`.
        let hidden_dim = normed.len();
        let faeden = integer_llm_kernels::linear::kerngrenze();
        if let Some(lmh) = &self.lm_head_int16 {
            let werte: &[i16] = &lmh.data;
            integer_llm_kernels::fadenpool::rechnen_i32(self.vocab_size, faeden, |row| {
                let base = row * hidden_dim;
                let acc = integer_llm_kernels::attention::dot_int(&werte[base..base + hidden_dim], normed);
                let row_frac = lmh.shifts[row] + self.final_norm_frac;
                let y = rescale_i64(acc, row_frac, logit_frac);
                y.clamp(i32::MIN as i64, i32::MAX as i64) as i32
            })
        } else {
            integer_llm_kernels::fadenpool::rechnen_i32(self.vocab_size, faeden, |row| {
                let spalten = self.lm_head.cols();
                let acc = integer_llm_kernels::dot::dot_i8_i16(&self.lm_head.data[row * spalten..(row + 1) * spalten], normed);
                let row_frac = self.lm_head.shifts[row] + self.final_norm_frac;
                let y = rescale_i64(acc, row_frac, logit_frac);
                y.clamp(i32::MIN as i64, i32::MAX as i64) as i32
            })
        }
    }

    ///
    /// # ⚑ Warum die Logitskala ein Argument ist (Fund 177)
    ///
    /// `logit_frac_bits` steht im Lader auf 6 mit der Begründung
    /// „nur fuer Sampling/Argmax (skaleninvariant)". Das ist richtig:
    /// Wer nur das Maximum sucht, dem ist die Skala gleich. **Wer eine
    /// Wahrscheinlichkeitsverteilung braucht, dem nicht**, und der
    /// Kreuzentropiegradient des Trainings braucht genau die. Bei Q6
    /// liest die exp-Tabelle, die Q8 erwartet, den Exponenten viermal zu
    /// flach; gemessen am 2026-09-04 fielen dabei 150.064 von 151.936
    /// Wörtern auf null.
    ///
    /// ⚑ **Ein Argument und kein vierter Kopf.**
    /// [`Self::head_logits`] reicht unverändert `cfg.logit_frac_bits`
    /// durch; der Inferenzpfad rechnet Bit für Bit dasselbe wie vorher.
    pub fn head_logits_mit_spur(
        &self,
        hidden: &[i16],
        logit_frac: u8,
        spur: Option<&mut integer_llm_kernels::rmsnorm::Rmsnormspur>,
    ) -> Vec<i32> {
        let cfg = &self.config;
        let normed = integer_llm_kernels::rmsnorm::rmsnorm_i16_mit_spur(
            hidden,
            &self.final_residual_frac,
            &self.final_norm_gamma.data,
            &self.final_norm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            self.final_norm_frac,
            spur,
        );
        self.logits_aus_normiertem(&normed, logit_frac)
    }

    /// **Der Aufmerksamkeitsteil einer Ebene, bis vor den MLP.**
    ///
    /// Gibt den Residualstrom nach der Aufmerksamkeit zurueck und seine
    /// Norm, also genau die beiden Werte, die der MLP-Teil braucht.
    ///
    /// # ⚑ Warum das eine eigene Methode ist
    ///
    /// **Damit die Vorbereitung den MLP fuer viele Token zugleich
    /// rechnen kann.** Dieser Teil bleibt tokenweise, und das muss er
    /// auch: Die Aufmerksamkeit von Token `b` liest den KV-Speicher,
    /// den die Token davor in **dieser** Ebene gerade gefuellt haben.
    ///
    /// ⚑ **Es ist eine Naht und keine zweite Umsetzung.** Der Rumpf ist
    /// Zeile fuer Zeile der frueher in [`Model::forward_layer`]
    /// stehende, und dieselben Konformitaetsvektoren laufen darueber.
    fn ebene_bis_mlp(
        &self,
        layer: &TransformerLayer,
        hidden: &[i16],
        pos: usize,
        cache: &mut KVCache,
        mut auf: Option<&mut crate::mitschnitt::Ebenenmitschnitt>,
    ) -> (Vec<i16>, Vec<i16>) {
        if let Some(a) = auf.as_mut() {
            a.residual_ein = hidden.to_vec();
        }

        // === Der Mischer: Achtsamkeit oder Zustandsschicht ===
        let norm_hidden = self.norm_vor_aufmerksamkeit(layer, hidden);
        if let Some(a) = auf.as_mut() {
            a.norm_ein = norm_hidden.clone();
        }

        let acc_mischer = akkumulationsskala_mischer(&layer.scales);
        let o_out = match &layer.mischer {
            Mischer::Achtsamkeit(_) => {
                // ⚑ **Dieselben vier Schritte wie in der gebuendelten
                // Vorbereitung**, hier mit einer einzigen Eingabe. Siehe
                // [`IntegerModel::vorbereiten_stapel`].
                let [q_roh, k_flat, v_flat] = self
                    .projektionen_qkv(layer, &[norm_hidden.as_slice()])
                    .pop()
                    .expect("eine Eingabe ergibt genau eine Projektion");
                // ⛔️ **`q_proj` liefert Abfrage UND Tor**, je Kopf
                //   hintereinander (Qwen3.6). Ohne Tor ist `q_roh` schon
                //   die Abfrage.
                let (q_flat, tor) = if self.achtsamkeit_mit_tor {
                    let (q, g) = abfrage_und_tor_trennen(
                        &q_roh, self.num_heads, self.head_dim,
                    );
                    (q, Some(g))
                } else {
                    (q_roh, None)
                };
                let attn_out = self.aufmerksamkeit_eines_tokens(
                    layer,
                    &q_flat,
                    &k_flat,
                    &v_flat,
                    pos,
                    cache,
                    auf.as_deref_mut(),
                );
                // ⚑ **Das Tor wirkt VOR `o_proj`**, wie in der Vorlage:
                //   `attn_output * sigmoid(gate)`, dann erst die
                //   Rueckprojektion.
                let attn_out = match tor {
                    Some(g) => self.tor_anwenden(&attn_out, &g, layer),
                    None => attn_out,
                };
                self.projektion_o(layer, &[attn_out.as_slice()], &acc_mischer)
                    .pop()
                    .expect("eine Eingabe ergibt genau eine Projektion")
            }
            Mischer::Zustand(zs) => {
                if std::env::var_os("MYL_ZUSTANDSSPUR").is_some() {
                    eprintln!(
                        "[spur] Ebene {} ZUSTANDSZWEIG: konv_frac={} norm_aus_frac={} \
                         qkv_frac={} z_frac={}",
                        layer.layer_idx, zs.skalen.konv_frac, zs.skalen.norm_aus_frac,
                        zs.skalen.qkv_frac, zs.skalen.z_frac
                    );
                }
                let masse = self.zustandsmasse.as_ref().expect(
                    "eine Ebene mischt rekurrent, aber das Modell traegt keine \
                     Zustandsmasse; der Lader haette das abfangen muessen",
                );
                // ⚑ **Welche Ebenen rekurrent sind, steht am Modell**, und
                //   der Speicher legt sich beim ersten Mal danach an.
                let rekurrent: Vec<bool> =
                    self.layers.iter().map(|l| l.ist_rekurrent()).collect();
                let speicher = cache.zustand_bereit(
                    &rekurrent,
                    masse.wert_koepfe,
                    masse.schluessel_dim,
                    masse.wert_dim,
                    masse.kanaele,
                );
                // ⛔️ **Hier stand bis zum 2026-09-21 EINE gemeinsame
                //   Ausgangsskala** (das Minimum ueber alle Kanaele), und
                //   das war falsch. Der Residualstrom traegt eine Skala
                //   **je Kanal**, beim grossen Modell von 16 bis 19; die
                //   Achtsamkeit liefert ihren Beitrag ueber
                //   `projektion_o` entsprechend per Kanal. Ein Beitrag
                //   auf einer einzigen Skala wird beim Addieren um bis zu
                //   drei Bit falsch gewichtet.
                //
                //   📌 **Zwei Wege in denselben Strom muessen dieselbe
                //   Skalenform sprechen.**
                self.zustandsschicht_eines_tokens(
                    layer, zs, masse, &norm_hidden, speicher, &acc_mischer,
                )
            }
        };
        self.residual_eins_und_norm(layer, hidden, &o_out, &acc_mischer, auf)
    }

    /// Die Normierung vor der Aufmerksamkeit, fuer ein Token.
    fn norm_vor_aufmerksamkeit(&self, layer: &TransformerLayer, hidden: &[i16]) -> Vec<i16> {
        let cfg = &self.config;
        let sc = &layer.scales;
        // Pre-Attention RMSNorm (int16 -> int16 auf der kalibrierten
        // q/k/v-Eingangsskala; Gamma mit Per-Element-Skalen, theta_v 0.7.0).
        rmsnorm_i16(
            hidden,
            &sc.residual_in_frac,
            &layer.input_layernorm_gamma.data,
            &layer.input_layernorm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            sc.norm_attn_frac,
        )
    }

    /// **q, k und v fuer eine oder viele Positionen**, mit Bias.
    ///
    /// # 📌 Fund 366 (2026-09-14): die Haelfte, die tokenweise blieb
    ///
    /// Gebuendelt war bis zu diesem Tag nur der MLP. Die vier
    /// Projektionen der Aufmerksamkeit liefen auch in der Vorbereitung
    /// Token fuer Token, und gemessen am 4B-Modell mit 219 Token lagen
    /// dort **65 % der Vorbereitung**, davon der groessere Teil in genau
    /// diesen Matrizen. Jede Position las die 907 MB Gewichte der
    /// Aufmerksamkeitshaelfte einmal fuer sich.
    ///
    /// ⚑ **Die Projektion haengt nur an der Eingabe dieser Position**,
    /// nicht am KV-Speicher. Sie darf deshalb fuer alle Positionen einer
    /// Ebene vorab laufen; erst die Aufmerksamkeit selbst braucht die
    /// Reihenfolge. Jedes Ausgabeelement bleibt dasselbe Skalarprodukt.
    fn projektionen_qkv(&self, layer: &TransformerLayer, normen: &[&[i16]]) -> Vec<[Vec<i16>; 3]> {
        let sc = &layer.scales;
        // Q, K, V Projektionen: Per-Channel-Gewichtsskalen (theta_v 0.7.0),
        // Ausgang auf der jeweils kalibrierten Per-Layer-Skala.
        // Die Gewichte liegen im `QTensor` flach und werden flach
        // durchgereicht. Bis v0.13.4 stand hier `self.to_vec_vec(...)`,
        // das je Aufruf eine Heap-Allokation und eine Kopie **je
        // Ausgabe-Zeile** erzeugte: bei 0,5B zusammen 358 MB und 304 128
        // Allokationen je Token, denn diese Umwandlung lief achtmal je
        // Ebene und die Ebenen 24-mal je Token. Die Zahlen ändern sich
        // dadurch nicht, `dot_i8_i16` bekommt dieselben Bytes in
        // derselben Reihenfolge.
        let q = linear_fuer_alle(normen, &layer.achtsamkeit().q_proj.data, layer.achtsamkeit().q_proj.cols(), &layer.achtsamkeit().q_proj.shifts, sc.norm_attn_frac, sc.achtsamkeit().q_frac);
        let k = linear_fuer_alle(normen, &layer.achtsamkeit().k_proj.data, layer.achtsamkeit().k_proj.cols(), &layer.achtsamkeit().k_proj.shifts, sc.norm_attn_frac, sc.achtsamkeit().k_frac);
        let v = linear_fuer_alle(normen, &layer.achtsamkeit().v_proj.data, layer.achtsamkeit().v_proj.cols(), &layer.achtsamkeit().v_proj.shifts, sc.norm_attn_frac, sc.achtsamkeit().v_frac);

        // Attention-Biases (Qwen2.5: q/k/v_proj besitzen welche):
        // Per-Element-Skalen, Reskalierung auf die Q/K/V-Ausgabeskala und
        // i64-Addition mit Clamping — reine Ganzzahlarithmetik.
        q.into_iter()
            .zip(k)
            .zip(v)
            .map(|((mut q_flat, mut k_flat), mut v_flat)| {
                if let Some(qb) = &layer.achtsamkeit().q_bias {
                    add_bias_i16(&mut q_flat, &qb.data, &qb.shifts, sc.achtsamkeit().q_frac);
                }
                if let Some(kb) = &layer.achtsamkeit().k_bias {
                    add_bias_i16(&mut k_flat, &kb.data, &kb.shifts, sc.achtsamkeit().k_frac);
                }
                if let Some(vb) = &layer.achtsamkeit().v_bias {
                    add_bias_i16(&mut v_flat, &vb.data, &vb.shifts, sc.achtsamkeit().v_frac);
                }
                [q_flat, k_flat, v_flat]
            })
            .collect()
    }

    /// **Die Aufmerksamkeit eines Tokens**: Koepfe, QK-Norm, RoPE,
    /// KV-Speicher, Skalarprodukte, Umskalierung auf die Eingangsskala
    /// von `o_proj`.
    ///
    /// ⚑ **Drei Schritte, und die gebuendelte Vorbereitung ruft dieselben
    /// drei** (Fund 366): [`Model::koepfe_drehen`] haengt nur an diesem
    /// Token, [`Model::kv_schreiben`] fuellt den Speicher in der
    /// Reihenfolge der Positionen, [`Model::aufmerksamkeit_lesen`] liest
    /// ihn nur. Hier laufen sie fuer ein Token hintereinander.
    #[allow(clippy::too_many_arguments)]
    fn aufmerksamkeit_eines_tokens(
        &self,
        layer: &TransformerLayer,
        q_flat: &[i16],
        k_flat: &[i16],
        v_flat: &[i16],
        pos: usize,
        cache: &mut KVCache,
        mut auf: Option<&mut crate::mitschnitt::Ebenenmitschnitt>,
    ) -> Vec<i16> {
        let (q_heads, k_heads, v_heads) = self.koepfe_drehen(layer, q_flat, k_flat, v_flat, pos);
        // ⚑ **Nach RoPE und vor dem Zwischenspeicher**: So hat die
        // Aufmerksamkeit sie gesehen, und nur so passt der Gradient.
        // V wird nicht gedreht und steht deshalb unverändert daneben.
        if let Some(a) = auf.as_mut() {
            a.q = q_heads.clone();
            a.k = k_heads.clone();
            a.v = v_heads.clone();
        }
        self.kv_schreiben(layer, &k_heads, &v_heads, pos, cache);
        self.aufmerksamkeit_lesen(layer, &q_heads, pos, cache, auf)
    }

    /// Die Skalen, auf denen q und k nach der QK-Norm liegen: **ohne
    /// QK-Norm** `q_frac` und `k_frac`, **mit** die Ausgangsskalen der
    /// Normierung.
    fn skalen_der_koepfe(layer: &TransformerLayer) -> (u8, u8) {
        match &layer.achtsamkeit().qk_norm {
            Some(qkn) => (qkn.q_out_frac, qkn.k_out_frac),
            None => (layer.scales.achtsamkeit().q_frac, layer.scales.achtsamkeit().k_frac),
        }
    }

    /// **Koepfe, QK-Norm und RoPE eines Tokens.** Haengt nur an diesem
    /// Token und seiner Position, nicht am KV-Speicher.
    #[allow(clippy::type_complexity)]
    fn koepfe_drehen(
        &self,
        layer: &TransformerLayer,
        q_flat: &[i16],
        k_flat: &[i16],
        v_flat: &[i16],
        pos: usize,
    ) -> (Vec<Vec<i16>>, Vec<Vec<i16>>, Vec<Vec<i16>>) {
        let cfg = &self.config;
        let sc = &layer.scales;

        // Auf Heads aufteilen. Q hat num_heads Heads, K/V bei GQA nur
        // num_kv_heads (Qwen2.5-0.5B: 14 vs. 2) - deshalb getrennte Aufteilung
        // statt eines gemeinsamen Head-Counts.
        let mut q_heads = self.split_heads(q_flat, self.num_heads);
        let mut k_heads = self.split_heads(k_flat, self.num_kv_heads);
        let v_heads = self.split_heads(v_flat, self.num_kv_heads);

        // QK-Norm (Qwen3): RMSNorm je Kopf ueber head_dim, **vor** RoPE.
        //
        // Die Reihenfolge ist nicht verhandelbar. RoPE dreht ein Paar
        // (x_j, x_{j+half}) um einen positionsabhaengigen Winkel; eine
        // Normierung danach wuerde ueber gedrehte Werte mitteln und das
        // Ergebnis von der Position abhaengig machen, obwohl die Norm es
        // nicht sein soll. Das Referenzmodell normiert vorher, und die
        // Reihenfolge ist Teil des Ausfuehrungsprofils.
        //
        // Die Ausgangsskala wechselt hier von `q_frac`/`k_frac` auf die
        // kalibrierte Norm-Ausgangsskala; RoPE selbst ist skaleninvariant
        // und reicht sie unveraendert weiter. Welche Skalen danach gelten,
        // sagt [`Model::skalen_der_koepfe`].
        if let Some(qkn) = &layer.achtsamkeit().qk_norm {
            qk_norm_heads(
                &mut q_heads,
                sc.achtsamkeit().q_frac,
                &qkn.q_gamma.data,
                &qkn.q_gamma.shifts,
                &self.rsqrt_lut,
                cfg.rsqrt_input_shift,
                cfg.rsqrt_output_frac,
                qkn.q_out_frac,
            );
            qk_norm_heads(
                &mut k_heads,
                sc.achtsamkeit().k_frac,
                &qkn.k_gamma.data,
                &qkn.k_gamma.shifts,
                &self.rsqrt_lut,
                cfg.rsqrt_input_shift,
                cfg.rsqrt_output_frac,
                qkn.k_out_frac,
            );
        }

        // RoPE (Fund-15-Fix, theta_v 0.10.0): Multi-Frequenz-RoPE mit
        // half-split-Paarung. Die cos/sin-LUTs sind flach row-major
        // [max_seq_len, head_dim/2]; je Position wird die Zeile
        // [idx*half, (idx+1)*half) gelesen und jedes Paar j nutzt seinen
        // eigenen Winkel. Q- und K-Heads separat rotieren (unterschiedliche
        // Head-Anzahl). Die Rotation ist skaleninvariant gegenueber der
        // Eingangs-Skala (cos/sin tragen rope_frac_bits).
        // ⚑ **Die halbe DREHBREITE, nicht die halbe Kopfbreite.** Bei
        // voller Drehung sind beide gleich; bei einer Teildrehung ist die
        // Tabellenzeile schmaler, und der Rest des Kopfvektors geht
        // unveraendert durch (siehe `rotate_half_split_i16`).
        let half = self.drehbreite / 2;
        // ⛔️ Kein `pos % Zeilen` mehr (Fund 368, siehe `kontextgrenze`).
        assert!(
            pos < self.kontextgrenze(),
            "Position {pos} liegt hinter der Kontextgrenze {} des Modells",
            self.kontextgrenze()
        );
        let cos_row = &self.cos_lut[pos * half..(pos + 1) * half];
        let sin_row = &self.sin_lut[pos * half..(pos + 1) * half];
        for qh in q_heads.iter_mut() {
            *qh = rotate_half_split_i16(qh, cos_row, sin_row, cfg.rope_frac_bits);
        }
        for kh in k_heads.iter_mut() {
            *kh = rotate_half_split_i16(kh, cos_row, sin_row, cfg.rope_frac_bits);
        }
        (q_heads, k_heads, v_heads)
    }

    /// **Schreibt k und v eines Tokens in den KV-Speicher.**
    fn kv_schreiben(
        &self,
        layer: &TransformerLayer,
        k_heads: &[Vec<i16>],
        v_heads: &[Vec<i16>],
        pos: usize,
        cache: &mut KVCache,
    ) {
        // KV-Cache schreiben — OHNE Reskalierung (Fund 22, 2026-08-19).
        //
        // Bis theta_v 0.11.0 wurde hier auf eine GLOBALE Cache-Skala
        // (spec: kv_cache.frac_bits = 8) konvertiert und beim Lesen
        // zurueck auf dieselbe Per-Layer-Skala. Diese Rundreise
        // k_frac -> 8 -> k_frac gewann nichts (Quelle und Ziel sind
        // identisch, da Schreiben und Lesen dieselbe Ebene betreffen),
        // kostete aber:
        //   - doppelte Rundung je Position und Kopf,
        //   - 2-4 Bit Aufloesung auf FAST JEDER Ebene beider Modelle
        //     (0,5B: k median 3 Bit, v median 4 Bit; 7B: k 2, v 4),
        //   - hartes Clipping, wo der reale Wert die feste Kapazitaet
        //     von 32767/2^8 = 128 uebersteigt (7B Ebene 0: K-absmax 420,
        //     also Faktor 3,28 abgeschnitten - und das an der ERSTEN
        //     Ebene, deren Fehler durch alle 28 Ebenen propagiert).
        //
        // Der Cache haelt K/V jetzt in der nativen Per-Layer-Skala der
        // erzeugenden Projektion. Das ist streng verlustfrei gegenueber
        // vorher und beseitigt beide Effekte.
        for h in 0..self.num_kv_heads {
            cache.write(layer.layer_idx, h, pos, &k_heads[h], &v_heads[h]);
        }
    }

    /// **Die Aufmerksamkeit eines Tokens ueber dem schon gefuellten
    /// KV-Speicher**, bis zur Umskalierung auf die Eingangsskala von
    /// `o_proj`.
    ///
    /// ⚑ **Liest nur**, und nur die Positionen bis `pos`. Deshalb darf die
    /// gebuendelte Vorbereitung alle Token einer Ebene zuerst schreiben
    /// und dann gleichzeitig lesen: Token `b` sieht genau die Eintraege,
    /// die es tokenweise gesehen haette.
    fn aufmerksamkeit_lesen(
        &self,
        layer: &TransformerLayer,
        q_heads: &[Vec<i16>],
        pos: usize,
        cache: &KVCache,
        mut auf: Option<&mut crate::mitschnitt::Ebenenmitschnitt>,
    ) -> Vec<i16> {
        let cfg = &self.config;
        let sc = &layer.scales;
        let (q_akt_frac, k_akt_frac) = Self::skalen_der_koepfe(layer);

        // Attention pro Query-Head; group_size aufeinanderfolgende Query-Heads
        // teilen sich denselben KV-Head (Standard-GQA-Gruppierung, wie in
        // HF's repeat_kv: Head h liest KV-Head h / group_size).
        let group_size = self.num_heads / self.num_kv_heads;
        // ⚑ **Fund 59, zweite Stelle.** Hier stand `vec![0i16; hs]`, also
        // `hidden_size`. Geschrieben wird bis `num_heads · head_dim`, und
        // solange beide Zahlen zusammenfielen (Qwen2.5-0,5B und -7B), war
        // das dasselbe. Bei Qwen3-4B sind es 2560 gegen 4096, und der
        // erste Lauf brach hier ab: „len is 2560 but the index is 2560".
        //
        // `o_proj` bildet von `num_heads · head_dim` auf `hidden_size`
        // zurück, das ist die Stelle, an der die Breite wechselt, nicht
        // diese hier.
        //
        // Q liegt bei `q_akt_frac`, K bei `k_akt_frac`; der rohe
        // Skalarproduktwert traegt deren Summe an Nachkommabits.
        //
        // **Ohne QK-Norm sind das sc.achtsamkeit().q_frac und sc.achtsamkeit().k_frac, mit QK-Norm
        // die Ausgangsskalen der Normierung.** Die Unterscheidung ist
        // der Grund, warum die beiden Werte oben als eigene Variablen
        // entstehen und nicht hier aus `sc` gelesen werden: Ein
        // vergessenes `sc.achtsamkeit().q_frac` an dieser Stelle waere kein
        // Uebersetzungsfehler, sondern eine um Zweierpotenzen
        // verschobene Softmax. score_shift bringt ihn auf
        // die Score-Skala (score_frac_bits); exp_lut_shift uebersetzt von
        // dort in die Eingangsskala der exp-LUT (spec 0.5.2: Domaene
        // [0, 64) statt [0, 0.5), gemessene Score-Differenzen bis ~28).
        //
        // Fund 17 (Attention-Skalierung): HF-Qwen2 skaliert die Scores mit
        // 1/sqrt(head_dim) (attn_weights = q·k * head_dim^-0.5). Dieser
        // Faktor fehlte urspruenglich ganz; die Softmax war dadurch um
        // sqrt(head_dim) zu scharf.
        //
        // Fund 19: Die Umsetzung als reiner Rechtsshift um
        // log2(head_dim)/2 war Ganzzahldivision und damit nur fuer
        // GERADE Zweierpotenzen richtig. head_dim 128 (der Normalfall ab
        // 1,5B) bekam Shift 3 statt der noetigen 3,5, Faktor sqrt(2) zu
        // gross. Jetzt als Q15-Multiplikation; fuer head_dim 64 ist der
        // Multiplikator 4096 = 2^12 und das Ergebnis bitgleich zum
        // bisherigen Verhalten (siehe fixed_point::inv_sqrt_q15).
        let score_mult = inv_sqrt_q15(self.head_dim);
        let score_shift = (q_akt_frac as u16 + k_akt_frac as u16 + 15)
            .saturating_sub(cfg.score_frac_bits as u16) as u8;
        let exp_lut_shift = cfg.score_frac_bits.saturating_sub(cfg.exp_input_frac);

        // K, V liegen bereits in ihrer Per-Layer-Skala (Fund 22), keine
        // Reskalierung mehr noetig, und kommen als zusammenhaengende
        // Ausschnitte ohne Kopie aus dem Speicher. Sichtbar sind alle
        // Positionen bis `pos`: Die Aufmerksamkeit eines Tokens ist kausal,
        // weil spaetere Positionen noch nicht oder nur fuer spaetere Token
        // geschrieben sind.
        let kopf = |h: usize, spur: Option<&mut Vec<Vec<i32>>>| {
            let (k_seq, v_seq) = cache.lesen(layer.layer_idx, h / group_size, pos);
            aufmerksamkeit_einer_abfrage(
                &q_heads[h], k_seq, v_seq,
                score_mult, score_shift, &self.exp_lut, exp_lut_shift,
                cfg.prob_frac_bits,
                spur,
            )
        };

        // ⚑ **Die Koepfe rechnen verteilt**, denn jeder liest nur und
        // schreibt in seinen eigenen Abschnitt. Gemessen am 2026-09-14 (4B,
        // 1 907 Token Kontext): Die Aufmerksamkeit war 63 % eines
        // Decode-Schritts und lief auf einem Kern, waehrend die anderen
        // warteten. Aus der gebuendelten Vorbereitung heraus, die schon je
        // Token verteilt, rechnet der Pool die Koepfe selbst (siehe
        // `fadenpool::rechnen_breit`).
        //
        // ⚑ **Mit Mitschnitt nacheinander**: Die Spur haengt je Kopf eine
        // Zeile an, und ihre Reihenfolge ist die der Koepfe.
        let mut attn_out = match auf.as_mut() {
            Some(a) => {
                let mut aus = Vec::with_capacity(self.num_heads * self.head_dim);
                for h in 0..self.num_heads {
                    aus.extend(kopf(h, Some(&mut a.wahrscheinlichkeiten)));
                }
                aus
            }
            None => {
                let sichtbar = (pos + 1) * self.num_heads * self.head_dim;
                let faeden = if sichtbar >= KOEPFE_VERTEILEN_AB { integer_llm_kernels::linear::kerngrenze() } else { 1 };
                rechnen_breit(self.num_heads, self.head_dim, faeden, |h, ziel| {
                    ziel.copy_from_slice(&kopf(h, None));
                })
            }
        };

        // Die Attention-Ausgabe liegt auf der V-Skala (gewichtete Summe
        // erhaelt die V-Skala); Umreskalieren auf die kalibrierte
        // o_proj-Eingangsskala.
        if sc.achtsamkeit().attn_out_frac != sc.achtsamkeit().v_frac {
            for v in attn_out.iter_mut() {
                *v = clamp_i16(rescale(*v as i32, sc.achtsamkeit().v_frac, sc.achtsamkeit().attn_out_frac));
            }
        }
        // ⚑ **Nach der Umskalierung**, denn `o_proj` bekommt diese
        // Zahlen und keine anderen.
        if let Some(a) = auf.as_mut() {
            a.attn_aus = attn_out.clone();
        }

        attn_out
    }

    /// **o_proj fuer eine oder viele Positionen**, Begruendung bei
    /// [`Model::projektionen_qkv`].
    fn projektion_o(
        &self,
        layer: &TransformerLayer,
        attn_outs: &[&[i16]],
        acc_attn: &[u8],
    ) -> Vec<Vec<i16>> {
        let sc = &layer.scales;
        linear_pc_fuer_alle(attn_outs, &layer.achtsamkeit().o_proj.data, layer.achtsamkeit().o_proj.cols(), &layer.achtsamkeit().o_proj.shifts, sc.achtsamkeit().attn_out_frac, acc_attn)
    }

    /// Die erste Residualaddition und die Normierung vor dem MLP.
    fn residual_eins_und_norm(
        &self,
        layer: &TransformerLayer,
        hidden: &[i16],
        o_out: &[i16],
        acc_attn: &[u8],
        mut auf: Option<&mut crate::mitschnitt::Ebenenmitschnitt>,
    ) -> (Vec<i16>, Vec<i16>) {
        let cfg = &self.config;
        let hs = self.hidden_size;
        let sc = &layer.scales;

        // Residual Add 1: beide Operanden auf der Akkumulationsskala, Summe
        // in i64, DANN eine Reskalierung auf die mittlere Segmentskala und
        // eine Klemmung. Vor Fund 31 wurde `hidden` einzeln auf die
        // Zielskala geklemmt, bevor addiert wurde.
        let mut residual = vec![0i16; hs];
        for i in 0..hs {
            let h = rescale_i64(hidden[i] as i64, sc.residual_in_frac[i], acc_attn[i]);
            let summe = h + o_out[i] as i64;
            residual[i] = clamp_i16_from_i64(rescale_i64(summe, acc_attn[i], sc.residual_mid_frac[i]));
        }

        // === MLP-Block ===
        if let Some(a) = auf.as_mut() {
            a.residual_mitte = residual.clone();
        }

        let norm_residual = rmsnorm_i16(
            &residual,
            &sc.residual_mid_frac,
            &layer.post_attention_layernorm_gamma.data,
            &layer.post_attention_layernorm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            sc.norm_mlp_frac,
        );

        // Per-Layer-Skalen fuer alle Zwischenstufen; die SiLU-LUT arbeitet
        // in ihrer festen Domaene (silu_in_frac/Offset), Gate-Werte werden
        // dorthin reskaliert. out_residual_frac ist seit Fund 20 per Kanal
        // (down_proj addiert direkt in den Residualstrom).
        if let Some(a) = auf.as_mut() {
            a.norm_mitte = norm_residual.clone();
        }

        (residual, norm_residual)
    }

    // ⚑ **Acht Argumente, und die letzten beiden sind Ausgänge.** Sie
    // zu einem Typ zusammenzufassen hiesse, zwei unabhängige Mitschriften
    // aneinanderzubinden: die MoE-Diagnose läuft im Betrieb, der
    // Trainingsmitschnitt nur beim Training. Wer sie koppelte, zahlte
    // das eine, um das andere zu bekommen.
    #[allow(clippy::too_many_arguments)]
    fn forward_layer(
        &self,
        layer: &TransformerLayer,
        hidden: &[i16],
        pos: usize,
        cache: &mut KVCache,
        out_residual_frac: &[u8],
        befunde: Option<&mut Vec<Routingbefund>>,
        mitschnitt: Option<&mut crate::mitschnitt::Zwischenwerte>,
    ) -> Vec<i16> {
        let sc = &layer.scales;

        // ⚑ **Nur mitschneiden, wenn jemand zuhört** (TRAINING V).
        // Inferenz gibt `None` und zahlt je Aufnahmestelle einen
        // `is_some`, keine Kopie. Ein zweiter Vorwärtspass, der immer
        // alles behält, wäre eine zweite Wahrheit über den Rechenpfad.
        let mut auf = mitschnitt
            .is_some()
            .then(crate::mitschnitt::Ebenenmitschnitt::default);

        let (residual, norm_residual) =
            self.ebene_bis_mlp(layer, hidden, pos, cache, auf.as_mut());

        let acc_mlp: Vec<u8> = sc
            .residual_mid_frac
            .iter()
            .zip(out_residual_frac.iter())
            .map(|(&a, &b)| a.min(b))
            .collect();

        let aus = self.ebene_ab_mlp(
            layer,
            &residual,
            &norm_residual,
            &acc_mlp,
            out_residual_frac,
            befunde,
            auf.as_mut(),
        );

        // ⚑ **Ganz am Ende abgeben**, damit auch die MLP-Werte
        // drinstehen. Eine halb aufgezeichnete Ebene sähe vollständig
        // aus.
        if let (Some(m), Some(a)) = (mitschnitt, auf) {
            m.anhaengen(a);
        }

        aus
    }

    /// **Der MLP-Teil einer Ebene und die zweite Residualaddition.**
    ///
    /// # ⚑ Warum das eine eigene Methode ist
    ///
    /// **Damit die Vorbereitung ihn gebuendelt rechnen kann.** Die drei
    /// MLP-Matrizen sind beim 4B-Modell 74,7 MB von 100,9 MB je Ebene,
    /// also **74 % der gelesenen Gewichte**; wer sie fuer viele Token
    /// zugleich rechnet, liest sie einmal statt einmal je Token.
    ///
    /// ⚑ **Es ist eine Naht und keine zweite Umsetzung.**
    /// [`Model::forward_layer`] ruft dieselbe Methode; der Rumpf ist Zeile
    /// fuer Zeile der frueher hier stehende. Belegt durch den
    /// Konformitaetslauf, der beide Haelften ueber dieselben Vektoren
    /// fuehrt.
    #[allow(clippy::too_many_arguments)]
    fn ebene_ab_mlp(
        &self,
        layer: &TransformerLayer,
        residual: &[i16],
        norm_residual: &[i16],
        acc_mlp: &[u8],
        out_residual_frac: &[u8],
        befunde: Option<&mut Vec<Routingbefund>>,
        mut auf: Option<&mut crate::mitschnitt::Ebenenmitschnitt>,
    ) -> Vec<i16> {
        let cfg = &self.config;
        let sc = &layer.scales;
        let mlp_out = match &layer.ffn {
            Feedforward::Dense(mlp) => {
                let mut spur = auf.as_ref().map(|_| Mlpspur::default());
                let aus =
                    self.mlp_vorwaerts(mlp, norm_residual, sc, cfg, acc_mlp, spur.as_mut());
                if let (Some(a), Some(sp)) = (auf.as_mut(), spur) {
                    a.mlp = crate::mitschnitt::Mlpteil::Dicht {
                        gate: sp.gate,
                        up: sp.up,
                        h: sp.h,
                    };
                }
                aus
            }
            Feedforward::Moe(moe) => {
                // ⚑ **Seit dem 2026-09-05 wird auch hier aufgezeichnet**,
                // und zwar durch denselben Eingang wie beim dichten Fall:
                // ein `Option`, das im Regelbetrieb `None` ist.
                let mut spur = auf.as_ref().map(|_| crate::mitschnitt::Moespur::default());
                let aus = self.moe_vorwaerts(
                    moe,
                    norm_residual,
                    sc,
                    cfg,
                    acc_mlp,
                    befunde,
                    layer.layer_idx,
                    spur.as_mut(),
                );
                if let (Some(a), Some(sp)) = (auf.as_mut(), spur) {
                    a.mlp = crate::mitschnitt::Mlpteil::Expertengemisch {
                        experten: sp.experten,
                        gewichte: sp.gewichte,
                        ausgaben: sp.ausgaben,
                        teile: sp.teile,
                        logits: sp.logits,
                    };
                }
                aus
            }
        };

        // Final Residual Add — Fund 31 (2026-08-20, theta_v 0.17.0).
        //
        // Vorher wurde der Residualstrom EINZELN auf die Ausgangsskala
        // geklemmt und erst dann der MLP-Beitrag addiert. An einer
        // Ausloeschung zerstoert das den Wert vollstaendig, denn beide
        // Operanden koennen gross sein, waehrend nur ihre SUMME klein ist —
        // und die Ausgangsskala ist nach der Summe kalibriert.
        //
        // Gemessen an Qwen2.5-0,5B, Ebene 21, Kanal 62 (der Kanal mit der
        // "massive activation", Sun et al. 2024): Der wahre Wert faellt dort
        // von 1714 auf 61,6. Mit mid_frac 4 (+-2047,94) und out_frac 9
        // (+-64,00) ergab die alte Fassung
        //     Residualstrom  1723,2 -> roh 882 304 -> clamp  32 767 =  64,00
        //     MLP-Beitrag   -1653,0 -> roh -846 336 -> clamp -32 768 = -64,00
        //     Summe                                          -1 roh = -0,0020
        // also -0,002 statt 61,6 — zwei Klemmungen, die einander aufheben.
        //
        // Jetzt: Beide Operanden liegen auf der GROEBEREN der beiden Skalen
        // (kleinerer Shift), dort passen sie hinein; die Summe entsteht in
        // i64 und wird EINMAL auf die Ausgangsskala reskaliert und EINMAL
        // geklemmt. Die Aufloesung der groeberen Skala genuegt fuer das
        // Ergebnis: 61,6 bei Schrittweite 1/16 sind rund 10 Bit.
        //
        // Warum `min` und nicht `max`: `linear_w8a16_pc` liefert int16. Auf
        // einer feinen Skala wuerde der MLP-Beitrag selbst klemmen, bevor er
        // ueberhaupt addiert werden kann. Die Akkumulationsskala muss also
        // den OPERANDEN genuegen, nicht dem Ergebnis.
        self.residual_zwei(residual, &mlp_out, sc, acc_mlp, out_residual_frac)
    }

    /// **Die zweite Residualaddition, Fund 31.**
    ///
    /// ⚑ Beide Operanden auf der groeberen Skala, Summe in i64, **eine**
    /// Reskalierung und **eine** Klemmung. Die lange Begruendung steht
    /// ueber dem Aufrufer in [`Model::ebene_ab_mlp`].
    fn residual_zwei(
        &self,
        residual: &[i16],
        mlp_out: &[i16],
        sc: &LayerScales,
        acc_mlp: &[u8],
        out_residual_frac: &[u8],
    ) -> Vec<i16> {
        let hs = self.hidden_size;
        let mut out = vec![0i16; hs];
        for i in 0..hs {
            let r = rescale_i64(residual[i] as i64, sc.residual_mid_frac[i], acc_mlp[i]);
            let summe = r + mlp_out[i] as i64;
            out[i] = clamp_i16_from_i64(rescale_i64(summe, acc_mlp[i], out_residual_frac[i]));
        }
        out
    }

    /// Diagnose-Variante von `forward_token`: gibt zusätzlich je Layer den
    /// AbsMax und die ersten vier Werte des Residualstroms nach dem Layer
    /// zurück (inkl. der Skala des Segments). Nur für Messpfade — der
    /// Inferenzpfad bleibt unverändert.
    ///
    /// **Fund 20:** der Residualstrom trägt seit theta_v 0.11.0 eine Skala
    /// je Kanal, nicht mehr eine je Segment. Ein einzelnes `frac` fürs
    /// ganze Segment gibt es deshalb nicht mehr sinnvoll her — die
    /// Rückgabe liefert je Stufe den vollständigen Vektor UND seine
    /// Per-Kanal-Shifts, damit der Aufrufer jeden Kanal mit SEINEM
    /// eigenen Shift umrechnen kann.
    ///
    /// Die Umrechnung in Realwerte geschieht bewusst erst im aufrufenden
    /// Diagnose-Binary: `model.rs` steht auf der Heißpfad-Liste des
    /// Gleitkomma-Audits (`tests/audit/test_no_float.py`) und bleibt
    /// deshalb vollständig ganzzahlig.
    pub fn forward_token_dump(
        &self,
        token_id: usize,
        pos: usize,
        cache: &mut KVCache,
    ) -> (Vec<i32>, Vec<(Vec<i16>, Vec<u8>)>) {
        let cfg = &self.config;

        let first_residual_frac = &self.layers[0].scales.residual_in_frac;
        let emb = self.embedding_table.row(token_id);
        let emb_shift = self.embedding_table.shifts[token_id];
        let mut hidden: Vec<i16> = emb
            .iter()
            .enumerate()
            .map(|(i, v)| clamp_i16(rescale(*v as i32, emb_shift, first_residual_frac[i])))
            .collect();

        let mut dump = Vec::with_capacity(self.layers.len());
        for (i, layer) in self.layers.iter().enumerate() {
            let out_frac: &[u8] = if i + 1 < self.layers.len() {
                &self.layers[i + 1].scales.residual_in_frac
            } else {
                &self.final_residual_frac
            };
            hidden = self.forward_layer(layer, &hidden, pos, cache, out_frac, None, None);
            dump.push((hidden.clone(), out_frac.to_vec()));
        }

        // Finale Norm + LM-Head-Logits wie im echten Pfad.
        let normed = rmsnorm_i16(
            &hidden,
            &self.final_residual_frac,
            &self.final_norm_gamma.data,
            &self.final_norm_gamma.shifts,
            &self.rsqrt_lut,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            self.inv_n_q20,
            self.final_norm_frac,
        );
        // Ausgang der finalen Norm bleibt skalar (Post-Norm-Aktivierungen
        // haben keine Massive-Activation-Ausreisser, siehe rmsnorm.rs).
        let norm_shifts = vec![self.final_norm_frac; normed.len()];
        dump.push((normed.clone(), norm_shifts));

        let logits = self.logits_aus_normiertem(&normed, cfg.logit_frac_bits);

        (logits, dump)
    }

    /// Teilt einen flachen Q/K/V-Vektor in `n` Heads zu je `head_dim` auf.
    /// `n` ist `num_heads` fuer Q, bei GQA `num_kv_heads` fuer K/V.
    /// Eine dichte Feedforward-Einheit. Auch der einzelne Experte eines
    /// MoE laeuft hier durch: Er ist dieselbe Rechnung auf schmaleren
    /// Matrizen.
    fn mlp_vorwaerts(
        &self,
        mlp: &DenseMlp,
        x: &[i16],
        sc: &LayerScales,
        cfg: &ModelConfig,
        acc: &[u8],
        spur: Option<&mut Mlpspur>,
    ) -> Vec<i16> {
        mlp_int_mit_spur(
            x,
            &mlp.gate_proj.data,
            &mlp.up_proj.data,
            &mlp.down_proj.data,
            mlp.gate_proj.cols(),
            mlp.down_proj.cols(),
            &mlp.gate_proj.shifts,
            &mlp.up_proj.shifts,
            &mlp.down_proj.shifts,
            &self.silu_lut,
            sc.norm_mlp_frac,
            sc.gate_frac,
            sc.up_frac,
            sc.down_in_frac,
            cfg.silu_in_frac,
            cfg.silu_lut_offset,
            cfg.silu_out_frac,
            acc,
            spur,
        )
    }

    /// Ein Mixture-of-Experts-Modell: Router befragen, `top_k` Experten rechnen,
    /// Ergebnisse gewichtet mischen.
    ///
    /// **Die Arbeitsmenge ist konstant.** Es feuern immer genau `top_k`
    /// Experten, nie mehr und nie weniger; welche, haengt am Token, wie
    /// viele nicht. Daran haengt, dass die vTFE-Zuschreibung ohne
    /// Anfragezustand nachrechenbar bleibt.
    ///
    /// ⚑ **Kein Token-Dropping.** Es gibt hier keine Kapazitaetsgrenze,
    /// und das ist Absicht: Verwirft eine Implementierung Token, wenn ein
    /// Experte im Batch ueberlaeuft, haengt das Ergebnis an Position *i*
    /// davon ab, welche anderen Token im Batch lagen. Zwei redundante
    /// Pods batchen verschieden, und der Redundanzvergleich meldete zwei
    /// ehrliche Pods als abweichend.
    #[allow(clippy::too_many_arguments)]
    /// ⚑ **Neun Argumente, und die letzten beiden sind Ausgänge.**
    /// Dieselbe Aufteilung wie bei [`Self::forward_layer`]: Die
    /// Routing-Diagnose läuft im Betrieb, der Trainingsmitschnitt nur
    /// beim Training. Wer sie koppelte, zahlte das eine, um das andere
    /// zu bekommen.
    /// **Wen der Router waehlt, und mit welchem Gewicht.**
    fn moe_routing(
        &self,
        moe: &MoeLayer,
        x: &[i16],
        sc: &LayerScales,
        cfg: &ModelConfig,
    ) -> (Vec<i32>, integer_llm_kernels::moe::Routing) {
        // Router-Logits. Die Projektion laeuft wie jede andere; ihre
        // Ausgangsskala ist kalibriert wie die der uebrigen Projektionen.
        let logits_i16 = linear_w8a16(
            x,
            &moe.router.data,
            moe.router.cols(),
            &moe.router.shifts,
            sc.norm_mlp_frac,
            moe.router_frac,
        );
        let logits: Vec<i32> = logits_i16.iter().map(|l| *l as i32).collect();

        // Von der Router-Skala in die Eingangsskala der exp-Tabelle,
        // dieselbe Uebersetzung wie bei den Attention-Scores.
        let exp_lut_shift = moe.router_frac.saturating_sub(cfg.exp_input_frac);
        let routing = route_top_k(
            &logits,
            moe.top_k,
            &self.exp_lut,
            exp_lut_shift,
            cfg.prob_frac_bits,
            moe.norm_topk_prob,
        );
        (logits, routing)
    }

    #[allow(clippy::too_many_arguments)]
    fn moe_vorwaerts(
        &self,
        moe: &MoeLayer,
        x: &[i16],
        sc: &LayerScales,
        cfg: &ModelConfig,
        acc: &[u8],
        befunde: Option<&mut Vec<Routingbefund>>,
        layer_idx: usize,
        spur: Option<&mut crate::mitschnitt::Moespur>,
    ) -> Vec<i16> {
        let (logits, routing) = self.moe_routing(moe, x, sc, cfg);

        // Nur wenn jemand misst. Im Regelbetrieb ist das ein
        // Option-Test je MoE-Layer, sonst nichts; `randgleichstaende`
        // laeuft ein zweites Mal ueber die Logits und hat im heissen
        // Pfad nichts zu suchen.
        if let Some(sammler) = befunde {
            let ueber_wahrscheinlichkeit = integer_llm_kernels::softmax::softmax_int(
                &logits,
                &self.exp_lut,
                moe.router_frac.saturating_sub(cfg.exp_input_frac),
                cfg.prob_frac_bits,
            );
            let groesstes = logits.iter().copied().max().unwrap_or(0);
            let kleinstes = logits.iter().copied().min().unwrap_or(0);
            sammler.push(Routingbefund {
                layer: layer_idx,
                experten: routing.experten.clone(),
                randgleichstaende: integer_llm_kernels::moe::randgleichstaende(&logits, moe.top_k),
                randgleichstaende_wahrscheinlichkeit:
                    integer_llm_kernels::moe::randgleichstaende(
                        &ueber_wahrscheinlichkeit, moe.top_k,
                    ),
                gewichte: routing.gewichte.clone(),
                logit_spanne: groesstes.saturating_sub(kleinstes),
            });
        }

        // **Erst ankuendigen, dann rechnen.** Die Wahl steht fest,
        // sobald der Router gesprochen hat; ab hier ist bekannt, welche
        // vierundzwanzig Matrizen gebraucht werden. Ein Rat je Matrix,
        // bevor die erste gelesen wird, buendelt die Seitenfehler einer
        // ganzen Ebene zu einem Lauf (Fund 331, siehe
        // [`Gewichtsdaten::vorbereiten`]).
        //
        // ⚑ **Vor der Schleife und nicht darin.** In der Schleife wuerde
        // je Experte ein eigener Lauf entstehen, also acht statt einem;
        // der Sinn ist gerade, dass die Platte alle vierundzwanzig
        // Bereiche gleichzeitig kennt.
        for e in &routing.experten {
            moe.experts[*e as usize].vorbereiten();
        }

        // Die gewaehlten Experten rechnen. Alle schreiben auf dieselbe
        // Ausgangsskala `acc`, denn sie addieren in denselben
        // Residualstrom; deshalb mischt `mische_experten` ohne
        // Umskalierung.
        //
        // ⚑ **Ein Experte ist ein dichter Block**, und deshalb laeuft er
        // durch dieselbe `mlp_vorwaerts` wie eine dichte Ebene. Damit
        // gilt auch `schritt_auf_mlp` fuer ihn unveraendert, gemessen an
        // echten 30B-A3B-Gewichten.
        // 📌 **Fund 332: in zwei Poolrunden statt in vierundzwanzig.**
        // Die Begruendung und die Messung stehen bei
        // [`integer_llm_kernels::linear::linear_w8a16_buendel`]. Hier
        // steht nur, was sie fuer diese Stelle heisst: Bis zum
        // 2026-09-11 lief je Experte ein eigenes `mlp_vorwaerts`, also
        // drei eigene Runden, und jede bekam **zwei** von zwoelf
        // Faeden, weil eine Expertenmatrix knapp ueber der
        // Parallelschwelle liegt. Gebuendelt rechnen alle Kerne.
        // ⚠️ **Ohne gewaehlte Experten ist der Beitrag null.**
        // `route_top_k` liefert eine leere Wahl nur bei `top_k == 0`,
        // also bei einer kaputten Modellkonfiguration. Der gebuendelte
        // Weg fragt danach den ersten Experten nach seiner Form, und den
        // gaebe es dann nicht.
        //
        // 📌 **Und hier stand vorher etwas anderes, ohne dass es jemand
        // so gemeint haette:** `mische_experten` nimmt die Breite vom
        // ersten Element und gab bei leerer Liste einen **leeren**
        // Vektor zurueck. Der Residualstrom haette ihn nicht addieren
        // koennen. Eine leere Summe ist null, und null hat die Breite
        // des Stroms.
        if routing.experten.is_empty() {
            return vec![0i16; acc.len()];
        }
        let buendel: Vec<Expertenteil<'_>> = routing
            .experten
            .iter()
            .map(|e| {
                let ex = &moe.experts[*e as usize];
                Expertenteil {
                    gate: &ex.gate_proj.data,
                    up: &ex.up_proj.data,
                    down: &ex.down_proj.data,
                    gate_shifts: &ex.gate_proj.shifts,
                    up_shifts: &ex.up_proj.shifts,
                    down_shifts: &ex.down_proj.shifts,
                }
            })
            .collect();
        let mut teile: Vec<Mlpspur> = Vec::new();
        let ausgaben = mlp_int_experten(
            x,
            &buendel,
            moe.experts[routing.experten[0] as usize].gate_proj.cols(),
            moe.experts[routing.experten[0] as usize].down_proj.cols(),
            &self.silu_lut,
            sc.norm_mlp_frac,
            sc.gate_frac,
            sc.up_frac,
            sc.down_in_frac,
            cfg.silu_in_frac,
            cfg.silu_lut_offset,
            cfg.silu_out_frac,
            acc,
            spur.as_ref().map(|_| &mut teile),
        );

        if let Some(ziel) = spur {
            ziel.experten = routing.experten.clone();
            ziel.gewichte = routing.gewichte.clone();
            ziel.ausgaben = ausgaben.clone();
            ziel.teile = teile;
            ziel.logits = logits.clone();
        }

        let gemischt = mische_experten(&ausgaben, &routing.gewichte, cfg.prob_frac_bits);

        // ⚑ **Der geteilte Experte kommt obendrauf**, und zwar bei jedem
        // Token. Die Rechnung steht in `geteilten_experten_addieren`,
        // **einmal**, und der gebuendelte Weg ruft dieselbe (Fund 427).
        self.geteilten_experten_addieren(moe, x, gemischt, sc, cfg, acc)
    }

    /// **Der geteilte Experte obendrauf:**
    /// `aus = experten + sigmoid(tor(x)) * geteilt(x)`.
    ///
    /// # ⛔️ Fund 427: der gebuendelte Weg kannte ihn nicht
    ///
    /// Diese Rechnung stand bis zum 2026-09-22 im Rumpf von
    /// `moe_vorwaerts`, also nur auf dem Weg Token fuer Token.
    /// `moe_stapel` lief daran vorbei, und der geteilte Experte feuert
    /// bei **jedem** Token auf **jeder** Gemischebene. Gemessen: Die
    /// Perplexitaet des gebuendelten Weges lag um Groessenordnungen
    /// ueber der des einzelnen, waehrend die Logits des einzelnen
    /// Weges Token fuer Token mit dem offiziellen Modell uebereinstimmten.
    ///
    /// 📌 **Zwei Fassungen derselben Rechnung laufen auseinander, sobald
    /// ein Modell etwas Neues mitbringt.** Erst fehlte dem gebuendelten
    /// Weg das Ausgangstor der Achtsamkeit (Fund 426), dann der geteilte
    /// Experte. Beide Male war die alte Fassung vollstaendig **fuer die
    /// Modelle, die es damals gab**. Deshalb steht die Rechnung jetzt
    /// einmal da und hat zwei Aufrufer.
    #[allow(clippy::too_many_arguments)]
    fn geteilten_experten_addieren(
        &self,
        moe: &MoeLayer,
        x: &[i16],
        gemischt: Vec<i16>,
        sc: &LayerScales,
        cfg: &ModelConfig,
        acc: &[u8],
    ) -> Vec<i16> {
        let Some(ge) = moe.geteilter_experte.as_ref() else {
            return gemischt;
        };
        // ⛔️ **Fund 429: hier stand ein `let`, dessen einzige Wirkung
        //   ein Abbruch war.**
        //
        // `let masse = self.zustandsmasse.as_ref().expect("ein geteilter
        // Experte braucht die Sigmoid-Tabelle, und die haengt an der
        // Zustandsmasse")`. Die Begruendung stimmte nicht: Die Tabelle
        // haengt an `self`, wie die drei Zeilen darunter zeigen, und
        // `masse` wurde nie gelesen.
        //
        // ⚠️ **Die Wirkung war eine Kopplung, die es nicht gibt.** Ein
        // Modell mit geteiltem Experten, aber **ohne** rekurrente Ebenen
        // waere auf jeder Gemischebene abgebrochen. Beim Qwen3.6 fiel es
        // nicht auf, weil er beides hat.
        //
        // 📌 **Eine Bindung, die niemand liest, ist entweder ueberfluessig
        // oder eine versteckte Vorbedingung.** Hier war sie beides: Sie
        // stand da wie eine Beschaffung und wirkte wie eine Zusicherung,
        // und die Zusicherung war falsch.
        //
        // ⚑ Nebenwirkung: Eine Testvorlage mit geteiltem Experten
        // braucht jetzt keine Zustandsschicht mehr.

        // Das Tor: eine Zeile, also ein Wert je Token.
        let tor_roh = integer_llm_kernels::linear::linear_w8a16(
            x, &ge.tor.data, x.len(),
            &ge.tor.shifts, sc.norm_mlp_frac, ge.tor_frac,
        );
        let tor_dom = integer_llm_kernels::fixed_point::rescale(
            i32::from(tor_roh[0]), ge.tor_frac, self.sigmoid_ein_frac,
        );
        let tor = integer_llm_kernels::integer_math::sigmoid_nachschlagen(
            tor_dom, &self.sigmoid_lut, self.sigmoid_versatz,
            self.sigmoid_ein_frac, self.sigmoid_aus_frac,
        );
        // Der geteilte Experte selbst, dieselbe Rechnung wie eine dichte MLP.
        let geteilt = integer_llm_kernels::mlp::mlp_int(
            x,
            &ge.mlp.gate_proj.data,
            &ge.mlp.up_proj.data,
            &ge.mlp.down_proj.data,
            x.len(),
            ge.zwischen,
            &ge.mlp.gate_proj.shifts,
            &ge.mlp.up_proj.shifts,
            &ge.mlp.down_proj.shifts,
            &self.silu_lut,
            sc.norm_mlp_frac,
            ge.gate_frac,
            ge.up_frac,
            ge.down_in_frac,
            cfg.silu_in_frac,
            cfg.silu_lut_offset,
            cfg.silu_out_frac,
            acc,
        );
        // ⛔️ `zip` bricht an der kuerzeren Seite ab (Fund 346); beide
        //    sind hidden_size lang, und genau das wird geprueft.
        assert_eq!(
            gemischt.len(),
            geteilt.len(),
            "Gemisch und geteilter Experte verschieden lang"
        );
        gemischt
            .iter()
            .zip(geteilt.iter())
            .map(|(&g, &s)| {
                let beitrag = integer_llm_kernels::fixed_point::rshift_round_i64(
                    i64::from(s) * tor,
                    self.sigmoid_aus_frac,
                );
                integer_llm_kernels::fixed_point::clamp_i16_from_i64(
                    i64::from(g) + beitrag,
                )
            })
            .collect()
    }


    fn split_heads(&self, flat: &[i16], n: usize) -> Vec<Vec<i16>> {
        let mut heads = Vec::with_capacity(n);
        for h in 0..n {
            let start = h * self.head_dim;
            let end = start + self.head_dim;
            heads.push(flat[start..end].to_vec());
        }
        heads
    }


    /// Greedy Decoding fuer ein Token.
    pub fn greedy_next(&self, logits: &[i32]) -> usize {
        argmax_int(logits)
    }

    /// Sampling mit deterministischem Seed.
    pub fn sample_next(&self, logits: &[i32], seed: u64) -> (usize, u64) {
        sample_integer_cdf(logits, seed)
    }
}

/// Wie viele Faeden die Experten einer Gemischebene gleichzeitig beim
/// System ankuendigen.
///
/// # 📌 Fund 371, zweiter Teil (2026-09-14): die Vorbereitung wartete auf die Platte
///
/// Nach der Gruppierung verbrachte die Vorbereitung des 30B (368 Token,
/// `metal`) **56 % des Hauptfadens in `madvise`**, und in acht Sekunden
/// kamen 14,8 GB von der SSD: 29 GB Expertengewichte passen nicht in den
/// Dateicache einer Maschine mit 24 GiB, also liest jede lange Vorbereitung
/// die Experten jeder Ebene neu. Der Rat nacheinander liess die SSD einen
/// Experten nach dem anderen holen. Gemessen, bitgleich:
///
/// | Faeden | 1 | 4 | 8 | 16 | 32 |
/// |---|---|---|---|---|---|
/// | Vorbereitung | 8,45 s | 5,65 s | 5,16 s | **5,04 s** | 5,05 s |
///
/// ⚑ **Sechzehn, weil die Kurve dort flach wird.** Die naechste Ebene
/// schon waehrend der Rechnung vorzuladen, brachte gemessen nichts
/// zusaetzlich (5,21 s) und liest bei kurzen Prompts Experten, die niemand
/// braucht; es ist deshalb nicht gebaut. **Im Decode bleibt der Rat
/// seriell**: Acht Experten je Ebene sind zu wenige, um einen Fadenstart je
/// Token und Ebene zu tragen.
const ANKUENDIGUNGSFAEDEN: usize = 16;

/// Wie viele Faeden die verteilten Schritte der Vorbereitung nehmen.
///
/// ⚑ **Die Kerngrenze des Nutzers, sobald es mehr als ein Token ist.** Die
/// Schritte rechnen je Token ein Vielfaches einer Matrixzeile (eine
/// Aufmerksamkeit ueber alle Positionen davor, eine Norm, eine Drehung),
/// und eine Poolrunde kostet einmal je Schritt und Ebene, nicht je Token.
/// **An keiner Zahl aendert die Fadenzahl etwas**: Jedes Token schreibt in
/// seinen eigenen Abschnitt.
/// Ab wie vielen sichtbaren Werten (Positionen mal Koepfe mal Kopfbreite)
/// die Koepfe eines Tokens verteilt rechnen.
///
/// ⚑ **Gemessen und nicht geschaetzt**, siehe den Kommentar an der
/// Aufrufstelle in `aufmerksamkeit_lesen`.
const KOEPFE_VERTEILEN_AB: usize = 1 << 16;

fn vorbereitungsfaeden(n: usize) -> usize {
    if n < 2 {
        1
    } else {
        integer_llm_kernels::linear::kerngrenze()
    }
}

/// Wie viele Prompt-Token eine gebuendelte Vorbereitung hoechstens
/// zugleich nimmt.
///
/// ⚑ **Eine Speichergrenze, keine Rechengrenze.** Die Vorbereitung haelt
/// je Ebene alle Zwischenwerte aller Token ihres Fensters: beim 4B-Modell
/// rund 104 KB je Token (Strom, Normen, q, k, v, Aufmerksamkeit, o, gate,
/// up, Produkt, down). 512 Token sind damit 53 MB; ein Prompt von 32 000
/// Token ohne Fenster waeren 3,3 GB, und das auf einer Maschine, deren
/// Gewichte schon im Speicher liegen.
///
/// **An keiner Zahl aendert das Fenster etwas.** Jedes Token rechnet
/// dieselbe Rechnung, und der KV-Speicher wird in derselben Reihenfolge
/// gefuellt; das naechste Fenster beginnt bei der Position, an der das
/// vorige aufgehoert hat.
pub const VORBEREITUNGSFENSTER: usize = 512;

/// Die Akkumulationsskala der ersten Residualaddition, je Kanal.
///
/// O-Projektion: Eingangsskala ist die Attention-Ausgabe, Ausgang auf der
/// Skala des mittleren Residual-Segments (vor der zweiten Norm).
/// Fund 20: per-Kanal-Ziel statt Skalar, o_proj addiert direkt in den
/// Residualstrom. Fund 31 (theta_v 0.17.0): Akkumulationsskala je Kanal
/// ist die GROEBERE der beiden Segmentskalen (kleinerer Shift). Der Grund
/// steht beim zweiten Residual-Add, dort trat der Fehler auf.
/// ⚑ **Die Skala gilt fuer beide Mischer.** Sie haengt nur an den
/// beiden Residualsegmenten, nicht daran, was dazwischen gerechnet hat.
/// Bis zum 2026-09-21 hiess sie `..._aufmerksamkeit`, und das waere ab
/// der ersten Zustandsebene ein Name gewesen, der luegt.
fn akkumulationsskala_mischer(sc: &LayerScales) -> Vec<u8> {
    sc.residual_in_frac
        .iter()
        .zip(sc.residual_mid_frac.iter())
        .map(|(&a, &b)| a.min(b))
        .collect()
}

/// W8A16 fuer eine oder viele Eingaben auf denselben Gewichten.
///
/// ⚑ **Eine Eingabe geht den einzelnen Weg**, und zwar Zeichen fuer
/// Zeichen den, der vor Fund 366 hier stand: Der Decode rechnet je Schritt
/// genau ein Token und soll keinen Buendelrumpf bezahlen. Viele Eingaben
/// gehen den Stapelweg; beide sind elementweise dasselbe Skalarprodukt,
/// geprueft in `linear.rs` (`gebuendelt_ist_dasselbe`).
fn linear_fuer_alle(
    xs: &[&[i16]],
    w: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: u8,
) -> Vec<Vec<i16>> {
    match xs {
        [x] => vec![linear_w8a16(x, w, in_features, w_shifts, act_frac_bits, out_frac_bits)],
        _ => linear_w8a16_stapel(xs, w, in_features, w_shifts, act_frac_bits, out_frac_bits),
    }
}

/// Wie [`linear_fuer_alle`], mit einer Ausgangsskala je Kanal (Fund 20).
fn linear_pc_fuer_alle(
    xs: &[&[i16]],
    w: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    out_frac_bits: &[u8],
) -> Vec<Vec<i16>> {
    match xs {
        [x] => vec![linear_w8a16_pc(x, w, in_features, w_shifts, act_frac_bits, out_frac_bits)],
        _ => linear_w8a16_pc_stapel(xs, w, in_features, w_shifts, act_frac_bits, out_frac_bits),
    }
}

// ============================================================================
// Der Vorwaertspass einer rekurrenten Zustandsschicht
// ============================================================================

impl IntegerModel {
    /// **`aus * sigmoid(tor)`, elementweise.**
    ///
    /// ⚑ Beide Seiten liegen auf der Achtsamkeits-Ausgangsskala; das
    /// Tor bringt nur einen Faktor in `(0, 1)` und aendert die Skala
    /// nicht.
    fn tor_anwenden(&self, aus: &[i16], tor: &[i16], layer: &TransformerLayer) -> Vec<i16> {
        use integer_llm_kernels::fixed_point::{clamp_i16_from_i64, rescale, rshift_round_i64};
        use integer_llm_kernels::integer_math::sigmoid_nachschlagen;
        // ⛔️ `zip` bricht an der kuerzeren Seite ab (Fund 346).
        assert_eq!(aus.len(), tor.len(), "Ausgang und Tor verschieden lang");
        // ⛔️ **Fund 422: das Tor traegt die Skala von `q_proj`, nicht
        //   die des Achtsamkeitsausgangs.**
        //
        // Hier stand `attn_out_frac`. Das Tor kommt aber aus
        // `abfrage_und_tor_trennen`, und das teilt den **rohen
        // `q_proj`-Ausgang**; `q_norm` fasst nur die Abfragehaelfte an.
        // Beim Qwen3.6-35B-A3B sind das auf Ebene 3 die Schiebungen 10
        // gegen 14, also **vier Bit**.
        //
        // ⚠️ **Und der Fehler zerstoert nicht, er glaettet**, was ihn
        // schwerer sichtbar macht: Ein um 16 zu klein gelesenes
        // Argument macht aus `sigmoid` im Bereich ±16, also einem
        // nahezu binaeren Schalter, eine sanfte Schwankung zwischen
        // 0,27 und 0,73. Das Tor hoert damit auf, Koepfe abzuschalten,
        // und wirkt nur noch wie ein Faktor um ein halb.
        //
        // 📌 **Ein Tor, das nicht mehr schaltet, sieht aus wie ein Tor.**
        // Die Ausgabe behaelt Groessenordnung und Vorzeichen; nur die
        // Auswahl faellt weg. Eine Probe auf „ist der Betrag
        // plausibel" haette das nie gefunden.
        let tor_frac = layer.scales.achtsamkeit().q_frac;
        aus.iter()
            .zip(tor.iter())
            .map(|(&a, &g)| {
                let dom = rescale(i32::from(g), tor_frac, self.sigmoid_ein_frac);
                let s = sigmoid_nachschlagen(
                    dom, &self.sigmoid_lut, self.sigmoid_versatz,
                    self.sigmoid_ein_frac, self.sigmoid_aus_frac,
                );
                clamp_i16_from_i64(rshift_round_i64(
                    i64::from(a) * s,
                    self.sigmoid_aus_frac,
                ))
            })
            .collect()
    }

    /// **Eine rekurrente Zustandsschicht fuer ein Token.**
    ///
    /// Sie ersetzt in einer Ebene genau das, was sonst die Achtsamkeit
    /// tut: Aus dem normierten Residualstrom wird ein Beitrag, der
    /// zurueckaddiert wird. Norm davor und Residuum danach sind
    /// dieselben.
    ///
    /// # Die sieben Schritte
    ///
    /// ```text
    /// 1. Projektionen   qkv, z, a, b        aus dem normierten Strom
    /// 2. Faltung        kausal, 4 Stellen, tiefenweise, dann SiLU
    /// 3. Aufteilen      q | k | v
    /// 4. Normieren      q und k auf Einheitslaenge, q zusaetzlich /sqrt(d)
    /// 5. Tore           beta = sigmoid(b), g = exp(-exp_A*softplus(a+dt))
    /// 6. Rekurrenz      je Wertkopf ein Schritt auf seinem Zustand
    /// 7. Ausgangstor    Norm mal silu(z), dann out_proj
    /// ```
    ///
    /// # ⚑ Die Auffaecherung ist Indexierung und kein Kopieren
    ///
    /// Es gibt 16 Schluesselkoepfe und 32 Wertkoepfe. Die Vorlage
    /// verdoppelt `q` und `k` mit `repeat_interleave`; hier liest der
    /// Wertkopf `h` einfach den Schluesselkopf `h / 2`. **Dieselbe
    /// Rechnung ohne die Kopie.**
    #[allow(clippy::too_many_arguments)]
    fn zustandsschicht_eines_tokens(
        &self,
        layer: &TransformerLayer,
        zs: &Zustandsschicht,
        masse: &Zustandsmasse,
        norm_hidden: &[i16],
        speicher: &mut crate::zustandsspeicher::Zustandsspeicher,
        out_frac: &[u8],
    ) -> Vec<i16> {
        use integer_llm_kernels::integer_math::{
            sigmoid_nachschlagen, softplus_nachschlagen, zerfall_nachschlagen,
        };
        use integer_llm_kernels::fixed_point::{clamp_i16_from_i64, rescale, rescale_i64};
        use integer_llm_kernels::linear::linear_w8a16;
        use integer_llm_kernels::zustandsschicht as zk;

        let sk = &zs.skalen;
        let ebene = layer.layer_idx;
        let ein_frac = layer.scales.norm_attn_frac;
        let hidden = norm_hidden.len();

        // --- 1. Die vier Projektionen.
        let mut qkv = linear_w8a16(
            norm_hidden, &zs.in_proj_qkv.data, hidden,
            &zs.in_proj_qkv.shifts, ein_frac, sk.qkv_frac,
        );
        let z = linear_w8a16(
            norm_hidden, &zs.in_proj_z.data, hidden,
            &zs.in_proj_z.shifts, ein_frac, sk.z_frac,
        );
        let b_roh = linear_w8a16(
            norm_hidden, &zs.in_proj_b.data, hidden,
            &zs.in_proj_b.shifts, ein_frac, sk.b_frac,
        );
        let a_roh = linear_w8a16(
            norm_hidden, &zs.in_proj_a.data, hidden,
            &zs.in_proj_a.shifts, ein_frac, sk.a_frac,
        );

        // --- 2. Die kausale Faltung, tiefenweise mit SiLU.
        let mut gefaltet = vec![0i16; masse.kanaele];
        integer_llm_kernels::faltung::schritt(
            speicher.fenster_mut(ebene),
            &qkv,
            &zs.conv1d.data,
            &zs.conv1d.shifts,
            sk.qkv_frac,
            // ⛔️ **Die Sigmoid-Tabelle und nicht die SiLU-Tabelle**
            //   (Fund 417): `silu_zerlegt` braucht nur den Faktor.
            &self.sigmoid_lut,
            self.sigmoid_versatz,
            self.sigmoid_ein_frac,
            self.sigmoid_aus_frac,
            &sk.konv_fracs,
            &mut gefaltet,
        );
        std::mem::swap(&mut qkv, &mut gefaltet);

        // --- 3. Aufteilen in q, k, v.
        let k_breite = masse.schluessel_dim * masse.schluessel_koepfe;
        let (q_teil, rest) = qkv.split_at(k_breite);
        let (k_teil, v_teil) = rest.split_at(k_breite);

        // --- 4. Auf Einheitslaenge, je Schluesselkopf.
        //
        // ⚑ **Die vorhandene RMSNorm leistet beides.** Sie rechnet
        // `x * rsqrt(Summe(x^2) * inv_n)`; mit `inv_n = 1` ist das genau
        // die L2-Normierung, mit `inv_n = d` ist es dieselbe mal
        // `1/sqrt(d)`, und das ist die Skalierung, die `q` braucht.
        // **Kein zweiter Kern fuer etwas, das der erste schon kann.**
        let q_norm = einheitslaenge(
            q_teil, masse.schluessel_dim, &sk.konv_fracs[..k_breite],
            (masse.schluessel_dim as i64) << 20, self,
            zk::NORM_FRAC as u8,
        );
        let k_norm = einheitslaenge(
            k_teil, masse.schluessel_dim, &sk.konv_fracs[k_breite..2 * k_breite],
            1i64 << 20, self,
            zk::NORM_FRAC as u8,
        );

        // --- 5. Die beiden Tore je Wertkopf.
        let mut beta = vec![0i16; masse.wert_koepfe];
        let mut zerfall = vec![0i64; masse.wert_koepfe];
        for h in 0..masse.wert_koepfe {
            // beta = sigmoid(b), in WERT_FRAC.
            let b_dom = rescale(
                i32::from(b_roh[h]), sk.b_frac, self.sigmoid_ein_frac,
            );
            let s = sigmoid_nachschlagen(
                b_dom, &self.sigmoid_lut, self.sigmoid_versatz,
                self.sigmoid_ein_frac, self.sigmoid_aus_frac,
            );
            beta[h] = clamp_i16_from_i64(rescale_i64(
                s, self.sigmoid_aus_frac, zk::WERT_FRAC as u8,
            ));

            // d = exp_A * softplus(a + dt_bias), dann g = exp(-d).
            let a_dom = rescale(
                i32::from(a_roh[h]), sk.a_frac, masse.softplus_ein_frac,
            );
            let dt = vorspannwert(&zs.dt_bias, h, masse.softplus_ein_frac);
            let sp = softplus_nachschlagen(
                a_dom + dt, &masse.softplus_rest,
                masse.softplus_ein_frac, masse.softplus_aus_frac,
            );
            let d = mal_vorspann(sp, &zs.exp_a, h);
            zerfall[h] = zerfall_nachschlagen(
                d, &masse.zerfall_exp, masse.softplus_aus_frac as u32,
                masse.zerfall_raster_frac as u32, masse.zerfall_aus_frac as u32,
            );
        }

        // --- 5b. `v` auf die Wertauflösung bringen.
        //
        // ⛔️ **Hier fehlte bis zum 2026-09-22 die Umrechnung**, und sie
        // hat einen Abend gekostet. Die Faltung liefert auf
        // `konv_frac` (bei Ebene 0 des grossen Modells **15**), die
        // Rekurrenz erwartet `WERT_FRAC` (**8**). `v` ging damit um den
        // Faktor 128 zu gross hinein.
        //
        // 📌 **Drei von vier Eingaengen waren richtig skaliert.** `q`
        // und `k` kommen ueber `einheitslaenge` auf `NORM_FRAC`, `beta`
        // wird ausdruecklich reskaliert, und `v` wurde durchgereicht.
        // Der Kopf von `zustandsschicht` sagt es woertlich: „`v` und
        // `beta` in `2^-WERT_FRAC`". **Ich habe die Zeile geschrieben
        // und dann nicht befolgt.**
        //
        // ⚠️ **Warum es nicht auffiel:** Die Ausgabe hatte die richtige
        // Groessenordnung, weil die torgesteuerte Norm dahinter
        // skaleninvariant ist; nur die Verhaeltnisse zwischen den
        // Koepfen stimmten nicht mehr. Ein zu grosser Faktor sieht
        // harmloser aus als ein falsches Vorzeichen.
        let v_wert: Vec<i16> = v_teil
            .iter()
            .zip(sk.konv_fracs[2 * k_breite..].iter())
            .map(|(&x, &f)| {
                clamp_i16_from_i64(rescale_i64(i64::from(x), f, zk::WERT_FRAC as u8))
            })
            .collect();

        // ⚑ **Die Eingaenge der Rekurrenz, fuer die Halbierung.** Der
        //   Nachbau in Python trifft mit denselben Gewichten 0,13; die
        //   Laufzeit liefert 0,34. Der Unterschied muss in einem dieser
        //   drei Felder sichtbar werden, sonst liegt er im Kern.
        if std::env::var_os("MYL_ZUSTANDSSPUR").is_some() {
            let zeig = |name: &str, f: &[i16]| {
                eprintln!(
                    "[feld] {name} {}",
                    f.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
                );
            };
            zeig("konv", &qkv);
            zeig("qnorm", &q_norm);
            zeig("knorm", &k_norm);
            zeig("vwert", &v_wert);
        }

        // --- 6. Die Rekurrenz, je Wertkopf ein Schritt.
        let faecher = masse.auffaecherung();
        let mut roh_aus = vec![0i16; masse.wert_koepfe * masse.wert_dim];
        // ⚑ **Eine Skala je Kopf** (Fund 418): `schritt` waehlt sie aus
        //   dem Groesstwert des Kopfes und gibt sie zurueck.
        let mut roh_fracs = vec![0u8; masse.wert_koepfe];
        for h in 0..masse.wert_koepfe {
            let s_kopf = h / faecher;
            let q = &q_norm[s_kopf * masse.schluessel_dim..(s_kopf + 1) * masse.schluessel_dim];
            let k = &k_norm[s_kopf * masse.schluessel_dim..(s_kopf + 1) * masse.schluessel_dim];
            let v = &v_wert[h * masse.wert_dim..(h + 1) * masse.wert_dim];
            let ziel = &mut roh_aus[h * masse.wert_dim..(h + 1) * masse.wert_dim];
            roh_fracs[h] = zk::schritt(
                speicher.zustand_mut(ebene, h), q, k, v,
                zerfall[h], beta[h], ziel,
            );
        }

        if std::env::var_os("MYL_ZUSTANDSSPUR").is_some() {
            let nz = roh_aus.iter().filter(|&&x| x == 0).count();
            let je_kopf: Vec<usize> = (0..masse.wert_koepfe)
                .map(|h| {
                    roh_aus[h * masse.wert_dim..(h + 1) * masse.wert_dim]
                        .iter()
                        .filter(|&&x| x != 0)
                        .count()
                })
                .collect();
            eprintln!("[spur] nicht-null je Kopf: {je_kopf:?}");
            // ⚑ Die Skalen je Kopf, sonst liest die Python-Seite die
            //   Zahlen mit der falschen Skala (Fund 418).
            eprintln!(
                "[feld] rohfrac {}",
                roh_fracs.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
            );
            // ⚑ Die ganzen Felder, damit die Python-Seite sie gegen die
            //   Referenz halten kann statt nur ihr Maximum.
            eprintln!(
                "[feld] roh_aus {}",
                roh_aus.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
            );
            eprintln!(
                "[spur] beta: {:?}",
                &beta[..beta.len().min(8)]
            );
            eprintln!(
                "[spur] roh_aus: max {} nullen {}/{} | z max {} | v_wert max {}",
                roh_aus.iter().map(|x| x.abs()).max().unwrap_or(0),
                nz, roh_aus.len(),
                z.iter().map(|x| x.abs()).max().unwrap_or(0),
                v_wert.iter().map(|x| x.abs()).max().unwrap_or(0)
            );
        }

        // --- 7. Das Ausgangstor und die Ruecktransformation.
        let getort = torgesteuerte_norm(
            &roh_aus, &z, zs, masse, self,
            sk.norm_aus_frac, &roh_fracs, sk.z_frac,
        );
        if std::env::var_os("MYL_ZUSTANDSSPUR").is_some() {
            eprintln!(
                "[feld] getort {}",
                getort.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
            );
        }
        // ⚑ **Per Kanal**, wie `projektion_o` bei der Achtsamkeit.
        integer_llm_kernels::linear::linear_w8a16_pc(
            &getort, &zs.out_proj.data, getort.len(),
            &zs.out_proj.shifts, sk.norm_aus_frac, out_frac,
        )
    }
}

/// **Ein Wert aus einem int16-Vorspann**, auf eine Zielskala gebracht.
///
/// ⚑ `exp_A` und `dt_bias` sind keine GEMM-Gewichte, sondern je ein Wert
/// pro Wertkopf, und tragen deshalb eine Skala **je Element**.
#[inline]
fn vorspannwert(vorspann: &crate::loader::BiasTensor, i: usize, ziel_frac: u8) -> i32 {
    use integer_llm_kernels::fixed_point::rescale;
    rescale(i32::from(vorspann.data[i]), vorspann.shifts[i], ziel_frac)
}

/// **`exp_A[h] * wert`**, ohne die Skala des Ergebnisses zu verschieben.
///
/// ⚑ `exp_A` liegt als `data * 2^-shift` vor; das Produkt behaelt damit
/// die Bruchbits von `wert`, wenn um `shift` nach rechts geschoben wird.
#[inline]
fn mal_vorspann(wert: i64, vorspann: &crate::loader::BiasTensor, i: usize) -> i64 {
    use integer_llm_kernels::fixed_point::rshift_round_i128;
    rshift_round_i128(
        i128::from(vorspann.data[i]) * i128::from(wert),
        u32::from(vorspann.shifts[i]),
    ) as i64
}

/// **Je Kopf auf Einheitslaenge**, ueber die vorhandene RMSNorm.
///
/// ⚑ **Kein zweiter Kern.** `rmsnorm_i16` rechnet
/// `x * rsqrt(Summe(x^2) * inv_n)`. Mit `inv_n = 1` ist das die
/// L2-Normierung; mit `inv_n = d` ist es dieselbe mal `1/sqrt(d)`, und
/// genau das braucht `q`. Gamma ist eins.
///
/// 📌 **Eine Funktion, die der Rechenpfad schon hat und die die
/// Konformitaetsvektoren abdecken, ist mehr wert als eine neue, die
/// dasselbe tut.**
fn einheitslaenge(
    x: &[i16],
    kopf_dim: usize,
    ein_fracs: &[u8],
    inv_n_q20: i64,
    m: &IntegerModel,
    aus_frac: u8,
) -> Vec<i16> {
    use integer_llm_kernels::rmsnorm::rmsnorm_i16;
    let koepfe = x.len() / kopf_dim;
    debug_assert_eq!(x.len() % kopf_dim, 0, "die Breite passt nicht zur Kopfgroesse");
    // Gamma eins, je Element: `1 * 2^-0`.
    let gamma = vec![1i8; kopf_dim];
    let gamma_shifts = vec![0u8; kopf_dim];
    debug_assert_eq!(ein_fracs.len(), x.len(), "eine Eingangsskala je Kanal (Fund 421)");
    let mut aus = Vec::with_capacity(x.len());
    for h in 0..koepfe {
        let kopf = &x[h * kopf_dim..(h + 1) * kopf_dim];
        // ⚑ **Die Skalen je Kanal gehen unveraendert weiter.**
        //   `rmsnorm_i16` richtet sie seit Fund 24 gegen die groesste
        //   aus, und zwar per Linksshift, also verlustfrei.
        let x_shifts = &ein_fracs[h * kopf_dim..(h + 1) * kopf_dim];
        aus.extend_from_slice(&rmsnorm_i16(
            kopf, x_shifts, &gamma, &gamma_shifts,
            &m.rsqrt_lut, m.config.rsqrt_input_shift, m.config.rsqrt_output_frac,
            inv_n_q20, aus_frac,
        ));
    }
    aus
}

/// **Das Ausgangstor: Norm je Wertkopf, dann mal `silu(z)`.**
///
/// ⚑ **Norm zuerst, Tor danach**, wie die Vorlage. Beides gibt es
/// schon: die RMSNorm und `silu_produkt` aus dem MLP.
// ⚑ **Acht Argumente, und jedes ist eine eigene Skala.** Sie in einen
//   Sammeltyp zu packen verschoebe die Frage nur; hier steht jede
//   Angabe an der Stelle, an der der Leser sie braucht.
#[allow(clippy::too_many_arguments)]
fn torgesteuerte_norm(
    roh: &[i16],
    z: &[i16],
    zs: &Zustandsschicht,
    masse: &Zustandsmasse,
    m: &IntegerModel,
    aus_frac: u8,
    ein_fracs: &[u8],
    z_frac: u8,
) -> Vec<i16> {
    use integer_llm_kernels::mlp::silu_produkt;
    use integer_llm_kernels::rmsnorm::{inv_n_q20, rmsnorm_i16_mit_eps};
    let d = masse.wert_dim;
    debug_assert_eq!(roh.len(), z.len(), "Rohausgabe und Tor verschieden lang");
    debug_assert_eq!(ein_fracs.len(), masse.wert_koepfe, "eine Eingangsskala je Wertkopf");
    let inv_n = inv_n_q20(d);

    // ⛔️ **Fund 420: die Zwischenstufe braucht eine eigene Skala.**
    //
    // `aus_frac` kommt aus `scales.json` fuer `linear_attn.norm` und
    // stand dort mit `absmax_observed = 2.078`. Diese Zahl ist richtig
    // **fuer das, was gemessen wurde**: Die Vorlage ruft das Modul als
    // `norm(core_attn_out, z)` auf, es normiert und **torrt in einem**,
    // und der Haken sieht deshalb den Ausgang *hinter* dem Tor.
    //
    // ⚠️ **Benutzt wurde sie fuer den Zwischenstand davor**, und der ist
    // eine ganz andere Groesse: Nach der Normierung ist der Effektivwert
    // eins, einzelne Kanaele liegen aber weit darueber. Gemessen am
    // Qwen3.6-35B-A3B, Ebene 0: bis **6,37**, waehrend `frac = 13` in
    // i16 nur bis 4,0 reicht. Vierzig Werte je Token liefen in die
    // Saettigung, und es waren die groessten.
    //
    // 📌 **Eine kalibrierte Skala gilt an der Stelle, an der gemessen
    // wurde.** Wer sie eine Stufe frueher einsetzt, benutzt eine Zahl,
    // die etwas anderes beschreibt, und niemand meldet es.
    //
    // ⚑ **Hier ist keine Messung noetig, es gibt eine harte Schranke.**
    // Nach der Normierung gilt `|x_i| / rms(x) <= sqrt(d)`, denn der
    // schlimmste Fall ist der, in dem ein einziger Kanal die ganze
    // Energie traegt. Mal dem groessten Gamma ist das die Schranke, und
    // sie klemmt nie.
    let zwischen_frac = normskala(d, &zs.norm_gamma);

    let mut normiert = Vec::with_capacity(roh.len());
    for h in 0..masse.wert_koepfe {
        // ⚑ **Je Kopf seine eigene Eingangsskala** (Fund 418). Die
        //   RMSNorm ist in `x` skaleninvariant, die Skala aendert das
        //   Ergebnis also nicht; sie muss nur **stimmen**, damit die
        //   Angabe im Code nicht luegt.
        let x_shifts = vec![ein_fracs[h]; d];
        let kopf = &roh[h * d..(h + 1) * d];
        normiert.extend_from_slice(&rmsnorm_i16_mit_eps(
            kopf, &x_shifts, &zs.norm_gamma.data, &zs.norm_gamma.shifts,
            &m.rsqrt_lut, m.config.rsqrt_input_shift, m.config.rsqrt_output_frac,
            inv_n, zwischen_frac,
            // ⛔️ **Hier traegt das Epsilon** (Fund 419): Ein Wertkopf,
            //   der nichts zu sagen hat, ist wirklich fast null, und
            //   ohne Epsilon zieht die Norm ihn auf volle Hoehe.
            m.norm_eps_q40,
        ));
    }
    if std::env::var_os("MYL_ZUSTANDSSPUR").is_some() {
        let satt = normiert.iter().filter(|&&x| x == i16::MAX || x == i16::MIN).count();
        eprintln!(
            "[spur] normiert: |max| {} gesaettigt {}/{}",
            normiert.iter().map(|x| x.abs()).max().unwrap_or(0),
            satt, normiert.len()
        );
        eprintln!(
            "[feld] normiert {}",
            normiert.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
        );
    }
    // ⚑ `silu_produkt(tor, faktor)` rechnet `silu(tor) * faktor`, also
    //   hier `silu(z) * normiert`. Die Laengenpruefung darin ist seit
    //   dem 2026-09-21 da, und zwar fuer genau diesen zweiten Aufrufer.
    silu_produkt(
        z, &normiert, &m.silu_lut,
        z_frac, zwischen_frac, aus_frac,
        m.config.silu_in_frac, m.config.silu_lut_offset, m.config.silu_out_frac,
    )
}

/// **Die Skala des normierten Zwischenstands**, aus der harten
/// Schranke `sqrt(d) * max|gamma|` (Fund 420).
///
/// ⚑ **Ohne Wurzel und ohne Gleitkomma.** Gesucht ist das groesste
/// `f` mit `sqrt(d) * g * 2^f <= i16::MAX`; quadriert ist das
/// `d * g^2 * 2^(2f) <= i16::MAX^2`, und damit bleibt alles ganzzahlig.
fn normskala(d: usize, gamma: &QTensor) -> u8 {
    // Das groesste Gamma in Q20.
    let g_q20 = gamma
        .data
        .iter()
        .zip(gamma.shifts.iter())
        .map(|(&w, &s)| (i64::from(w).abs() << 20) >> s)
        .max()
        .unwrap_or(1 << 20)
        .max(1);
    let grenze = i128::from(i16::MAX) * i128::from(i16::MAX);
    let dg2 = i128::from(d as i64) * i128::from(g_q20) * i128::from(g_q20);
    // `dg2` traegt 2^40; gesucht ist f mit `dg2 * 2^(2f) >> 40 <= grenze`.
    for f in (0..=15u8).rev() {
        if (dg2 << (2 * u32::from(f))) >> 40 <= grenze {
            return f;
        }
    }
    0
}

/// **Abfrage und Tor aus einer `q_proj`-Ausgabe trennen.**
///
/// Beim `Qwen3.6-35B-A3B` liefert `q_proj` die **doppelte** Kopfbreite:
/// je Kopf erst `head_dim` Abfragewerte, dann `head_dim` Torwerte. Der
/// Achtsamkeitsausgang wird spaeter mit `sigmoid(tor)` multipliziert,
/// bevor `o_proj` rechnet.
///
/// # ⚠️ Die Trennung geschieht JE KOPF, nicht in der Mitte
///
/// Die Vorlage formt `q_proj(x)` zu `[..., koepfe, 2*head_dim]` um und
/// teilt dann die letzte Achse. Die Anordnung ist also
/// `[q_kopf0, tor_kopf0, q_kopf1, tor_kopf1, ...]` und **nicht**
/// `[alle q, alle tore]`.
///
/// 📌 **Eine Aufteilung „in zwei Haelften" waere hier fast richtig
/// gewesen**, und zwar auf eine Weise, die bei einem einzigen Kopf gar
/// nicht auffaellt.
fn abfrage_und_tor_trennen(roh: &[i16], koepfe: usize, kopf_dim: usize) -> (Vec<i16>, Vec<i16>) {
    debug_assert_eq!(
        roh.len(),
        koepfe * kopf_dim * 2,
        "q_proj liefert nicht die doppelte Kopfbreite"
    );
    let mut abfrage = Vec::with_capacity(koepfe * kopf_dim);
    let mut tor = Vec::with_capacity(koepfe * kopf_dim);
    for k in 0..koepfe {
        let basis = k * kopf_dim * 2;
        abfrage.extend_from_slice(&roh[basis..basis + kopf_dim]);
        tor.extend_from_slice(&roh[basis + kopf_dim..basis + 2 * kopf_dim]);
    }
    (abfrage, tor)
}
