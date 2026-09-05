//! Rückwärtspass, ganzzahlig.
//!
//! ## Warum dieses Modul existiert
//!
//! Der Vorwärtspfad rechnet vollständig in ganzen Zahlen und ist über
//! 30 Konformitätsvektoren als bitgleich belegt. Für **verifizierbares
//! Training** genügt das nicht: Solange der Gradient aus einer
//! Gleitkommarechnung kommt, ist er geräteabhängig, und mit ihm jede
//! Gewichtsänderung. Zwei Miner, die dasselbe Segment trainieren,
//! bekämen dann verschiedene Ergebnisse, und der Redundanzvergleich
//! meldete einen Betrug, wo nur zwei Prozessoren verschieden gerundet
//! haben.
//!
//! Die Messungen in `TRAINING/tests/diag/` haben vorher geklärt, dass
//! sich der Aufwand lohnt: Das Quantisierungsschema trägt im
//! Rückwärtspass (+0,67 % gegen die Gleitkomma-Referenz), und ein
//! Trainingsschritt ohne Gleitkommazustand ist möglich (+0,75 %).
//!
//! ## Was hier gilt
//!
//! Dieselben Regeln wie im Vorwärtspfad, und aus denselben Gründen:
//!
//! - **Akkumulation exakt in `i64`**, Sättigung genau **einmal** ganz am
//!   Ende. Wer zwischendurch klemmt, macht die Summe
//!   reihenfolgeabhängig; siehe den Determinismus-Vertrag im Kopf von
//!   [`crate::dot`].
//! - **Division ausschließlich als arithmetischer Rechtsshift** mit
//!   `rshift_round` (Whitepaper Kap. 6.2, Anhang B.5.4).
//! - **Kein Gleitkomma**, auch nicht in Zwischenschritten.
//!
//! ## Die Ableitungen leiten die IMPLEMENTIERUNG ab, nicht die Formel
//!
//! Ein Rückwärtspass, der eine andere Funktion ableitet als der
//! Vorwärtspfad rechnet, ist wertlos, und der Unterschied fällt in
//! keinem Test auf, der nur die Formel prüft. Jede Funktion hier nennt
//! deshalb, welche Vorwärtsfunktion sie ableitet, und die Tests
//! vergleichen gegen eine **numerische Ableitung des echten
//! Vorwärtskernels**, nicht gegen eine nachgerechnete Formel.
//!
//! ## Was noch fehlt
//!
//! Die Kernel stehen vollständig. Offen sind die Golden Vectors für den
//! Rückwärtspass und der Nachweis, dass zwei Maschinen denselben
//! Gradienten liefern; beides gehört in den Prüfstand, nicht in dieses
//! Modul.

use crate::fixed_point::{clamp_i32, rescale_i64, rshift_round_i64};

/// Die Wortbreite, in der Gradienten zwischen den Ebenen laufen.
///
/// **Warum `i32` und nicht `i8` wie die Gewichte.** Gemessen
/// (`TRAINING/tests/diag/backward_reference_simulation.py`): Der
/// Dynamikbereich der Gradienten beträgt im Median **26,6 Bits**, im
/// Maximum über 90. int8 deckt sieben ab. Die Quantisierung auf int8 je
/// Block ist deshalb eine **Übertragungsform** an der Ebenengrenze
/// (siehe [`quantisiere_block`]) und nicht die Rechenform: Wer schon
/// innerhalb einer Ebene auf acht Bit rundet, verliert dort, wo es nicht
/// nötig ist.
pub type Grad = i32;

// ---------------------------------------------------------------------------
// Übertragungsform: int8 mit Zweierpotenz-Skala je Block
// ---------------------------------------------------------------------------

/// Quantisiert einen Gradienten blockweise auf `bits` mit einer
/// Zweierpotenz-Skala je Block (Whitepaper Anhang B.6.2, NITI).
///
/// Liefert die Werte und den Shift je Block. Die Skala folgt aus dem
/// Betragsmaximum des Blocks, deshalb **kann per Konstruktion nichts
/// sättigen**; der Schaden sitzt am unteren Ende, wo kleine Beträge zu
/// null werden. Gemessen: 7,5 % der Werte im Median, im Maximum 59 %.
/// Dass das Verfahren trotzdem trägt, ist gemessen und nicht angenommen.
///
/// `bits` ist ein Parameter und keine Konstante: Die Messung hat gezeigt,
/// dass acht genügen, aber die Aussage gilt für ein Modell und eine
/// Lernrate.
pub fn quantisiere_block(g: &[Grad], blockgroesse: usize, bits: u8) -> (Vec<i8>, Vec<u8>) {
    assert!(blockgroesse > 0, "quantisiere_block: Blockgroesse muss > 0 sein");
    assert!((2..=8).contains(&bits), "quantisiere_block: bits ausserhalb 2..=8");
    let qmax = (1i64 << (bits - 1)) - 1;

    let mut werte = Vec::with_capacity(g.len());
    let mut shifts = Vec::with_capacity(g.len().div_ceil(blockgroesse));

    for block in g.chunks(blockgroesse) {
        let absmax = block.iter().map(|v| (*v as i64).abs()).max().unwrap_or(0);
        // Der Shift bringt den größten Betrag gerade unter `qmax`. Bei
        // einem Nullblock ist jede Skala richtig; 0 ist die einzige, die
        // keine Frage aufwirft.
        let shift = if absmax == 0 {
            0u8
        } else {
            let mut s = 0i32;
            while (absmax >> s.max(0)) > qmax && s < 62 {
                s += 1;
            }
            s as u8
        };
        shifts.push(shift);
        for v in block {
            let q = rshift_round_i64(*v as i64, shift);
            werte.push(q.clamp(-qmax - 1, qmax) as i8);
        }
    }
    (werte, shifts)
}

