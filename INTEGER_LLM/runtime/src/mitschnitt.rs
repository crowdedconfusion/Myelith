//! Was ein Vorwärtspass zurücklässt, damit ein Rückwärtspass rechnen
//! kann (TRAINING V, zweiter Teil).
//!
//! # ⚑ Warum es dieses Modul gibt
//!
//! `kernels::trainingsschritt` schliesst den Kreis für **eine** lineare
//! Ebene und sagt selbst, woran die Schleife über ein ganzes Netz
//! hängt: „Der Vorwärtspass der Laufzeit ist auf Inferenz zugeschnitten
//! und behält nichts."
//!
//! Jede Rückwärtsfunktion in `kernels::backward` braucht den **Eingang**
//! ihrer Vorwärtsentsprechung: `linear_backward` das `x` der Projektion,
//! `rmsnorm_backward` das `x` und die Gamma-Skalen, `attention_backward`
//! q, k und v nach RoPE. Ein Vorwärtspass, der nur seine Ausgabe
//! zurückgibt, macht den Rückwärtspass unmöglich.
//!
//! # ⚑ Ein Pfad mit Mitschnitt, nicht zwei Pfade
//!
//! **Entschieden am 2026-09-04.** `forward_layer` bekommt ein
//! `Option<&mut Zwischenwerte>`; Inferenz gibt `None` und ändert sich
//! nicht. **Der entscheidende Grund ist nicht Bequemlichkeit:** Genau
//! dieser Vorwärtspass ist über dreissig Konformitätsvektoren als
//! bitgleich belegt. Ein zweiter, der alles behält, wäre eine zweite
//! Wahrheit über den Rechenpfad, bräuchte eigene Vektoren und liefe
//! irgendwann auseinander.
//!
//! Das Muster steht ohnehin schon da: `forward_layer` trägt bereits ein
//! `Option<&mut Vec<Routingbefund>>` für die MoE-Diagnose.
//!
//! # ⚑ Was hier steht und was noch nicht
//!
//! **Zehn Werte je Ebene**, und damit alles, was der Rückwärtspass
//! einer dichten Ebene braucht. Sechs sieht `forward_layer` selbst;
//! die übrigen vier liegen in Kernen, die nur ihre Ausgabe
//! zurückgeben, und kommen seit dem 2026-09-04 über einen **zweiten
//! Eingang** dorthin: `attention_int_mit_spur` und `mlp_int_mit_spur`.
//!
//! ⚑ **Zweiter Eingang und nicht zusätzliches Argument**, denn beide
//! Kerne stehen im `Backend`-Merkmal in vier Umsetzungen. Ein Argument
//! mehr risse alle vier auf, für etwas, das nur das Training braucht.
//!
//! ⚑ **Was weiterhin fehlt, und es steht im Typ statt im Kommentar:**
//! ein Expertengemisch. [`Mlpteil::Expertengemisch`] sagt „hier wurde
//! nicht aufgezeichnet", und der Übersetzer zwingt jeden Leser, den
//! Fall zu behandeln. `moe_backward` verlangt andere Werte
//! (Expertenwahl, Gewichte, Expertenausgaben); das ist Phase 5.

/// Was eine Ebene an Zwischenwerten zurücklässt.
///
/// ⚑ **Alle Werte liegen so vor, wie der Rückwärtspass sie braucht**,
/// also nach der jeweiligen Umskalierung und nicht davor. Wer sie
/// später umrechnete, rechnete mit anderen Zahlen als der Vorwärtspass
/// und bekäme Gradienten zu einer Funktion, die so nie gelaufen ist.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ebenenmitschnitt {
    /// Der Residualstrom beim Eintritt: Eingang der ersten RMSNorm und
    /// zugleich der Summand der ersten Residualaddition.
    pub residual_ein: Vec<i16>,
    /// Der normierte Eingang: das `x` von `q_proj`, `k_proj` und
    /// `v_proj`.
    pub norm_ein: Vec<i16>,
    /// Die Q-Köpfe **nach** RoPE, wie die Aufmerksamkeit sie sah.
    pub q: Vec<Vec<i16>>,
    /// Die K-Köpfe nach RoPE.
    pub k: Vec<Vec<i16>>,
    /// Die V-Köpfe. ⚑ **Ohne RoPE**, denn V wird nicht gedreht.
    pub v: Vec<Vec<i16>>,
    /// Die Aufmerksamkeitsausgabe: das `x` von `o_proj`, bereits auf
    /// `attn_out_frac` umskaliert.
    pub attn_aus: Vec<i16>,
    /// Der Residualstrom nach der Aufmerksamkeit: Eingang der zweiten
    /// RMSNorm und Summand der zweiten Residualaddition.
    pub residual_mitte: Vec<i16>,
    /// Der zweite normierte Strom: das `x` des MLP-Blocks.
    pub norm_mitte: Vec<i16>,
    /// Die Aufmerksamkeitswahrscheinlichkeiten, **eine Zeile je Kopf**,
    /// auf `prob_frac_bits`.
    ///
    /// ⚑ **Das `p` von `softmax_backward`.** Die Ableitung des Softmax
    /// ist `p ⊙ (g − ⟨g, p⟩)`, rechnet also mit den
    /// Wahrscheinlichkeiten selbst. Sie nachzurechnen hiesse, die
    /// Punktprodukte ein zweites Mal zu bilden.
    pub wahrscheinlichkeiten: Vec<Vec<i32>>,
    /// Was der Feedforward-Zweig zurückgelassen hat.
    pub mlp: Mlpteil,
}

