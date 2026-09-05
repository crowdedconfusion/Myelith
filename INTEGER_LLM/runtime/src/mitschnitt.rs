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
//! ⚑ **Seit dem 2026-09-05 auch ein Expertengemisch.**
//! [`Mlpteil::Expertengemisch`] sagte bis dahin „hier wurde nicht
//! aufgezeichnet", und der Übersetzer zwang jeden Leser, den Fall zu
//! behandeln. Jetzt trägt die Variante, was `moe_backward` verlangt:
//! Expertenwahl, Mischgewichte, Expertenausgaben, die Zwischenwerte je
//! Experte und die Routerlogits.
//!
//! ⚑ **Und die Marke bleibt trotzdem ein Typ mit Inhalt, kein Tupel
//! leerer Vektoren.** Ein leerer Vektor sieht aus wie ein
//! aufgezeichneter ohne Inhalt; ein Feld, das fehlt, gibt es nicht.

use integer_llm_kernels::mlp::Mlpspur;

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
    /// Ein Expertengemisch, vollständig aufgezeichnet.
    ///
    /// # ⚑ Seit dem 2026-09-05, und was sich damit ändert
    ///
    /// Bis dahin stand hier eine **Marke ohne Inhalt**: „hier wurde
    /// nicht aufgezeichnet". Das war ehrlich und hat den Zweck erfüllt,
    /// den ein Typ erfüllen soll, nämlich jeden Leser zu zwingen, den
    /// Fall zu behandeln. **Jetzt trägt sie, was `moe_backward`
    /// verlangt**, und damit läuft der Rückwärtspass auch durch Router
    /// und Mischung.
    ///
    /// ⚑ **Der Anlass ist eine Entscheidung über das Primärmodell.**
    /// Wird es ein Expertengemisch, ist die zweite Hälfte des
    /// Arbeitsbegriffs ohne diesen Weg unprüfbar.
    Expertengemisch {
        /// Die gewählten Experten, in Auswahlreihenfolge.
        ///
        /// ⚑ **Die Reihenfolge ist nicht Zierrat.** `moe_backward`
        /// ordnet ihr `je_ausgabe` zu, und `gewichte` steht in derselben
        /// Folge. Wer sie sortierte, ordnete Gradienten den falschen
        /// Experten zu, und es fiele nicht auf.
        experten: Vec<u16>,
        /// Die Mischgewichte, auf `prob_frac_bits`, in derselben Folge.
        gewichte: Vec<i32>,
        /// Die Ausgabe **jedes gewählten** Experten, vor der Mischung.
        ausgaben: Vec<Vec<i16>>,
        /// Die Zwischenwerte jedes gewählten Experten.
        ///
        /// ⚑ **Ein Experte ist ein dichter Block**, und deshalb steht
        /// hier [`Mlpspur`], derselbe Typ wie beim dichten Fall, nur
        /// k-mal. `schritt_auf_mlp` gilt für einen Experten unverändert;
        /// das ist an echten 30B-A3B-Gewichten gemessen
        /// (`tests/training_moe.rs`).
        ///
        /// ⛑ **Hier stand für eine halbe Stunde ein eigener Typ
        /// `Expertenspur` mit denselben drei Feldern.** Das ist die
        /// Klasse von Fund 178: zwei Fassungen derselben Sache, die
        /// auseinanderlaufen können, und niemand merkt es, weil beide
        /// übersetzen.
        teile: Vec<Mlpspur>,
        /// Die Routerlogits über **alle** Experten der Ebene.
        ///
        /// ⚑ Sie sind der Eingang des Routergradienten, und sie stehen
        /// hier, weil `moe_backward` seinen Logitgradienten über alle
        /// Experten ausgibt: Wer ihn auf die Routergewichte zurückführen
        /// will, braucht die Eingabe der Projektion, und das ist
        /// `norm_mitte`, plus diese Logits zur Prüfung.
        logits: Vec<i32>,
    },
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

/// Der Sammelplatz, den [`crate::model::IntegerModel`] beim
/// Vorwärtspass eines Expertengemisches füllt.
///
/// ⚑ **Ein eigener Typ und nicht fünf Ausgangsargumente.** Fünf
/// `&mut`-Argumente an einer Funktion, die schon acht hat, liest
/// niemand mehr; und sie gehören zusammen, denn `moe_backward` braucht
/// sie zusammen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Moespur {
    /// Die gewählten Experten, in Auswahlreihenfolge.
    pub experten: Vec<u16>,
    /// Die Mischgewichte, in derselben Folge.
    pub gewichte: Vec<i32>,
    /// Die Ausgabe jedes gewählten Experten.
    pub ausgaben: Vec<Vec<i16>>,
    /// Die Zwischenwerte jedes gewählten Experten.
    pub teile: Vec<Mlpspur>,
    /// Die Routerlogits über alle Experten.
    pub logits: Vec<i32>,
}