/// Die Umkehrung: aus Werten und Blockshifts wieder ein Gradient.
pub fn entquantisiere_block(q: &[i8], shifts: &[u8], blockgroesse: usize) -> Vec<Grad> {
    let mut out = Vec::with_capacity(q.len());
    for (b, block) in q.chunks(blockgroesse).enumerate() {
        let shift = shifts[b] as u32;
        for v in block {
            // Linksshift, also exakte Multiplikation mit einer
            // Zweierpotenz: rundungsfrei und plattformgleich.
            out.push(clamp_i32((*v as i64) << shift));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Lineare Schicht
// ---------------------------------------------------------------------------

/// Rückwärts zu [`crate::linear::linear_w8a16`].
///
/// Vorwärts: `y[i] = Σ_j W[i][j] · x[j]`, herunterskaliert von
/// `act_frac + w_shifts[i]` auf `out_frac`.
///
/// Rückwärts, mit `g = dL/dy`:
///
/// ```text
/// dL/dx[j]    = Σ_i g[i] · W[i][j] / 2^w_shifts[i]
/// dL/dW[i][j] = g[i] · x[j]
/// ```
///
/// **Die Zeilenskala steht in `dL/dx` im Nenner**, nicht im Zähler: Im
/// Vorwärtspfad trägt jede Ausgabezeile ihre eigene Skala, und wer sie
/// hier vergisst, gewichtet die Zeilen gegeneinander falsch. Das ist der
/// wahrscheinlichste stille Fehler in dieser Funktion, deshalb steht er
/// hier und nicht nur im Test.
///
/// `dL/dW` bekommt keine Skalierung: Er wird von der Aktualisierung
/// weiterverarbeitet, die ihre eigene kennt (siehe das TRAINING-Konzept,
/// Abschnitt 2).
pub fn linear_backward(
    g: &[Grad],
    x: &[i16],
    w: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    g_frac: u8,
    gx_frac: u8,
) -> (Vec<Grad>, Vec<i64>) {
    let out_features = g.len();
    assert_eq!(w.len(), out_features * in_features, "linear_backward: W passt nicht zu g");
    assert_eq!(w_shifts.len(), out_features, "linear_backward: eine Skala je Ausgabezeile");
    assert_eq!(x.len(), in_features, "linear_backward: x passt nicht zu in_features");

    // dL/dx: über die Ausgaben summieren.
    //
    // **Ausgerichtet nach OBEN, dann EINMAL geschoben** (Fund 24, hier
    // wiedergefunden). Die erste Fassung schob jeden Summanden einzeln
    // um `w_shifts[i]` nach rechts und addierte danach. Bei
    // `rshift_round(-6, 4)` ist das null, und wenn jeder einzelne
    // Beitrag null wird, ist die Summe null: Der Test gegen die
    // numerische Ableitung des echten Kernels fand ein `dL/dx` von
    // exakt 0, wo −2 hingehörte.
    //
    // Richtig ist die Ausrichtung gegen den GRÖSSTEN Shift per
    // Linksshift, weil dabei kein Bit verlorengeht, und ein einziger
    // Rechtsshift ganz am Ende. Genau so löst `rmsnorm_i16` dasselbe
    // Problem in der Quadratsumme.
    //
    // Der Akkumulator ist `i128`: Ein Produkt erreicht 2^38, der
    // Linksshift geht bis MAX_FRAC_BITS = 20, und summiert wird über
    // alle Ausgabezeilen. In i64 wäre das ein Überlauf, und der
    // numerische Vertrag verbietet Wrapping ausdrücklich.
    let ref_shift = *w_shifts.iter().max().expect("linear_backward: w_shifts ist leer");
    let mut gx_acc = vec![0i128; in_features];
    for (i, zeile) in w.chunks_exact(in_features).enumerate() {
        let gi = g[i] as i128;
        if gi == 0 {
            continue;
        }
        let align = (ref_shift - w_shifts[i]) as u32;
        for (j, wij) in zeile.iter().enumerate() {
            gx_acc[j] += (gi * (*wij as i128)) << align;
        }
    }
    // Ein Rechtsshift, dann die Skalenanpassung, dann Sättigung: alles
    // genau einmal, ganz am Ende.
    let gx = gx_acc
        .into_iter()
        .map(|a| {
            let geschoben = crate::fixed_point::rshift_round_i128(a, ref_shift as u32);
            let begrenzt = geschoben.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
            clamp_i32(rescale_i64(begrenzt, g_frac, gx_frac))
        })
        .collect();

    // dL/dW: äußeres Produkt, keine Reduktion, kein Überlaufrisiko
    // (i32 × i16 passt mit Abstand in i64).
    let mut gw = Vec::with_capacity(out_features * in_features);
    for gi in g {
        for xj in x {
            gw.push((*gi as i64) * (*xj as i64));
        }
    }
    (gx, gw)
}

// ---------------------------------------------------------------------------
// Softmax
// ---------------------------------------------------------------------------

/// Rückwärts zu [`crate::softmax::softmax_int`].
///
/// Vorwärts liefert `p` als Festkommazahl mit `frac_bits`.
///
/// ```text
/// dL/dz[i] = p[i] · (g[i] − Σ_j g[j] · p[j])
/// ```
///
/// **Die Summe wird einmal gebildet und dann geteilt**, nicht je Element
/// neu: Das spart nicht nur Arbeit, es macht das Ergebnis auch
/// unabhängig davon, in welcher Reihenfolge die Elemente abgearbeitet
/// werden. Die Summe selbst ist eine exakte i64-Addition und damit
/// ohnehin ordnungsfrei.
pub fn softmax_backward(g: &[Grad], p: &[Grad], frac_bits: u8) -> Vec<Grad> {
    assert_eq!(g.len(), p.len(), "softmax_backward: g und p muessen gleich lang sein");

    let mut summe: i64 = 0;
    for (gi, pi) in g.iter().zip(p.iter()) {
        summe += (*gi as i64) * (*pi as i64);
    }
    // `summe` trägt g_frac + frac_bits; für den Vergleich mit g auf
    // g_frac zurückschieben.
    let summe = rshift_round_i64(summe, frac_bits);

    g.iter()
        .zip(p.iter())
        .map(|(gi, pi)| {
            let diff = *gi as i64 - summe;
            clamp_i32(rshift_round_i64(diff * (*pi as i64), frac_bits))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Mixture-of-Experts-Modell
// ---------------------------------------------------------------------------

/// Was ein Rückwärtsschritt durch ein Mixture-of-Experts-Modell liefert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoeGradienten {
    /// `dL/dAusgabe` je **gewähltem** Experten, in derselben Reihenfolge
    /// wie `experten` im Vorwärtspass.
    pub je_ausgabe: Vec<Vec<Grad>>,
    /// `dL/dLogit` über **alle** Experten der Layer.
    ///
    /// ⚑ **Nicht gewählte Experten tragen hier exakt null**, und das ist
    /// keine Näherung, sondern folgt aus dem Routing: Bei
    /// `norm_topk_prob` läuft der Softmax nur über die gewählten Logits,
    /// die übrigen kommen in der Ausgabe gar nicht vor. Ein Logit, der
    /// die Ausgabe nicht berührt, hat die Ableitung null.
    ///
    /// **Skala:** `g_frac + logit_zusatz_bits`, siehe [`moe_backward`].
    pub logits: Vec<Grad>,
}

/// Rückwärtspass durch die Mischung eines Mixture-of-Experts-Modells.
///
/// **Der Vorwärtspass, auf den sich das bezieht** (siehe
/// `crate::moe::mische_experten`):
///
/// ```text
/// y = ( Σ_i  w_i · a_i ) >> gewicht_frac_bits
/// ```
///
/// mit `w_i` den Mischgewichten der gewählten Experten und `a_i` deren
/// Ausgaben. Daraus:
///
/// ```text
/// dL/da_i = ( g · w_i ) >> gewicht_frac_bits
/// dL/dw_i = Σ_j g_j · a_ij
/// ```
///
/// Der zweite Gradient geht anschließend durch den Softmax über die
/// gewählten Logits ([`softmax_backward`]) und wird auf die volle
/// Logit-Reihe verteilt.
///
/// **Skalen.** `g` trägt `g_frac`, `gewichte` tragen
/// `gewicht_frac_bits`, `ausgaben` tragen `aus_frac`. `dL/da_i` kommt
/// wieder auf `g_frac` heraus. `dL/dw_i` entsteht auf
/// `g_frac + aus_frac` und wird um `aus_frac` zurückgeschoben, damit es
/// mit `g` vergleichbar ist, wie [`softmax_backward`] es erwartet.
///
/// ## ⚑ Fund 79: Ein gesättigter Router kann sich nie wieder ändern
///
/// **Der Ganzzahl-Softmax sättigt, und Sättigung ist ein absorbierender
/// Zustand.** Bei genügend Abstand zwischen zwei Logits liefert
/// `softmax_int` die Gewichte `(1 << frac, 0)` statt „fast alles" und
/// „fast nichts". Gemessen am Testaufbau dieses Moduls genügt dafür ein
/// Abstand von 80 Einheiten bei `frac = 8`.
///
/// **Dann ist der Gradient jedes Logits exakt null, und zwar aus
/// Rechengründen, nicht durch Rundung:**
///
/// ```text
/// out_i = ( (g_i − Σ_j g_j p_j / 2^frac) · p_i ) >> frac
/// ```
///
/// - Für einen Verlierer ist `p_i = 0`, also ist der Faktor null.
/// - Für den Gewinner ist `p_0 = 2^frac`, also `Σ_j g_j p_j / 2^frac =
///   g_0`, also ist die **Klammer** null.
///
/// ⚑ **Beide Wege führen auf null, und damit steht der Router still.**
/// Ein Router, der einmal sicher genug war, bleibt es für immer: Sein
/// Gradient verschwindet, bevor er ihn ändern könnte. **In Gleitkomma
/// gibt es diesen Zustand nicht**, dort ist ein Softmax bei endlichen
/// Logits nie exakt 0 oder 1. Er entsteht erst durch die Ganzzahltabelle
/// und ist damit ein Preis dieses Projekts, keine Eigenschaft von MoE.
///
/// **Auch ohne Sättigung ist der Gradient klein.** Er trägt den Faktor
/// `p_i · (1 − p_i)`; je entschiedener die Wahl, desto kleiner. Auf der
/// Skala des Aktivierungsgradienten rundet er dann auf null, bevor er
/// wirkt. Deshalb nimmt diese Funktion `logit_zusatz_bits`: Der
/// Logit-Gradient wird um so viele Bit **feiner** geführt und kommt auf
/// `g_frac + logit_zusatz_bits` heraus. Gegen die Sättigung hilft das
/// nicht, gegen die Rundung schon.
///
/// **Was daraus folgt, und es ist ein offener Entwurfspunkt:** Ein
/// Lastausgleich ist hier keine Verbesserung, sondern eine
/// Voraussetzung. Er muss verhindern, dass ein Router sättigt. Die
/// üblichen Verfahren tun das über einen Hilfsverlust mit
/// Batch-Statistiken und über Rauschen im Router; beides scheidet aus
/// (siehe unten). Solange kein Ersatz steht, ist Training auf einem
/// Mixture-of-Experts-Modell **möglich, aber nicht stabil**, und dieser Satz
/// gehört zu jedem Ergebnis dazu.
///
/// ⚑ **Was hier bewusst nicht passiert: eine Lastausgleichs-Korrektur.**
/// Die gängigen Verfahren addieren einen Hilfsverlust über
/// Batch-Statistiken und oft Rauschen im Router. Beides scheidet aus:
/// Rauschen ist nicht deterministisch, und eine Größe über den Batch
/// machte das Ergebnis an Position *i* davon abhängig, welche anderen
/// Token zufällig danebenlagen. Das ist dieselbe Klasse wie das für den
/// Vorwärtspfad bereits verbotene Token-Dropping.
#[allow(clippy::too_many_arguments)]
pub fn moe_backward(
    g: &[Grad],
    experten: &[u16],
    gewichte: &[i32],
    ausgaben: &[Vec<i16>],
    anzahl_experten: usize,
    gewicht_frac_bits: u8,
    aus_frac: u8,
    logit_zusatz_bits: u8,
) -> MoeGradienten {
    assert!(
        logit_zusatz_bits <= aus_frac,
        "moe_backward: logit_zusatz_bits {} ueber aus_frac {}, der Zwischenwert waere ungeschoben (Fund 79)",
        logit_zusatz_bits,
        aus_frac
    );
    assert_eq!(
        experten.len(),
        gewichte.len(),
        "moe_backward: je gewaehltem Experten genau ein Gewicht"
    );
    assert_eq!(
        experten.len(),
        ausgaben.len(),
        "moe_backward: je gewaehltem Experten genau eine Ausgabe"
    );

    // dL/da_i: das eigene Mischgewicht skaliert den eingehenden Gradienten.
    let je_ausgabe: Vec<Vec<Grad>> = gewichte
        .iter()
        .map(|w| {
            g.iter()
                .map(|gi| clamp_i32(rshift_round_i64((*gi as i64) * (*w as i64), gewicht_frac_bits)))
                .collect()
        })
        .collect();

    // dL/dw_i: das Skalarprodukt aus eingehendem Gradienten und
    // Expertenausgabe. In i64 summiert und **einmal** geschoben, nicht
    // je Summand (Fund 24).
    let d_gewichte: Vec<Grad> = ausgaben
        .iter()
        .map(|a| {
            assert_eq!(a.len(), g.len(), "moe_backward: Ausgabe passt nicht zu g");
            let mut summe: i64 = 0;
            for (gi, ai) in g.iter().zip(a.iter()) {
                summe += (*gi as i64) * (*ai as i64);
            }
            // Weniger weit schieben heißt feiner führen: Das Ergebnis
            // trägt `g_frac + logit_zusatz_bits` statt `g_frac`.
            clamp_i32(rshift_round_i64(summe, aus_frac - logit_zusatz_bits))
        })
        .collect();

    // Durch den Softmax über die **gewählten** Logits.
    let d_gewaehlte = softmax_backward(&d_gewichte, gewichte, gewicht_frac_bits);

    // Auf die volle Reihe verteilen. Alles, was nicht gewählt wurde,
    // bleibt null.
    let mut logits = vec![0 as Grad; anzahl_experten];
    for (e, d) in experten.iter().zip(d_gewaehlte.iter()) {
        let i = *e as usize;
        assert!(i < anzahl_experten, "moe_backward: Expertenindex ausserhalb");
        logits[i] = *d;
    }

    MoeGradienten { je_ausgabe, logits }
}

/// Gradient einer Spreizungsstrafe auf die Router-Logits.
///
/// ## ⚑ Was sie behebt (Fund 79)
///
/// Der Ganzzahl-Softmax sättigt ab einem Logit-Abstand von
/// [`crate::moe::saettigungsabstand`], und ein gesättigter Router hat
/// überall den Gradienten null. Er kann sich dann nie wieder ändern.
///
/// **Diese Strafe hat genau dort ihren größten Wert, wo der
/// Softmax-Gradient verschwunden ist**, und das ist ihre entscheidende
/// Eigenschaft: Sie liest die **Logit-Abstände**, nicht die
/// quantisierten Gewichte. Ob `p_i` auf null gerundet wurde, ist ihr
/// gleichgültig; sie sieht, dass `z_i` zu weit unten liegt, und schiebt
/// es zurück.
///
/// ```text
/// ueberschuss_i = max(0, (z_max − z_i) − schwelle)
/// dz_i          = + (ueberschuss_i >> daempfung)        für die Verlierer
/// dz_max        = − Σ_i (ueberschuss_i >> daempfung)    als Gegenbuchung
/// ```
///
/// ⚑ **Die Summe ist exakt null.** Die Strafe verschiebt den
/// Logit-Mittelwert nicht, sie staucht nur die Spreizung. Ohne diese
/// Eigenschaft zöge sie das Routing langsam in eine Richtung, und
/// niemand sähe es. Es gibt dafür einen Test.
///
/// **Sie greift nur, wenn sie muss.** Liegen alle gewählten Logits
/// innerhalb der Schwelle, ist der Rückgabewert **exakt null** an jeder
/// Stelle, und das Training verläuft, als gäbe es sie nicht. Ein
/// gesunder Router wird nicht verbogen.
///
/// ## Warum keines der üblichen Verfahren
///
/// - **Hilfsverlust über Batch-Statistiken** (Switch Transformer,
///   GShard): Das Ergebnis an Position *i* hinge davon ab, welche
///   anderen Token zufällig danebenlagen. Dieselbe Klasse wie das für
///   den Vorwärtspfad bereits verbotene Token-Dropping.
/// - **Rauschen im Router**: nicht deterministisch, und ohne
///   Determinismus keine Redundanzprüfung.
/// - **`z`-Verlust** (ST-MoE): Sein Gradient ist zwar je Token lokal und
///   damit brauchbar, aber er staucht **alle** Logits gegen null,
///   gleichgültig ob der Router gesund ist. Er braucht außerdem
///   `logsumexp`, also einen Logarithmus im Ganzzahlpfad. Die
///   Spreizungsstrafe erreicht dasselbe Ziel mit weniger Eingriff und
///   ohne neue Primitive.
///
/// **Was sie nicht leistet:** Lastausgleich. Ein Experte, der über viele
/// Token nie gewählt wird, bleibt untrainiert, und das ist eine Aussage
/// über die Segmentfolge und nicht über ein Token. Sie steht weiter
/// offen.
///
/// ## Skalen
///
/// Rückgabe in **Logit-Einheiten**, also derselben Skala wie `logits`.
/// [`moe_backward`] liefert seinen Logit-Gradienten dagegen auf
/// `g_frac + logit_zusatz_bits`. **Der Aufrufer bringt beide auf eine
/// Skala, bevor er sie addiert**; hier steht es, weil es sonst niemandem
/// auffiele.
pub fn router_spreizung(
    logits: &[i32],
    experten: &[u16],
    schwelle: i32,
    daempfung: u8,
) -> Vec<Grad> {
    let mut d = vec![0 as Grad; logits.len()];
    if experten.is_empty() {
        return d;
    }
    assert!(schwelle >= 0, "router_spreizung: negative Schwelle");

    // Das Maximum über die **gewählten** Logits. Nicht über alle: Ein
    // nicht gewählter Experte kann beliebig tief liegen, das ist der
    // Sinn von Top-k, und ihn hochzuziehen wäre Lastausgleich und nicht
    // Sättigungsschutz.
    let mut hoechster = experten[0] as usize;
    for e in experten.iter() {
        let i = *e as usize;
        assert!(i < logits.len(), "router_spreizung: Index ausserhalb");
        if logits[i] > logits[hoechster] {
            hoechster = i;
        }
    }

    let mut gegenbuchung: i64 = 0;
    for e in experten.iter() {
        let i = *e as usize;
        if i == hoechster {
            continue;
        }
        let abstand = (logits[hoechster] as i64) - (logits[i] as i64);
        let ueberschuss = abstand - schwelle as i64;
        if ueberschuss <= 0 {
            continue;
        }
        let schub = ueberschuss >> daempfung;
        d[i] = clamp_i32(schub);
        gegenbuchung += d[i] as i64;
    }
    d[hoechster] = clamp_i32(-gegenbuchung);
    d
}

// ---------------------------------------------------------------------------
// SiLU
// ---------------------------------------------------------------------------

/// Rückwärts zur SiLU-Aktivierung aus [`crate::mlp::mlp_int`].
///
/// Vorwärts wird `silu` über eine LUT ausgewertet. Die Ableitung
/// `silu'(x) = σ(x) · (1 + x · (1 − σ(x)))` ist keine Funktion, die sich
/// aus der Vorwärts-LUT sauber gewinnen ließe: `σ(x) = silu(x)/x` ist bei
/// null undefiniert und in der Umgebung numerisch unbrauchbar. Sie
/// bekommt deshalb **eine eigene LUT**, erzeugt wie die vorhandenen und
/// an `theta_v` gebunden.
///
/// Der Index wird wie im Vorwärtspfad gebildet: Eingang in die feste
/// LUT-Domäne reskalieren, dann [`crate::integer_math::lut_lookup`], das
/// am Rand deterministisch sättigt.
#[allow(clippy::too_many_arguments)]
pub fn silu_backward(
    g: &[Grad],
    x: &[i16],
    grad_lut: &[i16],
    x_frac: u8,
    lut_in_frac: u8,
    lut_offset: i16,
    lut_out_frac: u8,
    g_frac: u8,
    out_frac: u8,
) -> Vec<Grad> {
    assert_eq!(g.len(), x.len(), "silu_backward: g und x muessen gleich lang sein");
    g.iter()
        .zip(x.iter())
        .map(|(gi, xi)| {
            let dom = crate::fixed_point::rescale(*xi as i32, x_frac, lut_in_frac);
            let ableitung =
                crate::integer_math::lut_lookup(clamp_i16_sat(dom), grad_lut, 0, lut_offset) as i64;
            let prod = (*gi as i64) * ableitung;
            clamp_i32(rescale_i64(prod, g_frac + lut_out_frac, out_frac))
        })
        .collect()
}

/// Sättigt auf i16, ohne `clamp_i16` zu umgehen: Der LUT-Index darf
/// nicht wrappen, und ein zu großer Eingang gehört an den LUT-Rand.
fn clamp_i16_sat(v: i32) -> i16 {
    crate::fixed_point::clamp_i16(v)
}

/// Baut den Gradientenvorrat **aus der Vorwaerts-Tabelle** (TRAINING V).
///
/// # ⚑ Die Ableitung dessen, was gelaufen ist, nicht dessen, was gemeint war
///
/// Der Vorwaertspass rechnet Silu **nicht** analytisch, sondern schlaegt
/// in einer Tabelle nach. Ein Gradientenvorrat aus der analytischen
/// Ableitung passte deshalb zu einer Funktion, die so nie ausgefuehrt
/// wurde; an den Stellen, an denen die Tabelle rundet, zeigte er in eine
/// leicht andere Richtung. **Hier steht die zentrale Differenz der
/// Tabelle selbst**, und die passt immer.
///
/// # ⚑ Und sie ist exakt, weil sich zwei Zweien wegkuerzen
///
/// Die Ableitung ist `(lut[i+1] - lut[i-1]) / 2` je Eingangsschritt,
/// also mal `2^(in - out + f)` auf Bruchstellen `f`. Mit
/// `f = out - in + 1` bleibt genau `lut[i+1] - lut[i-1]` stehen:
/// **keine Division, keine Rundung, kein Gleitkomma.**
/// [`silu_grad_frac`] rechnet die Skala aus.
///
/// An den beiden Raendern steht die einseitige Differenz, verdoppelt,
/// damit sie dieselbe Skala traegt.
///
/// # Panics
///
/// Wenn die Tabelle weniger als zwei Eintraege hat: Aus einem Punkt
/// folgt keine Steigung.
pub fn silu_grad_aus_lut(lut: &[i16]) -> Vec<i16> {
    assert!(
        lut.len() >= 2,
        "silu_grad_aus_lut: aus {} Eintraegen folgt keine Steigung",
        lut.len()
    );
    let n = lut.len();
    let mut aus = vec![0i16; n];
    for i in 0..n {
        let d: i32 = if i == 0 {
            2 * (i32::from(lut[1]) - i32::from(lut[0]))
        } else if i == n - 1 {
            2 * (i32::from(lut[n - 1]) - i32::from(lut[n - 2]))
        } else {
            i32::from(lut[i + 1]) - i32::from(lut[i - 1])
        };
        aus[i] = crate::fixed_point::clamp_i16(d);
    }
    aus
}

/// Auf welchen Bruchstellen [`silu_grad_aus_lut`] liefert.
///
/// ⚑ **Eine Funktion und keine Zahl im Kommentar.** Wer die Ableitung
/// baut, braucht ihre Skala, und zwei Stellen, die sie unabhaengig
/// ausrechnen, laufen auseinander.
pub const fn silu_grad_frac(lut_in_frac: u8, lut_out_frac: u8) -> u8 {
    lut_out_frac + 1 - lut_in_frac
}

// ---------------------------------------------------------------------------
// RMSNorm
// ---------------------------------------------------------------------------

/// Die Skalen, mit denen eine RMSNorm rückwärts gerechnet wird.
///
/// ⚑ **`r` allein sagt nichts.** Seine Bruchstellen bestehen aus zwei
/// Zahlen, die der Vorwärtspass getrennt kennt: `norm_frac` (die Skala
/// des Tabellenwerts) und `ref_shift` (die Ausrichtung, gegen die die
/// Quadratsumme gebildet wurde). Beide kommen aus
/// [`crate::rmsnorm::Rmsnormspur`], und wer sie hier zu einer Zahl
/// zusammenzieht, legt eine Lesart fest, die niemand mehr nachprüfen
/// kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Normskalen {
    /// Der Tabellenwert des Kehrwerts der Wurzel, aus dem Vorwärtspass.
    pub r: i32,
    /// Seine Bruchstellen, bezogen auf die Darstellung der Quadratsumme.
    pub norm_frac: u8,
    /// Die Ausrichtung der Quadratsumme, also das Maximum der
    /// Eingangsverschiebungen.
    pub ref_shift: u8,
    /// `2^20 / n`, wie im Vorwärtspass.
    pub inv_n_q20: i64,
    /// Bruchstellen des eingehenden Gradienten (der Bus herein).
    pub g_frac: u8,
    /// Bruchstellen des ausgehenden Gradienten (der Bus heraus).
    pub gx_frac: u8,
}

/// Schutzstellen der Zwischenwerte im RMSNorm-Rückwärtspass.
///
/// ⚑ **Vierundzwanzig, und die Zahl ist gerechnet.** Der zweite Term
/// entsteht aus `r³ · x · Σ`, und jede Stufe, die auf die Einerstelle
/// rundet, wirft ihn bei kleinen `r` weg. Mit vierundzwanzig
/// Schutzstellen bleiben die beiden Faktoren unter `2^63`, sodass ihr
/// Produkt in `i128` passt: `r_real³ ≤ 1` gibt `2^(rf+24) ≤ 2^44`, mal
/// `x ≤ 2^15` sind `2^59`.
const SCHUTZ_BITS: i32 = 24;

/// Schiebt in beide Richtungen: rechts mit Rundung, links exakt.
///
/// ⚑ **Nötig, weil die Gesamtverschiebung hier negativ werden kann.**
/// `norm_frac − ref_shift` ist die Skala von `r` gegenüber dem realen
/// Wert, und sie ist bei grober Eingangsskala kleiner als null.
#[inline]
fn verschiebe_i128(v: i128, shift: i32) -> i128 {
    if shift >= 0 {
        crate::fixed_point::rshift_round_i128(v, shift as u32)
    } else {
        v << (-shift) as u32
    }
}

/// Rückwärts zu [`crate::rmsnorm::rmsnorm_i16`].
///
/// Vorwärts: `y_i = x_i · r · γ_i` mit `r = 1/sqrt(mean(x²))`, wobei `r`
/// aus der rsqrt-Tabelle stammt.
///
/// ```text
/// dL/dγ_i = g_i · x_i · r
/// dL/dx_j = r · γ_j · g_j − (r³ · x_j / n) · Σ_i g_i · γ_i · x_i
/// ```
///
/// **Der zweite Term ist der, den man vergisst.** Er kommt daher, dass
/// `r` von *allen* `x` abhängt: Eine Änderung an `x_j` verschiebt die
/// Norm und damit jede Ausgabe. Ohne ihn ist der Gradient nicht falsch
/// skaliert, sondern schlicht ein anderer, und der Fehler wächst mit der
/// Länge des Vektors.
///
/// `r` wird **nicht neu berechnet**, sondern vom Vorwärtspfad
/// übernommen: Ein zweiter Tabellennachschlag könnte einen anderen Index
/// treffen, und dann leitete dieser Kern eine andere Funktion ab als die,
/// die gerechnet wurde.
///
/// # ⚑ Jede Grösse dieser Formel ist eine **reale** Grösse (Fund 175)
///
/// Die Formel oben steht in realen Werten, und genau darin lag der
/// Fehler: Bis zum 2026-09-04 setzte die Umsetzung an drei Stellen
/// **Darstellungen** ein.
///
/// | Grösse | stand | gehört |
/// |---|---|---|
/// | `r` | `r / 2^norm_frac` | `r / 2^(norm_frac − ref_shift)` |
/// | `x` im zweiten Term | die Darstellung | `x / 2^x_shifts[j]` |
/// | `x` in der Summe | die Darstellung | `x / 2^x_shifts[i]` |
///
/// ⚑ **Die beiden Terme lagen dadurch auf verschiedenen Skalen**, und
/// das ist schlimmer als ein gemeinsamer Faktor: Gegen die numerische
/// Ableitung des echten Vorwärtskerns gemessen streuten die
/// Verhältnisse über acht Kanäle von **1,08 bis −37,4**, also
/// einschliesslich eines gedrehten Vorzeichens.
///
/// ⚑ **Und `x_shifts` fehlte ganz.** Der Vorwärtspass trägt eine Skala
/// **je Kanal** (Fund 20); dieser Kern kannte sie nicht und konnte
/// deshalb nicht einmal im Ansatz stimmen, sobald sie auseinandergehen.
///
/// **Aufgefallen ist es nicht, weil die Funktion ausserhalb ihrer Tests
/// keinen Aufrufer hatte** und ihre beiden Tests Eigenschaften prüften,
/// die von der Skala unabhängig sind: dass der zweite Term überhaupt
/// wirkt, und dass ein Nullgradient nichts erzeugt. Dieselbe Lage wie
/// bei Fund 173.
pub fn rmsnorm_backward(
    g: &[Grad],
    x: &[i16],
    x_shifts: &[u8],
    gamma: &[i8],
    gamma_shifts: &[u8],
    s: Normskalen,
) -> (Vec<Grad>, Vec<i64>) {
    let n = x.len();
    assert_eq!(g.len(), n, "rmsnorm_backward: g und x muessen gleich lang sein");
    assert_eq!(gamma.len(), n, "rmsnorm_backward: gamma passt nicht");
    assert_eq!(gamma_shifts.len(), n, "rmsnorm_backward: ein Shift je Gamma");
    assert_eq!(x_shifts.len(), n, "rmsnorm_backward: ein Shift je Eingangskanal (Fund 20)");

    let r = i128::from(s.r);
    // Die Bruchstellen von `r` gegenueber dem **realen** Wert.
    let rf = i32::from(s.norm_frac) - i32::from(s.ref_shift);

    // Σ g_i · γ_i^real · x_i^real.
    //
    // **Nach oben ausgerichtet und einmal geschoben**, aus demselben
    // Grund wie in [`linear_backward`]: Jeden Summanden einzeln zu
    // runden loescht die kleinen aus, und bei breiter Shift-Spanne
    // bleibt von der Summe nur der groebste Kanal uebrig (Fund 24).
    let ref_summe = (0..n)
        .map(|i| u32::from(gamma_shifts[i]) + u32::from(x_shifts[i]))
        .max()
        .expect("rmsnorm_backward: leerer Vektor");
    let mut s_acc: i128 = 0;
    for i in 0..n {
        let eigen = u32::from(gamma_shifts[i]) + u32::from(x_shifts[i]);
        let term = i128::from(g[i]) * i128::from(gamma[i]) * i128::from(x[i]);
        s_acc += term << (ref_summe - eigen);
    }
    // ⚑ **Nicht auf die Einerstelle runden, sondern auf `SCHUTZ_BITS`.**
    // Die Summe geht als Faktor in den zweiten Term ein; wer sie hier
    // ganzzahlig macht, wirft bei kleinen Werten den ganzen Term weg.
    let summe_fein = verschiebe_i128(s_acc, ref_summe as i32 - SCHUTZ_BITS)
        .clamp(i64::MIN as i128, i64::MAX as i128);

    // dL/dγ_i = g_i · x_i^real · r^real, ohne Reduktion.
    let mut ggamma = Vec::with_capacity(n);
    for i in 0..n {
        let roh = i128::from(g[i]) * i128::from(x[i]) * r;
        let geschoben = verschiebe_i128(roh, i32::from(x_shifts[i]) + rf);
        ggamma.push(geschoben.clamp(i64::MIN as i128, i64::MAX as i128) as i64);
    }

    // dL/dx
    //
    // ⚑ **Gestaffelt geschoben, aber mit Schutzbits.** Ein Produkt aus
    // `r³`, `x_j` und der Summe verliesse i128; wer dagegen nach jeder
    // Stufe auf die Einerstelle rundet, verliert den zweiten Term
    // ganz. ⛑ Gemessen: `r` von 106 auf neun Bruchstellen ergibt `r³`
    // gerundet **5**, und `5 · 300 >> 15` ist **null**: Der ganze
    // Normierungsterm verschwand, und der Test daneben sah es nicht,
    // weil seine Summe sich zufaellig fast aufhob.
    //
    // `r3f` traegt `r_real³ · 2^(rf + SCHUTZ_BITS)`.
    let r2f = verschiebe_i128(r * r, rf - SCHUTZ_BITS);
    let r3f = verschiebe_i128(r2f * r, rf);
    let mut gx = Vec::with_capacity(n);
    for j in 0..n {
        // Erster Term: r^real · γ_j^real · g_j
        let t1 = verschiebe_i128(
            i128::from(g[j]) * i128::from(gamma[j]) * r,
            i32::from(gamma_shifts[j]) + rf,
        );
        // Zweiter Term: (r³ · x_j^real / n) · Σ
        let mit_x = verschiebe_i128(r3f * i128::from(x[j]), i32::from(x_shifts[j]));
        let mit_n = ((mit_x * i128::from(s.inv_n_q20)) >> 20)
            .clamp(i64::MIN as i128, i64::MAX as i128);
        let t2 = verschiebe_i128(mit_n * summe_fein, rf + 2 * SCHUTZ_BITS);
        let roh = (t1 - t2).clamp(i64::MIN as i128, i64::MAX as i128) as i64;
        gx.push(clamp_i32(rescale_i64(roh, s.g_frac, s.gx_frac)));
    }
    (gx, ggamma)
}

// ---------------------------------------------------------------------------
// RoPE
// ---------------------------------------------------------------------------

/// Rückwärts zu [`crate::rope::rotate_half_split_i16`].
///
/// Vorwärts ist die Rotation eines Paares `(x0, x1)` um den Winkel θ:
///
/// ```text
/// y0 = x0·cos − x1·sin
/// y1 = x1·cos + x0·sin
/// ```
///
/// Die Jacobi-Matrix dieser Abbildung ist die Rotationsmatrix selbst,
/// und **eine Rotationsmatrix ist orthogonal**: Ihre Transponierte ist
/// ihre Inverse, also die Drehung um −θ. Daraus folgt:
///
/// ```text
/// gx0 = g0·cos + g1·sin
/// gx1 = g1·cos − g0·sin
/// ```
///
/// Nur die Vorzeichen wandern, `sin` wird **nicht** negiert und keine
/// neue LUT gebraucht. Wer stattdessen `sin` negiert und die Formel
/// unverändert lässt, bekommt dasselbe Ergebnis; wer beides tut, dreht
/// in die falsche Richtung, und der Fehler ist in einer Zahlenprobe
/// kaum zu sehen. Deshalb steht die Herleitung hier.
pub fn rope_backward(g: &[Grad], cos_row: &[i16], sin_row: &[i16], frac_bits: u8) -> Vec<Grad> {
    let half = g.len() / 2;
    assert_eq!(g.len(), 2 * half, "rope_backward: Länge muss gerade sein");
    assert_eq!(cos_row.len(), half, "rope_backward: cos_row-Länge muss head_dim/2 sein");
    assert_eq!(sin_row.len(), half, "rope_backward: sin_row-Länge muss head_dim/2 sein");

    let mut out = vec![0i32; g.len()];
    for j in 0..half {
        let cos = cos_row[j] as i64;
        let sin = sin_row[j] as i64;
        let g0 = g[j] as i64;
        let g1 = g[j + half] as i64;
        // Erst das volle Produkt, dann ein Rechtsshift: Wer je Summand
        // schiebt, verliert die kleinen Beiträge (siehe
        // [`linear_backward`]).
        out[j] = clamp_i32(rshift_round_i64(g0 * cos + g1 * sin, frac_bits));
        out[j + half] = clamp_i32(rshift_round_i64(g1 * cos - g0 * sin, frac_bits));
    }
    out
}

// ---------------------------------------------------------------------------
// Attention
// ---------------------------------------------------------------------------

/// Die Skalen, mit denen ein Aufmerksamkeitskopf rückwärts gerechnet
/// wird.
///
/// ⚑ **Zusammengefasst, weil sechs gleichartige Zahlen niemand richtig
/// übergibt.** `q_frac` und `k_frac` stehen in **verschiedenen**
/// Schiebeweiten und sind beide `u8`; eine Vertauschung ist kein
/// Übersetzungsfehler, sondern ein um Zweierpotenzen verschobener
/// Gradient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aufmerksamkeitsskalen {
    /// Der Faktor `1/sqrt(head_dim)` des Vorwärtspasses, als Ganzzahl.
    pub score_mult: i64,
    /// Bruchstellen von `score_mult`. Für
    /// [`crate::fixed_point::inv_sqrt_q15`] sind es 15.
    pub score_mult_frac: u8,
    /// Bruchstellen der Q-Werte, **wie die Aufmerksamkeit sie sah**,
    /// also nach RoPE und gegebenenfalls nach der QK-Normierung.
    pub q_frac: u8,
    /// Bruchstellen der K-Werte, ebenso.
    pub k_frac: u8,
    /// Bruchstellen der V-Werte. ⚑ **Und damit auch der Ausgabe**: Eine
    /// gewichtete Summe von V trägt die Skala von V.
    pub v_frac: u8,
    /// Bruchstellen der Wahrscheinlichkeiten.
    pub prob_frac: u8,
}

/// Rückwärts zu [`crate::attention::attention_int`], für **eine**
/// Abfrageposition.
///
/// Vorwärts, mit `s_j = (q·k_j)·score_mult >> score_shift`,
/// `p = softmax(s)` und `out = Σ_j p_j·v_j >> prob_frac`:
///
/// ```text
/// dL/dv_j = g · p_j
/// dL/dp_j = g · v_j                (Summe über die Kopf-Dimension)
/// dL/ds   = softmax_backward(dL/dp, p)
/// dL/dq   = Σ_j dL/ds_j · k_j / sqrt(head_dim)
/// dL/dk_j = dL/ds_j · q / sqrt(head_dim)
/// ```
///
/// # ⚑ Der Vertrag über einen Gradienten, und warum er hier stehen muss
///
/// Ein Gradient dieses Projekts trägt **`dL/dZ` nach dem realen Wert**
/// von `Z`, dargestellt auf einer gemeinsamen Skala, die der Aufrufer
/// wählt (dem Gradientenbus). Jeder Kern nimmt die Skala herein und
/// gibt die Skala heraus: [`linear_backward`] als `g_frac`/`gx_frac`,
/// [`silu_backward`] ebenso, [`rmsnorm_backward`] ebenso.
///
/// **Daraus folgen die Schiebeweiten hier**, und keine davon ist die
/// des Vorwärtspasses:
///
/// | Schritt | Schiebeweite | weil |
/// |---|---|---|
/// | `dL/dv` | `prob_frac` | der Faktor ist `p`, und `p` ist dimensionslos |
/// | `dL/dp` | **`v_frac`** | der Faktor ist `v`, nicht `p` |
/// | `dL/dq` | **`k_frac + score_mult_frac`** | der Faktor ist `k`, nicht `q` |
/// | `dL/dk` | **`q_frac + score_mult_frac`** | der Faktor ist `q`, nicht `k` |
///
/// ⚑ **Bis zum 2026-09-04 stand an allen vier Stellen etwas anderes:**
/// `prob_frac` für `dL/dp` und der **Vorwärts**-`score_shift`
/// (`q_frac + k_frac + score_mult_frac − score_frac`) für `dL/dq` und
/// `dL/dk`. Das ist in sich stimmig, wenn ein Gradient `dL/dZ` nach der
/// **Darstellung** von `Z` trägt statt nach ihrem Wert; und es ist mit
/// jedem anderen Rückwärtskern dieses Moduls unvereinbar, weil die
/// beiden Lesarten sich um `2^(2·frac)` unterscheiden. Gemessen an der
/// numerischen Ableitung des echten Vorwärtskerns war `dL/dq` um
/// `2^(v_frac − prob_frac + score_frac − q_frac)` daneben, bei
/// Qwen2.5-0,5B also um den Faktor 256, `dL/dk` um 512.
///
/// **Aufgefallen ist es nicht**, weil diese Funktion ausserhalb ihrer
/// eigenen Tests keinen Aufrufer hatte und die Tests genau die
/// Richtung prüften, in der sich die beiden Lesarten nicht
/// unterscheiden: `dL/dv` trägt dieselbe Skala wie die Ausgabe, und für
/// gleiche Skalen fallen beide Lesarten zusammen.
///
/// # ⚑ `score_frac` kommt nicht mehr vor, und das ist die Probe
///
/// Rückwärts gibt es keine Score-Skala: `s` und `p` sind reale Grössen,
/// und die Ableitung des Softmax `p ⊙ (g − ⟨g, p⟩)` ist zwischen ihnen
/// dimensionslos. Wer hier eine Score-Skala braucht, rechnet nach der
/// Darstellung statt nach dem Wert.
///
/// **Maskierte Positionen bekommen null**, und zwar ausdrücklich statt
/// nebenbei: Vorwärts bekommen sie `i32::MIN` und damit `p ≈ 0`, aber
/// „ungefähr null" ist im Rückwärtspass kein Argument. Ein Gradient auf
/// eine Position, die nie gelesen wurde, wäre ein Leck über die
/// Kausalitätsgrenze hinweg.
///
/// Die Funktion setzt [`softmax_backward`] ein und rechnet sonst nur
/// Produkte und Summen; sie ist die Zusammensetzung, als die sie im
/// Konzept beschrieben ist.
pub fn attention_backward(
    g: &[Grad],
    q: &[i16],
    k: &[Vec<i16>],
    v: &[Vec<i16>],
    p: &[Grad],
    mask: &[bool],
    s: Aufmerksamkeitsskalen,
) -> (Vec<Grad>, Vec<Vec<Grad>>, Vec<Vec<Grad>>) {
    let kv_len = k.len();
    let head_dim = q.len();
    assert_eq!(v.len(), kv_len, "attention_backward: k und v ungleich lang");
    assert_eq!(p.len(), kv_len, "attention_backward: p passt nicht zu k");
    assert_eq!(mask.len(), kv_len, "attention_backward: mask passt nicht zu k");
    assert_eq!(g.len(), head_dim, "attention_backward: g passt nicht zu q");

    // dL/dv_j = g · p_j, und dL/dp_j = Σ_d g_d · v_j[d].
    //
    // ⚑ **Zwei Produkte, zwei Schiebeweiten.** Im ersten ist der Faktor
    // `p` und die Weite deshalb `prob_frac`; im zweiten ist der Faktor
    // `v` und die Weite deshalb `v_frac`. Dass beide Zeilen
    // nebeneinander stehen und verschieden schieben, ist die Aussage.
    let mut gv = Vec::with_capacity(kv_len);
    let mut gp = Vec::with_capacity(kv_len);
    for j in 0..kv_len {
        if !mask[j] {
            gv.push(vec![0i32; head_dim]);
            gp.push(0i32);
            continue;
        }
        let mut zeile = Vec::with_capacity(head_dim);
        let mut acc: i64 = 0;
        for d in 0..head_dim {
            zeile.push(clamp_i32(rshift_round_i64(
                (g[d] as i64) * (p[j] as i64),
                s.prob_frac,
            )));
            acc += (g[d] as i64) * (v[j][d] as i64);
        }
        gv.push(zeile);
        gp.push(clamp_i32(rshift_round_i64(acc, s.v_frac)));
    }

    let gs = softmax_backward(&gp, p, s.prob_frac);

    // dL/dq und dL/dk. Der Faktor `1/sqrt(head_dim)` steht vorwärts wie
    // rückwärts (Fund 19: Q15-Multiplikation statt Shift, weil der Shift
    // nur für gerade Zweierpotenzen stimmt); die **Schiebeweite** ist
    // rückwärts eine andere, und zwar für jeden der beiden Ausgänge eine
    // eigene.
    let gq_shift = s.k_frac + s.score_mult_frac;
    let gk_shift = s.q_frac + s.score_mult_frac;
    // i128, weil `gs · k · score_mult` über alle Positionen summiert
    // 2^31 · 2^15 · 2^15 · kv_len erreicht und damit i64 verlässt. Der
    // Faktor kommt nach der Summe dazu, sonst kostete er je Summand eine
    // Multiplikation.
    let mut gq_acc = vec![0i128; head_dim];
    let mut gk = Vec::with_capacity(kv_len);
    for j in 0..kv_len {
        if !mask[j] || gs[j] == 0 {
            gk.push(vec![0i32; head_dim]);
            continue;
        }
        let gsj = gs[j] as i64;
        let mut zeile = Vec::with_capacity(head_dim);
        for d in 0..head_dim {
            gq_acc[d] += (gsj as i128) * (k[j][d] as i128);
            zeile.push(clamp_i32(rshift_round_i64(
                gsj * (q[d] as i64) * s.score_mult,
                gk_shift,
            )));
        }
        gk.push(zeile);
    }
    // Ein Shift ganz am Ende, nicht je Summand.
    let gq = gq_acc
        .into_iter()
        .map(|a| {
            let mit = a * (s.score_mult as i128);
            let geschoben =
                crate::fixed_point::rshift_round_i128(mit, gq_shift as u32);
            clamp_i32(geschoben.clamp(i64::MIN as i128, i64::MAX as i128) as i64)
        })
        .collect();

    (gq, gk, gv)
}

// ---------------------------------------------------------------------------
// Der Verlust
// ---------------------------------------------------------------------------

/// Der Gradient der Kreuzentropie nach den Logits.
///
/// ```text
/// dL/dz = p − onehot(ziel)
/// ```
///
/// # ⚑ Der Gradient ist ganzzahlig, der Verlustwert nicht
///
/// Die Kreuzentropie selbst ist `−log p[ziel]`, und ein Logarithmus
/// gehört nicht in den Rechenpfad dieses Projekts. **Ihre Ableitung
/// braucht keinen:** Sie ist `p − onehot`, und `p` liefert
/// [`crate::softmax::softmax_int`] als ganze Zahl.
///
/// ⚑ **Damit steht die Grenze an der richtigen Stelle.** Was in den
/// Ganzzahlpfad eintritt und die Gewichte bewegt, ist vollständig
/// ganzzahlig und auf jeder Maschine dasselbe. Der **Verlustwert** ist
/// eine Diagnose für Menschen und kein Vergleichswert; wer ihn zum
/// Vergleichswert macht, hängt die Bitgleichheit an einen Logarithmus.
///
/// Der zurückgegebene Gradient liegt auf `prob_frac`: `p` trägt diese
/// Bruchstellen, und die Eins des Zielworts ist deshalb `2^prob_frac`.
///
/// # Panics
///
/// Wenn `ziel` ausserhalb des Vokabulars liegt. **Ein Zielwort, das es
/// nicht gibt, ist ein Aufruferfehler**, und ein stillschweigend
/// ignoriertes ergäbe einen Gradienten, der überall in dieselbe Richtung
/// zeigt.
pub fn kreuzentropie_gradient(p: &[Grad], ziel: usize, prob_frac: u8) -> Vec<Grad> {
    assert!(
        ziel < p.len(),
        "kreuzentropie_gradient: Zielwort {ziel} liegt ausserhalb des Vokabulars ({})",
        p.len()
    );
    let eins = 1i64 << prob_frac;
    let mut aus: Vec<Grad> = p.to_vec();
    aus[ziel] = clamp_i32(i64::from(aus[ziel]) - eins);
    aus
}

// ---------------------------------------------------------------------------
// Embedding
// ---------------------------------------------------------------------------

/// Rückwärts zum Embedding-Nachschlag am Anfang von `forward_token`.
///
/// Vorwärts wird genau eine Zeile der Tabelle gelesen; rückwärts landet
/// der ganze Gradient in genau dieser Zeile. Alle anderen bleiben null.
///
/// **Akkumulierend, nicht setzend.** Kommt ein Token in einer Sequenz
/// mehrfach vor, muss sich sein Gradient addieren. Wer hier zuweist,
/// behält den letzten Vorkommen und verliert alle davor, und das fällt
/// bei seltenen Token nie auf und bei häufigen als langsames Lernen.
pub fn embedding_backward_akkumulieren(
    ziel: &mut [i64],
    vocab_size: usize,
    token_id: usize,
    g: &[Grad],
) {
    let hidden = g.len();
    assert_eq!(ziel.len(), vocab_size * hidden, "embedding_backward: Zielgröße passt nicht");
    assert!(token_id < vocab_size, "embedding_backward: token_id außerhalb des Vokabulars");
    let start = token_id * hidden;
    for (d, gd) in g.iter().enumerate() {
        ziel[start + d] += *gd as i64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numerische Ableitung **des echten Vorwärtskernels**.
    ///
    /// Der eigentliche Prüfmaßstab dieses Moduls. Eine gegen die Formel
    /// geprüfte Ableitung sagt nur, dass zwei Menschen dieselbe Formel
    /// gelesen haben; sie fällt nicht auf, wenn der Vorwärtspfad etwas
    /// anderes rechnet als die Formel behauptet. Der zentrale Differenz-
    /// quotient auf dem echten Kernel fällt darauf sehr wohl auf.
    ///
    /// `h` in Einheiten des Eingangsrasters: kleiner geht nicht, denn
    /// der Eingang ist ganzzahlig.
    fn numerisch<F>(x: &[i16], j: usize, h: i16, mut f: F) -> f64
    where
        F: FnMut(&[i16]) -> f64,
    {
        let mut plus = x.to_vec();
        let mut minus = x.to_vec();
        plus[j] = plus[j].saturating_add(h);
        minus[j] = minus[j].saturating_sub(h);
        (f(&plus) - f(&minus)) / (2.0 * h as f64)
    }

    // ---- Übertragungsform -------------------------------------------

    /// Der Digest der Blockquantisierung: Skala aus dem Betragsmaximum,
    /// also **kann nichts sättigen**. Das ist keine Feinheit, sondern
    /// der Grund, warum in der Messung die Sättigung bei null lag und
    /// der Schaden stattdessen unten sitzt.
    #[test]
    fn blockquantisierung_saettigt_nie() {
        let g: Vec<Grad> = (0..256).map(|i| (i * i * 7 - 30000) as Grad).collect();
        let (q, shifts) = quantisiere_block(&g, 64, 8);
        assert_eq!(q.len(), g.len());
        assert_eq!(shifts.len(), 4);
        assert!(
            q.iter().all(|v| *v > i8::MIN),
            "ein Wert liegt am Sättigungsrand, die Skalenwahl greift nicht"
        );
    }

    /// Hin und zurück darf den Betrag nicht vergrößern und das
    /// Vorzeichen nicht drehen: Beides wäre ein Gradient, der in eine
    /// andere Richtung zeigt als der echte.
    #[test]
    fn blockquantisierung_bleibt_richtungstreu() {
        let g: Vec<Grad> = (0..128i32).map(|i| (i - 64) * 137).collect();
        let (q, shifts) = quantisiere_block(&g, 32, 8);
        let zurueck = entquantisiere_block(&q, &shifts, 32);
        for (a, b) in g.iter().zip(zurueck.iter()) {
            if *a != 0 && *b != 0 {
                assert_eq!(a.signum(), b.signum(), "Vorzeichen gedreht: {a} -> {b}");
            }
        }
    }

    /// **Die Auslöschung, gemessen statt behauptet.** Kleine Beträge
    /// werden zu null, und das ist die stille Hälfte des Problems: Die
    /// Sättigung oben gibt es nicht, die Auslöschung unten schon.
    #[test]
    fn blockquantisierung_loescht_kleine_werte_aus() {
        // Ein großer Ausreißer zwingt die Blockskala nach oben.
        let mut g: Vec<Grad> = vec![3; 64];
        g[0] = 1_000_000;
        let (q, shifts) = quantisiere_block(&g, 64, 8);
        let zurueck = entquantisiere_block(&q, &shifts, 64);
        let ausgeloescht = g
            .iter()
            .zip(zurueck.iter())
            .filter(|(a, b)| **a != 0 && **b == 0)
            .count();
        assert!(
            ausgeloescht > 50,
            "erwartet: der Ausreißer löscht den Rest aus, gemessen {ausgeloescht}"
        );
    }

    // ---- Mixture-of-Experts-Modell ----------------------------------

    /// Die Bausteine eines kleinen Gemischs: vier Experten, Top-2.
    fn gemisch() -> (Vec<i32>, Vec<Vec<i16>>, usize, usize, u8) {
        let logits: Vec<i32> = vec![120, 40, 200, 80];
        let ausgaben: Vec<Vec<i16>> = vec![
            vec![10, -20, 30],
            vec![-5, 15, -25],
            vec![40, 40, -40],
            vec![1, 2, 3],
        ];
        (logits, ausgaben, 4, 2, 8)
    }

    /// Ein Gemisch mit **eng beieinander liegenden** Logits, das nicht
    /// sättigt. Der Abstand ist mit Absicht klein: Genau dort lebt der
    /// Router-Gradient noch.
    fn gemisch_eng() -> (Vec<i32>, Vec<Vec<i16>>, usize, usize, u8) {
        let logits: Vec<i32> = vec![100, 40, 104, 80];
        let ausgaben: Vec<Vec<i16>> = vec![
            vec![10, -20, 30],
            vec![-5, 15, -25],
            vec![40, 40, -40],
            vec![1, 2, 3],
        ];
        (logits, ausgaben, 4, 2, 8)
    }

    /// Routet und mischt wie der Vorwärtspfad, mit derselben Tabelle.
    fn vorwaerts(
        logits: &[i32],
        alle_ausgaben: &[Vec<i16>],
        k: usize,
        frac: u8,
    ) -> (Vec<u16>, Vec<i32>, Vec<Vec<i16>>, Vec<i16>) {
        let lut = crate::moe::tests_exp_lut();
        let routing = crate::moe::route_top_k(logits, k, &lut, 0, frac, true);
        let gewaehlt: Vec<Vec<i16>> = routing
            .experten
            .iter()
            .map(|e| alle_ausgaben[*e as usize].clone())
            .collect();
        let y = crate::moe::mische_experten(&gewaehlt, &routing.gewichte, frac);
        (routing.experten, routing.gewichte, gewaehlt, y)
    }

    /// Die Mischgewichte **ohne den Boden**, also so, wie `route_top_k`
    /// sie bis zum 2026-09-05 lieferte.
    ///
    /// # ⚑ Warum es diesen Nachbau gibt
    ///
    /// Seit dem 2026-09-05 hebt `route_top_k` bei `norm_topk_prob` jedes
    /// Gewicht auf mindestens eins und entfernt damit den absorbierenden
    /// Zustand aus Fund 79. **Drei Tests dieses Moduls sind genau dafür
    /// da, ihn zu belegen**, und sie konnten ihn danach nicht mehr
    /// bauen; ihre Meldung lautete „der Aufbau saettigt nicht mehr".
    ///
    /// ⚑ **Der Zustand ist damit aus einem Weg verschwunden, nicht aus
    /// der Welt.** `moe_backward` bekommt seine Gewichte als Argument
    /// und kann sie von überall her bekommen: von einem Modell ohne
    /// `norm_topk_prob`, von einem anderen Router, aus einer Datei. Was
    /// es bei Sättigung tut, gehört deshalb weiter geprüft, und dafür
    /// wird sie hier von Hand gebaut.
    fn gewichte_ohne_boden(logits: &[i32], experten: &[u16], frac: u8) -> Vec<i32> {
        let lut = crate::moe::tests_exp_lut();
        let gewaehlte: Vec<i32> = experten.iter().map(|e| logits[*e as usize]).collect();
        let mut w = crate::softmax::softmax_int(&gewaehlte, &lut, 0, frac);
        // Dieselbe Summenkorrektur wie `moe::korrigiere_summe`, nur ohne
        // den Boden davor.
        let soll = 1i64 << frac;
        let ist: i64 = w.iter().map(|g| i64::from(*g)).sum();
        let rest = soll - ist;
        if rest != 0 {
            let mut groesster = 0usize;
            for (i, v) in w.iter().enumerate() {
                if *v > w[groesster] {
                    groesster = i;
                }
            }
            w[groesster] = (i64::from(w[groesster]) + rest) as i32;
        }
        w
    }

    /// ⚑ **Der Satz, um den es bei Mixture-of-Experts-Modellen geht: Ein nicht
    /// gewählter Experte bekommt exakt null.**
    ///
    /// Nicht „fast null" und nicht „vernachlässigbar". Bei
    /// `norm_topk_prob` läuft der Softmax nur über die gewählten Logits;
    /// die übrigen kommen in der Ausgabe nicht vor und haben deshalb die
    /// Ableitung null. Das ist die Grundlage dafür, dass ein
    /// hinzugefügter Experte mit minimalem Logit **tot** wäre, und
    /// deshalb steht der Satz als Test da und nicht als Behauptung.
    #[test]
    fn nicht_gewaehlte_experten_bekommen_exakt_null() {
        // Der enge Aufbau, weil der weite sättigt und dann **alle**
        // Gradienten null sind; die Gegenprobe unten könnte sonst nicht
        // greifen. Die Sättigung selbst prüft der Test darunter.
        let (logits, alle, n, k, frac) = gemisch_eng();
        let (experten, gewichte, gewaehlt, _) = vorwaerts(&logits, &alle, k, frac);
        let g: Vec<Grad> = vec![7, -3, 11];

        let grad = moe_backward(&g, &experten, &gewichte, &gewaehlt, n, frac, 6, 6);

        assert_eq!(grad.logits.len(), n);
        for i in 0..n {
            if experten.contains(&(i as u16)) {
                continue;
            }
            assert_eq!(
                grad.logits[i], 0,
                "Experte {i} wurde nicht gewaehlt und bekam trotzdem einen Gradienten"
            );
        }
        // Gegenprobe: Die gewählten bekommen nicht alle null, sonst
        // bewiese der Test oben, dass der Rückwärtspass nichts tut.
        assert!(
            experten.iter().any(|e| grad.logits[*e as usize] != 0),
            "kein einziger gewaehlter Experte bekam einen Gradienten"
        );
    }

    /// ⚑ **Das Akzeptanzkriterium für Training auf einem
    /// Mixture-of-Experts-Modell:** Zwei redundante Miner, die dasselbe Segment
    /// rechnen, liefern **bitgleiche** Gradienten, Routing-Entscheidung
    /// eingeschlossen.
    ///
    /// Der Test fährt den ganzen Weg zweimal, einschließlich Routing,
    /// und vergleicht byteweise. Ohne die Routing-Entscheidung im
    /// Vergleich bewiese er zu wenig: Zwei Läufe könnten verschiedene
    /// Experten wählen und danach zufällig ähnliche Zahlen liefern.
    #[test]
    fn zwei_laeufe_liefern_bitgleiche_gradienten() {
        let (logits, alle, n, k, frac) = gemisch();
        let g: Vec<Grad> = vec![7, -3, 11];
        let lauf = || {
            let (experten, gewichte, gewaehlt, y) = vorwaerts(&logits, &alle, k, frac);
            let grad = moe_backward(&g, &experten, &gewichte, &gewaehlt, n, frac, 6, 6);
            (experten, gewichte, y, grad)
        };
        assert_eq!(lauf(), lauf());
    }

    /// Gegen die numerische Ableitung des echten Mischkernels.
    ///
    /// Abgeleitet wird nach der **Ausgabe eines gewählten Experten**,
    /// denn das ist die Stelle, an der die Gradienten in die Experten
    /// zurücklaufen.
    #[test]
    fn der_gradient_je_ausgabe_trifft_die_numerische_ableitung() {
        let (logits, alle, n, k, frac) = gemisch();
        let (experten, gewichte, gewaehlt, _) = vorwaerts(&logits, &alle, k, frac);
        let c: Vec<Grad> = vec![3, -5, 2];

        let grad = moe_backward(&c, &experten, &gewichte, &gewaehlt, n, frac, 6, 6);

        // Verlust als Σ y_i · c_i, abgeleitet nach der Ausgabe des
        // ersten gewählten Experten.
        let verlust = |a0: &[i16]| -> f64 {
            let mut ausgaben = gewaehlt.clone();
            ausgaben[0] = a0.to_vec();
            let y = crate::moe::mische_experten(&ausgaben, &gewichte, frac);
            y.iter().zip(c.iter()).map(|(a, b)| *a as f64 * *b as f64).sum()
        };

        for (j, gj) in grad.je_ausgabe[0].iter().enumerate() {
            let num = numerisch(&gewaehlt[0], j, 16, verlust);
            let ana = *gj as f64;
            let abweichung = (num - ana).abs();
            let bezug = num.abs().max(ana.abs()).max(1.0);
            assert!(
                abweichung <= 2.0 || abweichung / bezug < 0.25,
                "Kanal {j}: numerisch {num:.2}, analytisch {ana:.2}"
            );
        }
    }

    /// Das Mischgewicht skaliert den Gradienten: Wer mehr Gewicht
    /// bekommt, bekommt mehr Gradient. Ohne diese Kopplung liefe das
    /// Routing im Rückwärtspass leer mit.
    #[test]
    fn ein_groesseres_mischgewicht_gibt_mehr_gradient() {
        let (logits, alle, n, k, frac) = gemisch();
        let (experten, gewichte, gewaehlt, _) = vorwaerts(&logits, &alle, k, frac);
        let g: Vec<Grad> = vec![1000, 1000, 1000];
        let grad = moe_backward(&g, &experten, &gewichte, &gewaehlt, n, frac, 6, 6);

        // Der erste gewählte Experte hat das größte Logit und damit das
        // größte Gewicht.
        assert!(gewichte[0] > gewichte[1], "Aufbau des Tests stimmt nicht");
        assert!(
            grad.je_ausgabe[0][0].abs() > grad.je_ausgabe[1][0].abs(),
            "das groessere Gewicht gab nicht mehr Gradient"
        );
    }

    /// ⚑ **Fund 79, Teil 1: Sättigung ist ein absorbierender Zustand.**
    ///
    /// Bei einem Logit-Abstand von 80 liefert der Ganzzahl-Softmax die
    /// Gewichte `(256, 0)`, und dann ist der Gradient **jedes** Logits
    /// exakt null, auch der des Gewinners. Der Router steht still und
    /// kann sich nie wieder ändern. Zusatzbits helfen dagegen nicht,
    /// und der Test hält genau das fest.
    #[test]
    fn ein_gesaettigter_router_hat_ueberall_gradient_null() {
        let (logits, alle, n, k, frac) = gemisch();
        let (experten, _, gewaehlt, _) = vorwaerts(&logits, &alle, k, frac);
        // ⚑ **Von Hand und nicht ueber `route_top_k`**, siehe
        // [`gewichte_ohne_boden`].
        let gewichte = gewichte_ohne_boden(&logits, &experten, frac);
        let g: Vec<Grad> = vec![7, -3, 11];

        assert_eq!(
            gewichte,
            vec![1 << frac, 0],
            "der Aufbau saettigt nicht mehr, dann prueft dieser Test etwas anderes"
        );

        for zusatz in [0u8, 3, 6] {
            let grad = moe_backward(&g, &experten, &gewichte, &gewaehlt, n, frac, 6, zusatz);
            assert!(
                grad.logits.iter().all(|d| *d == 0),
                "bei Saettigung kam mit {zusatz} Zusatzbits doch ein Gradient an: {:?}",
                grad.logits
            );
        }
    }

    /// ⚑ **Fund 79, Teil 2: Ohne Sättigung entscheidet die Skala.**
    ///
    /// Beim engen Aufbau lebt der Gradient noch, ist aber klein. Ohne
    /// Zusatzbits rundet er auf null, mit ihnen nicht. Das ist die
    /// Hälfte des Fundes, gegen die sich etwas tun lässt.
    #[test]
    fn ohne_zusatzbits_rundet_der_router_gradient_auf_null() {
        let (logits, alle, n, k, frac) = gemisch_eng();
        let (experten, gewichte, gewaehlt, _) = vorwaerts(&logits, &alle, k, frac);
        let g: Vec<Grad> = vec![7, -3, 11];

        assert!(
            gewichte[1] > 0,
            "der enge Aufbau saettigt doch: {gewichte:?}"
        );

        let ohne = moe_backward(&g, &experten, &gewichte, &gewaehlt, n, frac, 6, 0);
        let mit = moe_backward(&g, &experten, &gewichte, &gewaehlt, n, frac, 6, 6);

        assert!(
            mit.logits.iter().any(|d| *d != 0),
            "auch mit Zusatzbits kam nichts an: {:?}",
            mit.logits
        );
        // Und die Gegenprobe: Ohne Zusatzbits ist es weniger, nicht mehr.
        let summe = |v: &[Grad]| v.iter().map(|d| d.unsigned_abs() as u64).sum::<u64>();
        assert!(
            summe(&ohne.logits) < summe(&mit.logits),
            "ohne Zusatzbits kam nicht weniger an: {:?} gegen {:?}",
            ohne.logits,
            mit.logits
        );
    }

    // ---- Fund 79: die Spreizungsstrafe -------------------------------

    /// ⚑ **Die hergeleitete Schwelle gegen die gemessene Sättigung.**
    ///
    /// `saettigungsabstand` rechnet `(frac + 1) · ln2 · 2^exp_input`.
    /// Der Test glaubt der Formel nicht, sondern sucht mit dem echten
    /// `softmax_int` den Abstand, ab dem das kleinere Gewicht auf null
    /// fällt, und vergleicht. Ohne diesen Vergleich wäre die Schwelle
    /// eine hübsche Herleitung ohne Deckung.
    #[test]
    fn die_hergeleitete_schwelle_trifft_die_gemessene_saettigung() {
        // Eine Tabelle mit den **echten** Parametern des Projekts:
        // exp_input_frac_bits = 8, exp_lut_frac_bits = 14. Die
        // Spielzeugtabelle des Moduls hat eine zu kleine Domäne; an ihr
        // gemessen träfe der Test die Tabellenkante und nicht die
        // Rundung.
        let exp_in = 8u8;
        let lut_frac = 14u8;
        let lut: Vec<i16> = (0..4096)
            .map(|i| {
                let x = i as f64 / (1u32 << exp_in) as f64;
                ((1u32 << lut_frac) as f64 * (-x).exp()).round() as i16
            })
            .collect();

        for frac in [8u8, 14u8] {
            let gemessen = (1..4000)
                .find(|abstand| {
                    let w = crate::softmax::softmax_int(&[*abstand, 0], &lut, 0, frac);
                    w[1] == 0
                })
                .expect("irgendwann saettigt es");
            let hergeleitet = crate::moe::saettigungsabstand(frac, exp_in);

            // Die Formel rechnet mit dem stetigen exp, die Tabelle
            // rastert. Erwartet wird deshalb eine Übereinstimmung auf
            // zehn Prozent, nicht auf die Einheit.
            let abweichung = (gemessen - hergeleitet).abs() as f64;
            let bezug = hergeleitet as f64;
            assert!(
                abweichung / bezug < 0.10,
                "frac {frac}: gemessen {gemessen}, hergeleitet {hergeleitet}"
            );
        }
    }

    /// ⚑ Der Vergleich, der die Einordnung trägt: Gleitkomma hat
    /// denselben Zustand, nur viel später.
    ///
    /// Ohne diese Zahl liest sich Fund 79 so, als sei Router-Kollaps ein
    /// Erzeugnis des Ganzzahlpfads. Er ist es nicht; die Tabelle macht
    /// ihn nur um eine Größenordnung leichter erreichbar.
    #[test]
    fn der_ganzzahlpfad_saettigt_frueher_als_gleitkomma() {
        // In nats: (frac + 1) · ln2 für den Ganzzahlpfad.
        let nats = |frac: u8| (frac as f64 + 1.0) * std::f64::consts::LN_2;
        assert!((nats(14) - 10.4).abs() < 0.1, "prob_frac_bits 14 liegt bei 10,4 nats");

        // f32 verliert das kleinere Gewicht erst, wenn es unter die
        // kleinste subnormale Zahl faellt.
        let f32_grenze = (1.0f64 / f32::MIN_POSITIVE as f64).ln();
        assert!(f32_grenze > 80.0, "f32 sollte weit spaeter saettigen");
        assert!(
            f32_grenze / nats(14) > 8.0,
            "der Abstand zwischen Ganzzahl und f32 ist kleiner als gedacht: {:.0}",
            f32_grenze / nats(14)
        );
    }

    /// Ein gesunder Router wird **exakt** nicht angefasst.
    ///
    /// Nicht „kaum" und nicht „wenig": null an jeder Stelle. Eine Strafe,
    /// die im Normalfall etwas tut, verschiebt das Modell dauerhaft, und
    /// niemand sähe woran.
    #[test]
    fn ein_gesunder_router_bleibt_exakt_unberuehrt() {
        let (logits, alle, _, k, frac) = gemisch_eng();
        let (experten, _, _, _) = vorwaerts(&logits, &alle, k, frac);
        let d = router_spreizung(&logits, &experten, 100, 0);
        assert!(
            d.iter().all(|x| *x == 0),
            "der gesunde Router wurde verbogen: {d:?}"
        );
    }

    /// ⚑ **Die Strafe verschiebt den Logit-Mittelwert nicht.**
    ///
    /// Sie staucht die Spreizung und sonst nichts. Ohne diese
    /// Eigenschaft zöge sie das Routing über viele Schritte in eine
    /// Richtung, ohne dass es jemandem auffiele.
    #[test]
    fn die_strafe_summiert_sich_zu_null() {
        for schwelle in [0i32, 10, 40] {
            let logits: Vec<i32> = vec![300, 40, 100, 80];
            let experten: Vec<u16> = vec![0, 2, 3];
            let d = router_spreizung(&logits, &experten, schwelle, 0);
            let summe: i64 = d.iter().map(|x| *x as i64).sum();
            assert_eq!(summe, 0, "Schwelle {schwelle}: Summe {summe} statt null");
        }
    }

    /// ⚑ **Der Nachweis, um den es geht: Die Strafe wirkt genau dort, wo
    /// der Softmax-Gradient verschwunden ist.**
    ///
    /// Derselbe gesättigte Aufbau, zwei Rückwärtswege. Der über den
    /// Softmax liefert überall null; die Spreizungsstrafe liefert einen
    /// Schub. Sie liest die Logit-Abstände und nicht die quantisierten
    /// Gewichte, und deshalb ist ihr die Sättigung gleichgültig.
    #[test]
    fn die_strafe_wirkt_wo_der_softmax_gradient_null_ist() {
        let (logits, alle, n, k, frac) = gemisch();
        let (experten, _, gewaehlt, _) = vorwaerts(&logits, &alle, k, frac);
        let gewichte = gewichte_ohne_boden(&logits, &experten, frac);
        assert_eq!(gewichte, vec![1 << frac, 0], "der Aufbau saettigt nicht mehr");

        let g: Vec<Grad> = vec![7, -3, 11];
        let ueber_softmax = moe_backward(&g, &experten, &gewichte, &gewaehlt, n, frac, 6, 6);
        assert!(
            ueber_softmax.logits.iter().all(|d| *d == 0),
            "der Softmax-Weg liefert doch etwas: {:?}",
            ueber_softmax.logits
        );

        let strafe = router_spreizung(&logits, &experten, 20, 0);
        assert!(
            strafe.iter().any(|d| *d != 0),
            "die Strafe liefert nichts, dann hilft sie auch nicht: {strafe:?}"
        );
        // Und sie zeigt in die richtige Richtung: Der Verlierer wird
        // hochgeschoben, der Gewinner heruntergezogen.
        assert!(strafe[experten[1] as usize] > 0, "der Verlierer wird nicht hochgeschoben");
        assert!(strafe[experten[0] as usize] < 0, "der Gewinner wird nicht heruntergezogen");
    }

    /// ⚑ **Der Ausstieg aus dem absorbierenden Zustand, als Lauf.**
    ///
    /// Ein gesättigter Router bekommt nur die Spreizungsstrafe, Schritt
    /// für Schritt, ohne jeden anderen Gradienten. Nach endlich vielen
    /// Schritten sättigt er nicht mehr, und der Softmax-Gradient lebt
    /// wieder. **Das ist die Stabilisierung, und sie ist damit kein
    /// Argument, sondern ein Lauf.**
    #[test]
    fn wiederholte_strafe_fuehrt_aus_der_saettigung_heraus() {
        let (start, alle, n, k, frac) = gemisch();
        let mut logits = start.clone();
        let (experten, _, _, _) = vorwaerts(&logits, &alle, k, frac);
        let gewichte = gewichte_ohne_boden(&logits, &experten, frac);
        assert_eq!(gewichte, vec![1 << frac, 0], "der Aufbau saettigt nicht");

        let g: Vec<Grad> = vec![7, -3, 11];
        let schwelle = 20;
        let mut schritte = 0;
        let mut befreit = false;
        for _ in 0..200 {
            let d = router_spreizung(&logits, &experten, schwelle, 2);
            if d.iter().all(|x| *x == 0) {
                break;
            }
            for (z, dz) in logits.iter_mut().zip(d.iter()) {
                *z += *dz;
            }
            schritte += 1;
            // ⚑ **Gemessen ohne Boden**, sonst prüfte der Test den
            // Boden statt die Strafe: Mit ihm ist jedes Gewicht ab dem
            // ersten Schritt über null, und „befreit" wäre trivial wahr.
            let w = gewichte_ohne_boden(&logits, &experten, frac);
            if w.iter().all(|x| *x > 0) {
                befreit = true;
                break;
            }
        }
        assert!(
            befreit,
            "nach {schritte} Schritten saettigt es immer noch: {logits:?}"
        );

        // Und der Beleg, dass es wirklich der Ausstieg ist: Jetzt kommt
        // wieder ein Gradient durch, wo vorher null stand.
        let (e2, _, a2, _) = vorwaerts(&logits, &alle, k, frac);
        let w2 = gewichte_ohne_boden(&logits, &e2, frac);
        let jetzt = moe_backward(&g, &e2, &w2, &a2, n, frac, 6, 6);
        assert!(
            jetzt.logits.iter().any(|d| *d != 0),
            "der Router ist frei und liefert trotzdem nichts: {:?}",
            jetzt.logits
        );
    }

    /// Zwei Läufe, dasselbe Ergebnis. Ohne das wäre die Strafe kein
    /// zulässiger Teil eines verifizierbaren Trainingsschritts.
    #[test]
    fn die_strafe_ist_deterministisch() {
        let logits: Vec<i32> = vec![300, 40, 100, 80];
        let experten: Vec<u16> = vec![0, 2, 3];
        assert_eq!(
            router_spreizung(&logits, &experten, 20, 1),
            router_spreizung(&logits, &experten, 20, 1)
        );
    }

    /// ⚑ Ein Experte, der mit minimalem Logit eingehängt wird, ist tot.
    ///
    /// Das ist der Grund, warum Expertenwachstum **nicht** dasselbe ist
    /// wie Breitenwachstum. Der Weg über das minimale Logit ist der
    /// einzige exakt funktionserhaltende, und er erkauft die Erhaltung
    /// damit, dass der neue Experte nie einen Gradienten sieht. Der Test
    /// hält das fest, damit ein späterer Entwurf sich daran messen muss.
    #[test]
    fn ein_neuer_experte_mit_minimalem_logit_bleibt_ohne_gradient() {
        let (mut logits, mut alle, _, k, frac) = gemisch();
        // Der neue Experte: eine Kopie des ersten, mit dem kleinsten
        // Logit der Reihe.
        let kleinstes = *logits.iter().min().expect("nicht leer");
        logits.push(kleinstes - 1);
        alle.push(alle[0].clone());
        let n = logits.len();

        let (experten, gewichte, gewaehlt, _) = vorwaerts(&logits, &alle, k, frac);
        assert!(
            !experten.contains(&((n - 1) as u16)),
            "der neue Experte wurde gewaehlt, der Test prueft dann etwas anderes"
        );
        let g: Vec<Grad> = vec![7, -3, 11];
        let grad = moe_backward(&g, &experten, &gewichte, &gewaehlt, n, frac, 6, 6);
        assert_eq!(
            grad.logits[n - 1],
            0,
            "der neue Experte bekam einen Gradienten, dann waere er nicht tot"
        );
    }

    // ---- Lineare Schicht --------------------------------------------

    /// Gegen die numerische Ableitung des echten `linear_w8a16`.
    #[test]
    fn linear_backward_trifft_die_numerische_ableitung() {
        let in_f = 8usize;
        let out_f = 4usize;
        let x: Vec<i16> = (0..in_f).map(|i| (i as i16 * 37 - 100) * 8).collect();
        let w: Vec<i8> = (0..in_f * out_f).map(|i| (((i * 13) % 97) as i32 - 48) as i8).collect();
        let shifts = vec![4u8; out_f];
        let (act_frac, out_frac) = (6u8, 6u8);

        // Skalarer Verlust: Σ y_i · c_i, damit dL/dy = c bekannt ist.
        let c: Vec<Grad> = vec![3, -5, 2, 7];
        let verlust = |xx: &[i16]| -> f64 {
            let y = crate::linear::linear_w8a16(xx, &w, in_f, &shifts, act_frac, out_frac);
            y.iter().zip(c.iter()).map(|(a, b)| *a as f64 * *b as f64).sum()
        };

        let (gx, gw) = linear_backward(&c, &x, &w, in_f, &shifts, out_frac, out_frac);
        assert_eq!(gw.len(), in_f * out_f);

        for (j, gxj) in gx.iter().enumerate() {
            let num = numerisch(&x, j, 4, verlust);
            let ana = *gxj as f64;
            // Die Toleranz folgt aus dem Raster: Der Eingang ist
            // ganzzahlig, die Ausgabe auch, und beide runden.
            let abweichung = (num - ana).abs();
            let bezug = num.abs().max(ana.abs()).max(1.0);
            assert!(
                abweichung <= 2.0 || abweichung / bezug < 0.25,
                "Kanal {j}: numerisch {num:.2}, analytisch {ana:.2}"
            );
        }
    }

    /// `dL/dW` ist das äußere Produkt, ohne Reduktion: exakt prüfbar.
    #[test]
    fn linear_backward_liefert_das_aeussere_produkt() {
        let x: Vec<i16> = vec![3, -7, 11];
        let g: Vec<Grad> = vec![5, -2];
        let w = vec![0i8; 6];
        let (_, gw) = linear_backward(&g, &x, &w, 3, &[0, 0], 0, 0);
        assert_eq!(gw, vec![15, -35, 55, -6, 14, -22]);
    }

    // ---- Softmax -----------------------------------------------------

    /// Gegen die numerische Ableitung des echten `softmax_int`.
    #[test]
    fn softmax_backward_trifft_die_numerische_ableitung() {
        let frac = 12u8;
        let lut: Vec<i16> = (0..128)
            .map(|i| ((-(i as f64) / 256.0).exp() * 256.0).round() as i16)
            .collect();
        let logits: Vec<i32> = vec![900, -400, 1500, 200];
        let c: Vec<Grad> = vec![2, -3, 5, 1];

        let verlust = |zz: &[i32]| -> f64 {
            let p = crate::softmax::softmax_int(zz, &lut, 8, frac);
            p.iter().zip(c.iter()).map(|(a, b)| *a as f64 * *b as f64).sum()
        };

        let p = crate::softmax::softmax_int(&logits, &lut, 8, frac);
        let gz = softmax_backward(&c, &p, frac);

        for j in 0..logits.len() {
            let mut plus = logits.clone();
            let mut minus = logits.clone();
            plus[j] += 64;
            minus[j] -= 64;
            let num = (verlust(&plus) - verlust(&minus)) / 128.0;
            let ana = gz[j] as f64;
            // **Absoluter Boden neben der relativen Schranke.** Bei
            // Logit 1 liegt p praktisch bei null, die numerische
            // Ableitung ist exakt 0 und die analytische ein LSB. Ein
            // relativer Vergleich macht daraus 100 % Abweichung, obwohl
            // beide dasselbe sagen: nichts. Zwei LSB Spielraum, mehr
            // nicht.
            let abweichung = (num - ana).abs();
            let bezug = num.abs().max(ana.abs()).max(1.0);
            assert!(
                abweichung <= 2.0 || abweichung / bezug < 0.30,
                "Logit {j}: numerisch {num:.3}, analytisch {ana:.3}"
            );
        }
    }

    /// **Die Erhaltungsgröße der Softmax-Ableitung.** Ein konstanter
    /// eingehender Gradient darf nichts bewirken: Softmax ist invariant
    /// gegen eine gemeinsame Verschiebung aller Logits, also muss die
    /// Ableitung in dieser Richtung null sein. Das prüft die Formel
    /// schärfer als jeder Zahlenvergleich.
    #[test]
    fn ein_konstanter_gradient_bewirkt_nichts() {
        let frac = 12u8;
        // **Muss sich zu 2^frac summieren.** Meine erste Fassung nahm
        // 1024+2048+1024+2048 = 6144 statt 4096 und war damit keine
        // Softmax-Ausgabe; der Test schlug zu Recht fehl, nur nicht aus
        // dem Grund, den er prüfen sollte.
        let p: Vec<Grad> = vec![1 << 10, 1 << 10, 1 << 10, 1 << 10]; // Summe = 2^12
        let g: Vec<Grad> = vec![7; 4];
        let gz = softmax_backward(&g, &p, frac);
        for (i, v) in gz.iter().enumerate() {
            assert!(v.abs() <= 1, "Kanal {i}: {v}, erwartet ~0");
        }
    }

    // ---- RMSNorm -----------------------------------------------------

    /// **Der zweite Term ist nicht optional.** Ohne ihn ist der Gradient
    /// eine andere Funktion, und der Unterschied wächst mit der Länge des
    /// Vektors. Der Test hält fest, dass er wirkt.
    #[test]
    fn rmsnorm_backward_traegt_den_normierungsterm() {
        let n = 16usize;
        let x: Vec<i16> = (0..n).map(|i| i as i16 * 91 - 700).collect();
        let gamma: Vec<i8> = vec![64; n];
        let gshifts = vec![6u8; n];
        let g: Vec<Grad> = (0..n).map(|i| (i as i32 % 5) - 2).collect();
        let inv_n = ((1i64 << 20) as f64 / n as f64).round() as i64;

        let (gx, ggamma) = rmsnorm_backward(
            &g, &x, &vec![0u8; n], &gamma, &gshifts,
            Normskalen { r: 1 << 12, norm_frac: 12, ref_shift: 0, inv_n_q20: inv_n, g_frac: 8, gx_frac: 8 },
        );
        assert_eq!(gx.len(), n);
        assert_eq!(ggamma.len(), n);

        // Ohne den zweiten Term wäre dL/dx exakt proportional zu g.
        // Mit ihm ist es das nicht, und genau das ist zu zeigen.
        let proportional = gx
            .iter()
            .zip(g.iter())
            .all(|(a, b)| *b == 0 || (*a as f64 / *b as f64 - gx[0] as f64 / g[0] as f64).abs() < 1e-9);
        assert!(!proportional, "der Normierungsterm fehlt: dL/dx ist proportional zu g");
    }

    /// Ein Nullgradient darf nichts erzeugen. Klingt banal, fängt aber
    /// jeden Vorzeichen- und Offsetfehler in beiden Termen.
    #[test]
    fn ohne_eingehenden_gradienten_kein_ausgehender() {
        let n = 8usize;
        let x: Vec<i16> = (0..n).map(|i| i as i16 * 13 + 5).collect();
        let gamma = vec![32i8; n];
        let gshifts = vec![5u8; n];
        let inv_n = ((1i64 << 20) as f64 / n as f64).round() as i64;
        let (gx, gg) = rmsnorm_backward(
            &vec![0; n], &x, &vec![0u8; n], &gamma, &gshifts,
            Normskalen { r: 1 << 12, norm_frac: 12, ref_shift: 0, inv_n_q20: inv_n, g_frac: 8, gx_frac: 8 },
        );
        assert!(gx.iter().all(|v| *v == 0), "{gx:?}");
        assert!(gg.iter().all(|v| *v == 0), "{gg:?}");
    }

    /// ⚑ **Gegen die geschlossene Form der Ableitung, mit paarweise
    /// verschiedenen Kanalskalen (Fund 175).**
    ///
    /// # ⚑ Warum hier keine numerische Ableitung steht
    ///
    /// Jeder andere Rueckwaertskern wird gegen die numerische Ableitung
    /// des echten Vorwaertskerns geprueft. **Hier traegt das nicht**, und
    /// der Grund ist die Tabelle: `r` ist eine **ganze** Zahl aus der
    /// rsqrt-Tabelle, also ist `dr/dx` eine Treppe. Ein Schub an einem
    /// grossen Kanal verschiebt den Tabellenindex um eine Stufe, und die
    /// Differenz misst dann den Sprung statt der Steigung. Gemessen:
    /// Die vier kleinen Kanaele trafen auf drei Promille, die vier
    /// grossen streuten zwischen -5 und +9.
    ///
    /// **Verglichen wird deshalb mit der Formel selbst**, ausgerechnet
    /// aus denselben **realen** Groessen. Das ist keine zweite Umsetzung
    /// des Vorwaertspasses, sondern die Spezifikation des
    /// Rueckwaertspasses: Genau ihre Skalenbuchhaltung war falsch.
    ///
    /// ⛑ **Die beiden Tests darueber koennen den Fehler nicht sehen.**
    /// „Der zweite Term wirkt" und „ohne Gradient kein Gradient" sind von
    /// jeder Skala unabhaengig. Mit der alten Fassung streuten die
    /// Verhaeltnisse hier von **1,08 bis -37,4**.
    #[test]
    fn rmsnorm_backward_trifft_die_geschlossene_form() {
        const N: usize = 8;
        const OUT: u8 = 13;
        let lut: Vec<i16> = (0..4096)
            .map(|x| if x == 0 { 256 } else { (4096.0 / (x as f64).sqrt()).round() as i16 })
            .collect();
        let x: Vec<i16> = vec![300, -180, 90, 240, -60, 410, -320, 150];
        // ⚑ **Absichtlich verschieden.** Der Vorwaertspass traegt eine
        // Skala je Kanal (Fund 20); mit lauter gleichen Werten pruefte
        // dieser Test genau das nicht, woran die alte Fassung scheiterte.
        let xs: Vec<u8> = vec![6, 6, 7, 6, 8, 6, 5, 7];
        let gamma: Vec<i8> = vec![64, -40, 90, 32, -70, 55, 20, -100];
        let gs: Vec<u8> = vec![5, 6, 7, 5, 6, 7, 5, 6];
        let inv_n = ((1i64 << 20) as f64 / N as f64).round() as i64;

        let mut spur = crate::rmsnorm::Rmsnormspur::Leer;
        let _ = crate::rmsnorm::rmsnorm_i16_mit_spur(
            &x, &xs, &gamma, &gs, &lut, 0, 12, inv_n, OUT, Some(&mut spur),
        );
        let crate::rmsnorm::Rmsnormspur::Wert { r, norm_frac, ref_shift } = spur else {
            panic!("der Vorwaertspass hat kein r hinterlassen");
        };
        let b = 12u8;
        // ⚑ **Die Vorzeichen sind gewaehlt, nicht gewuerfelt.** Mit
        // `+6` an Stelle sechs hob sich `Σ g·γ·x` fast auf (−0,38 statt
        // 74,6), und dann traegt der zweite Term nichts zum Ergebnis
        // bei: Der Test prueft dann nur den ersten. **Eine Summe, die
        // sich aufhebt, ist eine Pruefung, die nichts auswaehlt.**
        let g: Vec<Grad> = [3i32, -2, 5, 1, -4, 2, -6, -1].iter().map(|c| c << b).collect();

        let (gx, ggamma) = rmsnorm_backward(
            &g, &x, &xs, &gamma, &gs,
            Normskalen { r, norm_frac, ref_shift, inv_n_q20: inv_n, g_frac: b, gx_frac: b },
        );

        let rf = i32::from(norm_frac) - i32::from(ref_shift);
        let r_real = f64::from(r) / 2f64.powi(rf);
        let xr: Vec<f64> = (0..N).map(|i| f64::from(x[i]) / 2f64.powi(i32::from(xs[i]))).collect();
        let gr: Vec<f64> =
            (0..N).map(|i| f64::from(gamma[i]) / 2f64.powi(i32::from(gs[i]))).collect();
        let gg: Vec<f64> = (0..N).map(|i| f64::from(g[i]) / 2f64.powi(i32::from(b))).collect();
        let summe: f64 = (0..N).map(|i| gg[i] * gr[i] * xr[i]).sum();
        let bus = 2f64.powi(i32::from(b));

        for j in 0..N {
            let erwartet =
                bus * (r_real * gr[j] * gg[j] - (r_real.powi(3) * xr[j] / N as f64) * summe);
            let verhaeltnis = f64::from(gx[j]) / erwartet;
            assert!(
                erwartet.abs() > 100.0,
                "Kanal {j}: die Erwartung ist zu klein zum Vergleichen ({erwartet:.1})"
            );
            assert!(
                (0.95..=1.05).contains(&verhaeltnis),
                "Kanal {j}: dL/dx ist {}, die geschlossene Form sagt {erwartet:.1}, \
                 also das {verhaeltnis:.3}-fache",
                gx[j]
            );
            let erw_gamma = gg[j] * xr[j] * r_real * bus;
            let abweichung = (ggamma[j] as f64 - erw_gamma).abs();
            assert!(
                abweichung <= erw_gamma.abs() * 0.01 + 2.0,
                "Kanal {j}: dL/dgamma ist {}, die geschlossene Form sagt {erw_gamma:.1}",
                ggamma[j]
            );
        }
    }

    // ---- Determinismus -----------------------------------------------

    /// **Der Vertrag aus `dot.rs`, hier noch einmal.** Die Reduktionen im
    /// Rückwärtspass laufen exakt in i64, also ist jede
    /// Summationsreihenfolge dieselbe Zahl. Ohne diese Eigenschaft wäre
    /// verifizierbares Training unmöglich: Zwei Miner mit verschiedener
    /// Parallelisierung bekämen verschiedene Gradienten.
    #[test]
    fn die_reihenfolge_der_reduktion_aendert_nichts() {
        let in_f = 64usize;
        let out_f = 8usize;
        let x: Vec<i16> = (0..in_f).map(|i| ((i * 37) % 1000) as i16 - 500).collect();
        let w: Vec<i8> = (0..in_f * out_f)
            .map(|i| i32::try_from((i * 29) % 251).unwrap_or(0).wrapping_sub(125) as i8)
            .collect();
        let shifts = vec![5u8; out_f];
        let g: Vec<Grad> = (0..out_f).map(|i| (i as i32 * 17) - 60).collect();

        let (vorwaerts, _) = linear_backward(&g, &x, &w, in_f, &shifts, 8, 8);

        // Dieselbe Rechnung mit umgekehrter Reihenfolge der Ausgaben.
        let mut g_rev = g.clone();
        g_rev.reverse();
        let mut w_rev: Vec<i8> = Vec::with_capacity(w.len());
        for zeile in w.chunks_exact(in_f).rev() {
            w_rev.extend_from_slice(zeile);
        }
        let (rueckwaerts, _) = linear_backward(&g_rev, &x, &w_rev, in_f, &shifts, 8, 8);

        assert_eq!(vorwaerts, rueckwaerts, "die Reduktionsreihenfolge wirkt");
    }

    /// Zweimal dasselbe muss zweimal dasselbe sein. Der billigste Test
    /// des Moduls und der, dessen Ausfall am teuersten wäre.
    #[test]
    fn zweimal_gerechnet_ist_zweimal_dasselbe() {
        let g: Vec<Grad> = (0..97i32).map(|i| (i * 31 % 511) - 255).collect();
        let a = quantisiere_block(&g, 16, 8);
        let b = quantisiere_block(&g, 16, 8);
        assert_eq!(a, b);
    }

    /// **Der Fehler von 2026-08-22, als Test festgehalten.**
    ///
    /// Die erste Fassung von [`linear_backward`] schob jeden Summanden
    /// einzeln nach rechts und addierte danach. Bei kleinen Produkten
    /// rundet jeder Summand für sich auf null, und die Summe ist null,
    /// obwohl der wahre Gradient es nicht ist.
    ///
    /// Der Test baut genau diesen Fall: viele kleine Beiträge, deren
    /// Einzelrundung null ergäbe, deren Summe aber deutlich davon
    /// abweicht. Er ist die Gegenprobe zur Behebung, nicht ihre
    /// Wiederholung.
    #[test]
    fn viele_kleine_beitraege_gehen_nicht_verloren() {
        let in_f = 4usize;
        let out_f = 64usize;
        // Jedes Produkt ist 1 · 1 = 1, der Shift ist 5: einzeln
        // gerundet wäre jeder Beitrag null.
        let x = vec![0i16; in_f];
        let w = vec![1i8; in_f * out_f];
        let shifts = vec![5u8; out_f];
        let g: Vec<Grad> = vec![1; out_f];

        let (gx, _) = linear_backward(&g, &x, &w, in_f, &shifts, 0, 0);

        // Wahr: 64 Beiträge à 1, geteilt durch 2^5 = 2.
        let erwartet = (out_f as i32) >> 5;
        for (j, v) in gx.iter().enumerate() {
            assert_eq!(
                *v, erwartet,
                "Kanal {j}: {v} statt {erwartet}. Einzeln gerundet käme hier 0 heraus."
            );
        }
    }

    /// Dasselbe für die Summe in [`rmsnorm_backward`]: Eine breite
    /// Shift-Spanne darf die feinskalierten Kanäle nicht auslöschen.
    /// Das ist Fund 24 in der Rückwärtsrichtung.
    #[test]
    fn breite_gamma_spanne_loescht_keine_kanaele_aus() {
        let n = 32usize;
        // Ein Kanal grob (Shift 2), der Rest fein (Shift 12): Ohne
        // Ausrichtung nach oben trüge nur der grobe Kanal zur Summe bei.
        let mut gshifts = vec![12u8; n];
        gshifts[0] = 2;
        // **Kräftige Werte, damit der Unterschied das Ausgaberaster
        // erreicht.** Eine erste Fassung nahm x = 100 und g = 1; der
        // Unterschied entstand in der Summe und ging in der Skalierung
        // danach wieder unter. Der Test wäre grün geworden, ohne die
        // Eigenschaft zu prüfen, und das ist schlimmer als rot.
        let x: Vec<i16> = vec![30000; n];
        let gamma: Vec<i8> = vec![127; n];
        let g: Vec<Grad> = vec![64; n];
        let inv_n = ((1i64 << 20) as f64 / n as f64).round() as i64;
        // ⚑ **Die Skalen muessen zueinander passen, seit Fund 175.** Ein
        // Eingang von 30 000 auf Verschiebung null ist real 30 000, und
        // dazu ein `r` von eins waere die Behauptung, der quadratische
        // Mittelwert sei eins. Der zweite Term wird dann so gross, dass
        // alles saettigt. Mit Verschiebung 15 ist der reale Eingang
        // 0,92, und `r = 1` passt dazu.
        let xs = vec![15u8; n];
        let skalen = Normskalen {
            r: 1 << 12,
            norm_frac: 27,
            ref_shift: 15,
            inv_n_q20: inv_n,
            g_frac: 8,
            gx_frac: 8,
        };

        let (mit_feinen, _) = rmsnorm_backward(&g, &x, &xs, &gamma, &gshifts, skalen);

        // Dieselbe Rechnung, aber die feinen Kanäle auf null gesetzt:
        // Wenn sie nichts beitrügen, käme dasselbe heraus.
        let mut g_nur_grob = vec![0; n];
        g_nur_grob[0] = 64;
        let (nur_grob, _) = rmsnorm_backward(&g_nur_grob, &x, &xs, &gamma, &gshifts, skalen);

        assert_ne!(
            mit_feinen, nur_grob,
            "die feinskalierten Kanäle tragen nichts bei: Fund 24 in der Rückwärtsrichtung"
        );
    }


    // ---- RoPE --------------------------------------------------------

    /// **Die Rotation ist orthogonal, also ist ihr Rückwärtspass ihre
    /// Umkehrung.** Vorwärts drehen, rückwärts zurückdrehen, und man
    /// steht wieder am Anfang. Das prüft die Vorzeichen schärfer als
    /// jeder Zahlenvergleich: Ein vertauschtes Vorzeichen dreht in die
    /// falsche Richtung und kommt nicht zurück.
    #[test]
    fn rope_rueckwaerts_dreht_die_drehung_zurueck() {
        let frac = 14u8;
        let half = 4usize;
        // Ein Winkel je Paar, wie im Vorwärtspfad.
        let winkel = [0.3f64, 0.7, 1.1, 2.5];
        let cos: Vec<i16> = winkel.iter().map(|a| (a.cos() * (1 << frac) as f64).round() as i16).collect();
        let sin: Vec<i16> = winkel.iter().map(|a| (a.sin() * (1 << frac) as f64).round() as i16).collect();

        let x: Vec<i16> = vec![3000, -1200, 800, 2500, -900, 1700, -2200, 400];
        let gedreht = crate::rope::rotate_half_split_i16(&x, &cos, &sin, frac);
        let g: Vec<Grad> = gedreht.iter().map(|v| *v as Grad).collect();
        let zurueck = rope_backward(&g, &cos, &sin, frac);

        for j in 0..2 * half {
            let abw = (zurueck[j] - x[j] as i32).abs();
            assert!(
                abw <= 4,
                "Kanal {j}: {} statt {} (Abweichung {abw})",
                zurueck[j], x[j]
            );
        }
    }

    /// Bei Winkel null ist die Drehung die Identität, vorwärts wie
    /// rückwärts. Fängt jeden Offsetfehler in der Paarbildung.
    #[test]
    fn rope_bei_winkel_null_ist_identitaet() {
        let frac = 14u8;
        let eins = 1i16 << frac;
        let cos = vec![eins; 3];
        let sin = vec![0i16; 3];
        let g: Vec<Grad> = vec![5, -9, 13, 21, -3, 7];
        let out = rope_backward(&g, &cos, &sin, frac);
        assert_eq!(out, g);
    }

    // ---- Attention ----------------------------------------------------

    /// Skalen für die kleinen Attention-Proben: alle Aktivierungsskalen
    /// gleich, damit diese Tests von der Skalenfrage unabhängig bleiben.
    /// Wo sie **nicht** unabhängig ist, steht
    /// `der_gradient_nach_q_und_k_trifft_die_numerische_ableitung`.
    fn probeskalen(prob_frac: u8) -> Aufmerksamkeitsskalen {
        Aufmerksamkeitsskalen {
            score_mult: 1 << 15,
            score_mult_frac: 15,
            q_frac: 0,
            k_frac: 0,
            v_frac: prob_frac,
            prob_frac,
        }
    }

    /// **Maskierte Positionen bekommen exakt null.** Vorwärts sind sie
    /// „ungefähr null", und das ist im Rückwärtspass kein Argument: Ein
    /// Gradient auf eine Position, die nie gelesen wurde, wäre ein Leck
    /// über die Kausalitätsgrenze.
    #[test]
    fn maskierte_positionen_bekommen_keinen_gradienten() {
        let head_dim = 4usize;
        let q: Vec<i16> = vec![100, -50, 25, 75];
        let k: Vec<Vec<i16>> = vec![vec![10, 20, 30, 40], vec![50, 60, 70, 80], vec![1, 2, 3, 4]];
        let v: Vec<Vec<i16>> = vec![vec![5; head_dim], vec![7; head_dim], vec![11; head_dim]];
        let p: Vec<Grad> = vec![1 << 7, 1 << 7, 0];
        let mask = vec![true, true, false];
        let g: Vec<Grad> = vec![3, -4, 5, -6];

        let (_, gk, gv) = attention_backward(&g, &q, &k, &v, &p, &mask, probeskalen(8));

        assert!(gk[2].iter().all(|x| *x == 0), "gk der maskierten Position: {:?}", gk[2]);
        assert!(gv[2].iter().all(|x| *x == 0), "gv der maskierten Position: {:?}", gv[2]);
        // Und die unmaskierten sind es nicht.
        assert!(gv[0].iter().any(|x| *x != 0), "gv[0] ist null, der Test misst nichts");
    }

    /// Ohne eingehenden Gradienten kein ausgehender, über alle drei
    /// Ausgänge.
    #[test]
    fn attention_ohne_gradient_bleibt_still() {
        let q: Vec<i16> = vec![10, 20];
        let k: Vec<Vec<i16>> = vec![vec![1, 2], vec![3, 4]];
        let v: Vec<Vec<i16>> = vec![vec![5, 6], vec![7, 8]];
        let p: Vec<Grad> = vec![1 << 7, 1 << 7];
        let mask = vec![true, true];
        let (gq, gk, gv) = attention_backward(&[0, 0], &q, &k, &v, &p, &mask, probeskalen(8));
        assert!(gq.iter().all(|x| *x == 0), "{gq:?}");
        assert!(gk.iter().all(|z| z.iter().all(|x| *x == 0)));
        assert!(gv.iter().all(|z| z.iter().all(|x| *x == 0)));
    }

    /// Gegen die numerische Ableitung des echten `attention_int`, in der
    /// Richtung, die am leichtesten falsch wird: `dL/dv`.
    #[test]
    fn attention_backward_trifft_die_numerische_ableitung_nach_v() {
        let frac = 8u8;
        let lut: Vec<i16> = (0..129)
            .map(|i| ((-(i as f64) / 256.0).exp() * 256.0).round() as i16)
            .collect();
        let q = vec![vec![64i16, 32]];
        let k = vec![vec![64i16, 0], vec![0i16, 64]];
        let v = vec![vec![1000i16, -400], vec![-600i16, 900]];
        let mask = vec![vec![true, true]];
        let c: Vec<Grad> = vec![3, -2];

        let verlust = |vv: &Vec<Vec<i16>>| -> f64 {
            let out = crate::attention::attention_int(
                &q, &k, vv, &mask, 1 << 15, 15, &lut, 0, frac);
            out[0].iter().zip(c.iter()).map(|(a, b)| *a as f64 * *b as f64).sum()
        };

        // p wie im Vorwärtspfad, damit derselbe Punkt abgeleitet wird.
        let scores: Vec<i32> = (0..2)
            .map(|j| {
                let s = crate::attention::dot_int(&q[0], &k[j]) * (1 << 15);
                crate::fixed_point::rshift_round_i64(s, 15) as i32
            })
            .collect();
        let p = crate::softmax::softmax_int(&scores, &lut, 0, frac);
        let (_, _, gv) = attention_backward(
            &c, &q[0], &k, &v, &p, &mask[0], probeskalen(frac));

        for j in 0..2 {
            for d in 0..2 {
                let mut plus = v.clone();
                let mut minus = v.clone();
                plus[j][d] += 64;
                minus[j][d] -= 64;
                let num = (verlust(&plus) - verlust(&minus)) / 128.0;
                let ana = gv[j][d] as f64;
                let abw = (num - ana).abs();
                let bezug = num.abs().max(ana.abs()).max(1.0);
                assert!(
                    abw <= 2.0 || abw / bezug < 0.30,
                    "v[{j}][{d}]: numerisch {num:.3}, analytisch {ana:.3}"
                );
            }
        }
    }

    /// ⚑ **Gegen die numerische Ableitung nach `q` und `k`, mit
    /// absichtlich paarweise verschiedenen Skalen.**
    ///
    /// ⛑ **Der Test darüber kann diesen Fehler nicht sehen.** Er leitet
    /// nach `v` ab, und `v` trägt dieselbe Skala wie die Ausgabe; für
    /// gleiche Skalen fallen die beiden möglichen Lesarten eines
    /// Gradienten (nach dem Wert oder nach der Darstellung) zusammen.
    /// Hier sind `q_frac`, `k_frac`, `v_frac`, `score_frac` und
    /// `prob_frac` verschieden, und mit den Schiebeweiten des
    /// **Vorwärts**passes lag `dL/dq` um den Faktor 4 daneben und
    /// `dL/dk` um den Faktor 8.
    ///
    /// Gemessen wird gegen `2^B · dL/dZ` nach dem **realen** Wert, denn
    /// das ist der Vertrag: `B` ist der Gradientenbus, hier `v_frac`.
    #[test]
    fn der_gradient_nach_q_und_k_trifft_die_numerische_ableitung() {
        const Q_FRAC: u8 = 8;
        const K_FRAC: u8 = 7;
        const V_FRAC: u8 = 10;
        const SCORE_FRAC: u8 = 12;
        const PROB_FRAC: u8 = 14;
        const EXP_IN: u8 = 8;
        const HEAD_DIM: usize = 4;

        // exp(-x) auf `SCORE_FRAC` Bruchstellen, Eingangsraster 2^-EXP_IN.
        let lut: Vec<i16> = (0..4096)
            .map(|i| {
                let x = i as f64 / (1u32 << EXP_IN) as f64;
                ((-x).exp() * (1u32 << SCORE_FRAC) as f64).round() as i16
            })
            .collect();
        let q: Vec<i16> = vec![300, -180, 90, 240];
        let k: Vec<Vec<i16>> = vec![
            vec![120, 60, -40, 200],
            vec![-90, 150, 220, 30],
            vec![200, -20, 70, -110],
        ];
        let v: Vec<Vec<i16>> = vec![
            vec![1000, -400, 250, 700],
            vec![-600, 900, -120, 340],
            vec![50, 80, -900, 120],
        ];
        let mask = vec![true, true, true];
        let c: Vec<i64> = vec![3, -2, 5, 1];

        let score_mult = crate::fixed_point::inv_sqrt_q15(HEAD_DIM);
        let score_shift =
            (Q_FRAC as u16 + K_FRAC as u16 + 15).saturating_sub(SCORE_FRAC as u16) as u8;
        let lut_shift = SCORE_FRAC - EXP_IN;

        let vorwaerts = |q: &[i16], k: &[Vec<i16>], v: &[Vec<i16>]| {
            let mut spur: Vec<Vec<i32>> = Vec::new();
            let q_zeile = [q.to_vec()];
            let out = crate::attention::attention_int_mit_spur(
                &q_zeile, k, v, std::slice::from_ref(&mask), score_mult, score_shift,
                &lut, lut_shift, PROB_FRAC, Some(&mut spur),
            );
            (out[0].clone(), spur[0].clone())
        };
        // L = Σ_d c_d · out_real[d]
        let verlust = |q: &[i16], k: &[Vec<i16>], v: &[Vec<i16>]| -> f64 {
            let (out, _) = vorwaerts(q, k, v);
            out.iter().zip(c.iter()).map(|(o, ci)| *o as f64 * *ci as f64).sum::<f64>()
                / (1u64 << V_FRAC) as f64
        };

        let (_, p) = vorwaerts(&q, &k, &v);
        // Der Bus: `dL/dout_real` mal 2^B mit B = V_FRAC.
        let b = V_FRAC;
        let g: Vec<Grad> = c.iter().map(|ci| (*ci << b) as Grad).collect();
        let (gq, gk, _) = attention_backward(
            &g, &q, &k, &v, &p, &mask,
            Aufmerksamkeitsskalen {
                score_mult,
                score_mult_frac: 15,
                q_frac: Q_FRAC,
                k_frac: K_FRAC,
                v_frac: V_FRAC,
                prob_frac: PROB_FRAC,
            },
        );

        let h = 64i16;
        let pruefe = |name: String, num: f64, ana: f64| {
            let erwartet = num * (1u64 << b) as f64;
            let abweichung = (erwartet - ana).abs();
            let bezug = erwartet.abs().max(ana.abs()).max(1.0);
            assert!(
                abweichung <= 2.0 || abweichung / bezug < 0.30,
                "{name}: numerisch {erwartet:.1}, analytisch {ana:.1}"
            );
        };

        for d in 0..HEAD_DIM {
            let mut plus = q.clone();
            let mut minus = q.clone();
            plus[d] += h;
            minus[d] -= h;
            let num = (verlust(&plus, &k, &v) - verlust(&minus, &k, &v))
                / (2.0 * h as f64 / (1u64 << Q_FRAC) as f64);
            pruefe(format!("q[{d}]"), num, gq[d] as f64);
        }
        for j in 0..k.len() {
            for d in 0..HEAD_DIM {
                let mut plus = k.clone();
                let mut minus = k.clone();
                plus[j][d] += h;
                minus[j][d] -= h;
                let num = (verlust(&q, &plus, &v) - verlust(&q, &minus, &v))
                    / (2.0 * h as f64 / (1u64 << K_FRAC) as f64);
                pruefe(format!("k[{j}][{d}]"), num, gk[j][d] as f64);
            }
        }
        // ⛑ Ohne diese Zeile bestünde der Test auch, wenn beide Seiten
        // überall null wären.
        assert!(gq.iter().any(|x| *x != 0), "gq ist überall null, der Test misst nichts");
    }

    // ---- Der Verlust ---------------------------------------------------

    /// ⚑ **Der Gradient der Kreuzentropie summiert sich zu null.**
    ///
    /// `Σ p = 1` und `Σ onehot = 1`, also ist `Σ (p − onehot) = 0`. Das
    /// ist keine Zierde, sondern die Aussage, dass eine gemeinsame
    /// Verschiebung aller Logits den Verlust nicht aendert: **Der
    /// Softmax ist gegen sie invariant**, und ein Gradient, der das
    /// nicht abbildet, schoebe alle Logits gemeinsam nach oben oder
    /// unten.
    #[test]
    fn der_kreuzentropie_gradient_summiert_sich_zu_null() {
        let frac = 12u8;
        let lut: Vec<i16> = (0..128)
            .map(|i| ((-(i as f64) / 256.0).exp() * 256.0).round() as i16)
            .collect();
        let logits: Vec<i32> = vec![900, -400, 1500, 200, 60];
        let p = crate::softmax::softmax_int(&logits, &lut, 8, frac);
        assert_eq!(p.iter().map(|v| i64::from(*v)).sum::<i64>(), 1 << frac);

        let g = kreuzentropie_gradient(&p, 2, frac);
        let summe: i64 = g.iter().map(|v| i64::from(*v)).sum();
        assert_eq!(summe, 0, "der Gradient summiert sich zu {summe} statt zu null");
    }

    /// ⚑ **Das Zielwort zieht nach unten, alle anderen nach oben.**
    ///
    /// Der Gradient zeigt in die Richtung **steigenden** Verlusts; ein
    /// Schritt geht ihm entgegen. Das Zielwort bekommt deshalb einen
    /// negativen Eintrag, damit sein Logit steigt.
    ///
    /// ⛑ Ein vertauschtes Vorzeichen faellt an keiner Summe auf und
    /// traegt einen Lauf, der zuverlaessig das **falsche** Wort lernt.
    #[test]
    fn das_zielwort_bekommt_das_andere_vorzeichen() {
        let frac = 12u8;
        // ⛑ Alle vier gleich, damit keiner null ist: Ein Wort mit
        // Wahrscheinlichkeit null bekaeme den Gradienten null, und der
        // Test prueft dann eine Ungleichung, die gar nicht gilt.
        let p: Vec<Grad> = vec![1 << 10; 4];
        assert_eq!(p.iter().map(|v| i64::from(*v)).sum::<i64>(), 1 << frac);
        let g = kreuzentropie_gradient(&p, 1, frac);
        assert!(g[1] < 0, "das Zielwort bekam {} statt eines negativen Eintrags", g[1]);
        for (i, v) in g.iter().enumerate() {
            if i != 1 {
                assert!(*v > 0, "Wort {i} bekam {v} statt eines positiven Eintrags");
            }
        }
    }

    /// Ein Zielwort ausserhalb des Vokabulars ist ein Aufruferfehler.
    #[test]
    #[should_panic(expected = "ausserhalb des Vokabulars")]
    fn ein_zielwort_ausserhalb_des_vokabulars_bricht_ab() {
        let _ = kreuzentropie_gradient(&[1, 2, 3], 3, 8);
    }

    // ---- Embedding ----------------------------------------------------

    /// **Akkumulierend, nicht setzend.** Kommt ein Token zweimal vor,
    /// muss sich sein Gradient addieren. Wer zuweist, behält das letzte
    /// Vorkommen; das fällt bei seltenen Token nie auf und bei häufigen
    /// als langsames Lernen.
    #[test]
    fn ein_token_zweimal_addiert_sich() {
        let vocab = 5usize;
        let hidden = 3usize;
        let mut ziel = vec![0i64; vocab * hidden];
        embedding_backward_akkumulieren(&mut ziel, vocab, 2, &[10, -20, 30]);
        embedding_backward_akkumulieren(&mut ziel, vocab, 2, &[1, 2, 3]);
        assert_eq!(&ziel[6..9], &[11, -18, 33]);
        // Alle anderen Zeilen bleiben unberührt.
        assert!(ziel[..6].iter().all(|v| *v == 0));
        assert!(ziel[9..].iter().all(|v| *v == 0));
    }


    // ---- SiLU ---------------------------------------------------------

    /// Baut die Ableitungs-LUT wie `calibrate/src/luts.py`.
    ///
    /// Bewusst hier nachgebaut und nicht aus einer Datei gelesen: Der
    /// Test soll den Kernel prüfen und nicht die Verfügbarkeit eines
    /// Artefakts. Weicht die Erzeugung eines Tages ab, fällt das im
    /// Konformitätslauf auf, nicht hier, und das ist die richtige
    /// Arbeitsteilung.
    fn silu_grad_lut(min: i32, max: i32, in_frac: u8, out_frac: u8) -> Vec<i16> {
        let in_scale = (1 << in_frac) as f64;
        let out_scale = (1 << out_frac) as f64;
        (min..=max)
            .map(|x| {
                let xf = x as f64 / in_scale;
                let s = 1.0 / (1.0 + (-xf).exp());
                (s * (1.0 + xf * (1.0 - s)) * out_scale).round() as i16
            })
            .collect()
    }

    /// ⚑ **Der gebaute Vorrat trifft die analytische Ableitung.**
    ///
    /// Gebaut wird er ganzzahlig aus der Vorwaertstabelle; hier steht
    /// die analytische Rechnung als **unabhaengige** Gegenprobe. Sie
    /// darf Gleitkomma benutzen, weil sie nur prueft und nie laeuft.
    #[test]
    fn der_gradientenvorrat_trifft_die_analytische_ableitung() {
        let (in_frac, out_frac) = (6u8, 12u8);
        let (min, max) = (-256i32, 256i32);
        let vorwaerts: Vec<i16> = (min..=max)
            .map(|x| {
                let xf = x as f64 / (1 << in_frac) as f64;
                let s = 1.0 / (1.0 + (-xf).exp());
                (xf * s * (1 << out_frac) as f64).round() as i16
            })
            .collect();
        let gebaut = silu_grad_aus_lut(&vorwaerts);
        let f = silu_grad_frac(in_frac, out_frac);
        assert_eq!(gebaut.len(), vorwaerts.len());

        // ⚑ **Die Raender bleiben draussen**: Dort steht die einseitige
        // Differenz, und die ist von Natur aus ungenauer.
        let mut groesster = 0.0f64;
        for (i, x) in (min..=max).enumerate() {
            if i == 0 || i + 1 == gebaut.len() {
                continue;
            }
            let xf = x as f64 / (1 << in_frac) as f64;
            let sg = 1.0 / (1.0 + (-xf).exp());
            let wahr = sg * (1.0 + xf * (1.0 - sg));
            let unser = gebaut[i] as f64 / (1i64 << f) as f64;
            groesster = groesster.max((wahr - unser).abs());
        }
        assert!(
            groesster < 0.02,
            "der gebaute Vorrat weicht um {groesster} von der Ableitung ab"
        );
    }

    /// Aus einem Punkt folgt keine Steigung.
    #[test]
    #[should_panic(expected = "keine Steigung")]
    fn ein_einzelner_punkt_hat_keine_ableitung() {
        let _ = silu_grad_aus_lut(&[7]);
    }

    /// Gegen die numerische Ableitung der **Vorwärts-LUT**, also gegen
    /// das, was `mlp_int` tatsächlich rechnet.
    #[test]
    fn silu_backward_trifft_die_ableitung_der_vorwaerts_lut() {
        let (in_frac, out_frac) = (6u8, 12u8);
        let (min, max) = (-256i32, 256i32);
        let offset = -min as i16;
        let grad_lut = silu_grad_lut(min, max, in_frac, out_frac);

        let vor: Vec<i16> = (min..=max)
            .map(|x| {
                let xf = x as f64 / (1 << in_frac) as f64;
                (xf / (1.0 + (-xf).exp()) * (1 << out_frac) as f64).round() as i16
            })
            .collect();

        for x in [-128i16, -64, 0, 64, 128, 154] {
            let idx = (x as i32 - min) as usize;
            let num = (vor[idx + 1] as f64 - vor[idx - 1] as f64) / 2.0
                * (1 << in_frac) as f64
                / (1 << out_frac) as f64;

            // Ein Gradient von 1 auf der Ausgangsskala: dann ist das
            // Ergebnis die Ableitung selbst.
            let g: Vec<Grad> = vec![1 << out_frac];
            let out = silu_backward(
                &g, &[x], &grad_lut, in_frac, in_frac, offset, out_frac,
                out_frac, out_frac,
            );
            let ana = out[0] as f64 / (1 << out_frac) as f64;
            assert!(
                (num - ana).abs() < 0.02,
                "x = {}: numerisch {num:.5}, Kernel {ana:.5}",
                x as f64 / (1 << in_frac) as f64
            );
        }
    }

    /// **Die Ableitung überschwingt über eins.** silu' erreicht rund
    /// 1,10 bei x ≈ 2,36 und fällt links auf etwa −0,10. Wer den
    /// Ausgangsbereich wie bei SiLU selbst wählt, sättigt genau dort,
    /// wo die Ableitung am größten ist. Der Test hält die Eigenschaft
    /// fest, damit sie bei einer Änderung der spec auffällt.
    #[test]
    fn die_silu_ableitung_ueberschwingt_und_wird_negativ() {
        let lut = silu_grad_lut(-256, 256, 6, 12);
        let max = *lut.iter().max().unwrap() as f64 / 4096.0;
        let min = *lut.iter().min().unwrap() as f64 / 4096.0;
        assert!(max > 1.05 && max < 1.15, "Maximum {max:.4}, erwartet ~1,10");
        assert!(min < -0.05 && min > -0.15, "Minimum {min:.4}, erwartet ~-0,10");
    }

    /// Ohne eingehenden Gradienten kein ausgehender.
    #[test]
    fn silu_ohne_gradient_bleibt_still() {
        let lut = silu_grad_lut(-256, 256, 6, 12);
        let out = silu_backward(&[0, 0, 0], &[10, -20, 30], &lut, 6, 6, 256, 12, 12, 12);
        assert!(out.iter().all(|v| *v == 0), "{out:?}");
    }

}