/// Der Feedforward-Teil eines Mitschnitts.
///
/// # ⚑ Warum ein Typ und kein leerer Vektor (2026-09-04)
///
/// Bis hierher standen `mlp_gate`, `mlp_up` und `mlp_h` als drei
/// Vektoren da, und für ein Expertengemisch blieben sie **leer**. Die
/// Regel „bei MoE ist der MLP-Teil nicht aufgezeichnet" stand im
/// Modulkopf, also in einem Kommentar.
///
/// ⚑ **Ein leerer Vektor sieht aus wie ein aufgezeichneter ohne
/// Inhalt.** Wer den Rückwärtspass baut, liest drei Vektoren, findet
/// sie leer und rechnet einen Gradienten aus nichts; nichts an der Form
/// sagt ihm, dass hier gar nicht aufgezeichnet **wurde**. Als Typ muss
/// er den Fall behandeln, und der Übersetzer besteht darauf.
///
/// Dieselbe Haltung wie an anderen Stellen dieses Projekts: **Die Regel
/// steht nicht im Kommentar, sie steht im Typ.**
///
/// ⚑ **Der Anlass war ein Versuch, das Training gegen ein
/// Expertengemisch zu prüfen.** Er gelingt: Die Gewichte sind
/// speicherabgebildet, also passt auch ein 29-GB-Artefakt auf eine
/// Maschine mit 24 GB. `runtime/tests/training_moe.rs` ist die einzige
/// Stelle im Baum, die den MoE-Zweig erreicht, und sie hält fest, dass
/// er `Expertengemisch` meldet statt drei leerer Vektoren.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Mlpteil {
    /// Noch nichts gerechnet.
    #[default]
    Leer,
    /// Eine dichte Einheit, vollständig aufgezeichnet.
    Dicht {
        /// Die Gate-Projektion **vor** der Aktivierung: das `x` von
        /// `silu_backward`.
        gate: Vec<i16>,
        /// Die Up-Projektion: der zweite Faktor des Produkts.
        up: Vec<i16>,
        /// Das Produkt `silu(gate) · up`: das `x` von `down_proj`.
        h: Vec<i16>,
    },
    /// Ein Expertengemisch, **nicht aufgezeichnet**.
    ///
    /// ⚑ `moe_backward` verlangt andere Werte als eine dichte Einheit:
    /// Expertenwahl, Gewichte und Expertenausgaben. Sie hier
    /// mitzuschneiden ist eigene Arbeit und steht als Phase 5 im
    /// Fahrplan.
    Expertengemisch,
}

/// Der Mitschnitt eines Vorwärtspasses über mehrere Ebenen.
///
/// ⚑ **Eine Liste und kein Verzeichnis.** Die Ebenen laufen der Reihe
/// nach; ein `HashMap<usize, _>` liesse offen, ob eine fehlt, und der
/// Rückwärtspass geht ohnehin genau rückwärts durch diese Liste.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Zwischenwerte {
    ebenen: Vec<Ebenenmitschnitt>,
}

impl Zwischenwerte {
    /// Leer.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Hängt den Mitschnitt einer Ebene an.
    ///
    /// **Nur der Vorwärtspass ruft das**, und zwar genau einmal je
    /// Ebene und in der Reihenfolge, in der er sie rechnet.
    pub fn anhaengen(&mut self, ebene: Ebenenmitschnitt) {
        self.ebenen.push(ebene);
    }

    /// Die Ebenen in Rechenreihenfolge.
    pub fn ebenen(&self) -> &[Ebenenmitschnitt] {
        &self.ebenen
    }

    /// Wie viele Ebenen aufgezeichnet wurden.
    pub fn len(&self) -> usize {
        self.ebenen.len()
    }

    /// Ob nichts aufgezeichnet wurde.
    pub fn is_empty(&self) -> bool {
        self.ebenen.is_empty()
    }

    /// Wirft den Mitschnitt weg.
    ///
    /// ⚑ **Ein Mitschnitt ist gross**, je Ebene mehrere Vektoren über
    /// die versteckte Breite. Wer über eine Folge trainiert, hält sie
    /// alle gleichzeitig; wer sie nicht mehr braucht, sagt es hier.
    pub fn leeren(&mut self) {
        self.ebenen.clear();
    }
}
