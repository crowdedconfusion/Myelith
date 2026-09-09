//! Der Trainingsschritt als Ganzes: vorwärts, rückwärts, fortschreiben.
//!
//! # ⚑ Warum es dieses Modul gibt
//!
//! `backward` liefert Gradienten, `optimierer` schreibt Gewichte fort,
//! und beide sind gegen Golden-Vektoren geprüft. **Bis zum 2026-09-01
//! hatte keiner der beiden einen Aufrufer**: `linear_backward`,
//! `moe_backward`, `attention_backward`, `rmsnorm_backward`,
//! `silu_backward` und `optimierer::schritt` wurden von nichts außerhalb
//! ihrer eigenen Tests gerufen.
//!
//! Die Verdrahtung zur Schleife stand als offener Punkt fest, war also
//! bekannt und kein Fund. **Aber es ist dieselbe
//! Lage**, und sie hat dieselbe Folge: Einzeln geprüfte Teile sagen
//! nichts über ihr Zusammenspiel. ⚑ **Wo Ganzzahltraining bricht, ist
//! nicht der einzelne Kern, sondern die Skala zwischen zweien.**
//!
//! # ⚑ Die Brücke, die fehlte: Master zu Übertragungsgewicht
//!
//! Der Optimierer rechnet auf `Master` (i32, mit [`FEIN_BITS`]
//! Bruchstellen darunter), der Vorwärtspass will `i8` mit einer Skala je
//! Zeile. **Diese Umrechnung gab es nirgends**, und ohne sie ist der
//! Kreis nicht zu schließen: Man kann fortschreiben oder rechnen, nicht
//! beides.
//!
//! # Was hier noch nicht steht
//!
//! Ein **Modell**. Dieses Modul schließt den Kreis für eine lineare
//! Ebene; die Schleife über ein ganzes Netz braucht einen Vorwärtspass,
//! der seine Zwischenwerte behält, und der Vorwärtspass der Laufzeit ist
//! auf Inferenz zugeschnitten und behält nichts. Das ist eigene Arbeit
//! und ein eigener offener Punkt.

use crate::attention::attention_int_mit_spur;
use crate::backward::{
    attention_backward, linear_backward, rope_backward, silu_backward, silu_grad_frac,
    Aufmerksamkeitsskalen, Grad,
};
use crate::fixed_point::{clamp_i16, clamp_i16_from_i64 as clamp_i16_von_i64, inv_sqrt_q15, rescale, rescale_i64};
use crate::integer_math::lut_lookup;
use crate::linear::{add_bias_i16, linear_w8a16};
use crate::mlp::{mlp_int_mit_spur, Mlpspur};
use crate::optimierer::{schritt, Master, Schrittkennung};
use crate::rope::rotate_half_split_i16;

/// Wandelt Mastergewichte in die Übertragungsform: `i8` je Wert und eine
/// Verschiebung je Ausgabezeile.
///
/// # ⚑ Je Zeile eine Skala, nicht eine für alles
///
/// `linear_w8a16` erwartet `w_shifts` mit einem Eintrag je Ausgabezeile,
/// und das ist kein Zufall: Zwei Zeilen eines Gewichts können sich um
/// Größenordnungen unterscheiden. Eine gemeinsame Skala richtete sich
/// nach der größten und ließe die kleine auf null zusammenfallen.
///
/// # ⚑ Der Nullfall ist eine eigene Aussage
///
/// Eine Zeile, die nur Nullen enthält, hat keine sinnvolle Skala. Sie
/// bekommt Verschiebung `0`, und das ist richtig: Aus Nullen wird bei
/// jeder Verschiebung wieder null, und eine erfundene Skala wäre eine
/// Zahl ohne Deckung.
///
/// # ⚑ Die Skala des Masters ist ein Argument, kein Zufall (Fund 174)
///
/// Der reale Wert eines Gewichts ist `W / 2^shift`. Der Master trägt
/// `master_frac` Bruchstellen, also ist `W_real = master / 2^master_frac`,
/// **unabhängig davon, wie viele Stellen zum Hineinpassen in `i8`
/// wegfallen**. Genau deshalb steht in `shifts` die **Differenz**
/// `master_frac − s` und nicht `s`.
///
/// ⛑ **Bis zum 2026-09-04 stand dort `s`**, und damit war der reale Wert
/// `master / 2^(2·s)`. Solange `s` sich nicht änderte, war das eine
/// Proportionalität und alles stimmte. **Änderte `s` sich, sprang der
/// Wert der ganzen Zeile:**
///
/// | Betragsmaximum | `s` | grösstes `i8` | realer Wert, alt |
/// |---|---|---|---|
/// | 127 | 0 | 127 | 127,00 |
/// | **128** | **1** | **64** | **32,00** |
/// | 254 | 1 | 127 | 63,50 |
/// | **256** | **2** | **64** | **16,00** |
///
/// Ein Master, der um **eins** wächst, liess das Gewicht der ganzen
/// Zeile auf ein Viertel fallen, sobald ihr Betragsmaximum eine
/// Zweierpotenz überschritt. Die Abbildung war über die Oktavgrenze
/// hinweg nicht einmal monoton. **In einem Trainingslauf über tausende
/// Schritte passiert das zwangsläufig**, und es sähe aus wie „das Modell
/// wird plötzlich schlechter".
///
/// # Panics
///
/// Wenn eine Zeile mehr Stellen braucht, als der Master trägt. Das
/// heisst, ihr realer Wert überschreitet 127, und die Übertragungsform
/// kann ihn nicht ausdrücken. **Panik statt stiller Sättigung:** Ein
/// stillschweigend gekappter Wert fiele erst an der Verlustkurve auf.
pub fn gewicht_aus_master(
    master: &[Master],
    in_features: usize,
    master_frac: u8,
) -> (Vec<i8>, Vec<u8>) {
    assert!(in_features > 0, "gewicht_aus_master: in_features muss > 0 sein");
    assert_eq!(
        master.len() % in_features,
        0,
        "gewicht_aus_master: {} Master passen nicht zu Zeilen à {}",
        master.len(),
        in_features
    );
    let zeilen = master.len() / in_features;
    let mut w = vec![0i8; master.len()];
    let mut shifts = vec![0u8; zeilen];

    for z in 0..zeilen {
        let bereich = &master[z * in_features..(z + 1) * in_features];
        let groesster = bereich.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
        if groesster == 0 {
            // ⚑ **Die Skala des Masters, nicht null.** Aus Nullen wird
            // bei jeder Verschiebung wieder null; die ehrliche Angabe
            // ist die, auf der der Master steht.
            shifts[z] = master_frac;
            continue;
        }
        // Wie weit muss nach rechts geschoben werden, damit der groesste
        // Betrag in i8 passt? `127` ist der groesste darstellbare Betrag.
        let mut s = 0u32;
        while (groesster >> s) > 127 {
            s += 1;
        }
        assert!(
            s <= u32::from(master_frac),
            "gewicht_aus_master: Zeile {z} hat den Betrag {groesster} und braucht {s} \
             Stellen, aber der Master traegt nur {master_frac}. Der reale Wert ueberschreitet \
             127; die Uebertragungsform kann ihn nicht ausdruecken"
        );
        // ⚑ **Hier steht die Differenz und nicht `s` selbst (Fund 174).**
        // `s` sagt, wie viele Stellen zum Hineinpassen wegfallen; die
        // Ausgabe braucht, **wie viele Bruchstellen uebrig bleiben**.
        shifts[z] = master_frac - u8::try_from(s).unwrap_or(u8::MAX);
        for (i, v) in bereich.iter().enumerate() {
            let gerundet = crate::fixed_point::rshift_round_i64(
                i64::from(*v),
                u8::try_from(s).unwrap_or(u8::MAX),
            );
            w[z * in_features + i] = gerundet.clamp(-127, 127) as i8;
        }
    }
    (w, shifts)
}

/// Ein vollständiger Schritt auf **einer** linearen Ebene.
///
/// Vorwärts mit dem aus dem Master gewonnenen Gewicht, Verlustgradient
/// gegen das Ziel, rückwärts, fortschreiben. Gibt den quadratischen
/// Abstand **vor** dem Schritt zurück, damit ein Aufrufer sieht, wohin
/// es geht.
///
/// ⚑ **Der Verlustgradient ist hier `2·(y − ziel)`**, also der des
/// quadratischen Abstands. Das ist keine Trainingsvorschrift, sondern
/// die einfachste, an der sich zeigen lässt, **dass der Kreis
/// geschlossen ist**: Wenn er stimmt, sinkt der Abstand.
/// Die Zahlen, mit denen ein Schritt gerechnet wird.
///
/// ⚑ **Zusammengefasst, weil neun Argumente niemand richtig übergibt.**
/// Bei einer Reihe gleichartiger Zahlen fällt eine Vertauschung nicht
/// auf: `act_frac_bits` und `out_frac_bits` sind beide `u8`, Zähler und
/// Nenner beide `i64`. In einem Typ tragen sie ihren Namen mit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schrittvorgaben {
    /// Spaltenzahl des Gewichts.
    pub in_features: usize,
    /// Bruchstellen der Eingabeaktivierungen.
    pub act_frac_bits: u8,
    /// Bruchstellen der Ausgabe.
    pub out_frac_bits: u8,
    /// Bruchstellen des Masters.
    ///
    /// ⚑ **Die Skala, auf der die Mastergewichte stehen** (Fund 174).
    /// Sie ist eine Festlegung des Aufrufers und keine Folge der Zahlen:
    /// Aus ihr und der Zeilenverschiebung folgt der reale Wert, und
    /// solange sie feststeht, bleibt er stetig, wenn die Gewichte
    /// wachsen.
    pub master_frac: u8,
    /// Zähler der Lernrate.
    pub lr_zaehler: i64,
    /// Nenner der Lernrate. **Muss größer als null sein.**
    pub lr_nenner: i64,
    /// Ebene, Schritt und Versatz für den Würfel.
    pub kennung: Schrittkennung,
}

pub fn schritt_auf_linear(
    master: &mut [Master],
    x: &[i16],
    ziel: &[i16],
    v: Schrittvorgaben,
) -> i64 {
    let Schrittvorgaben {
        in_features,
        act_frac_bits,
        out_frac_bits,
        master_frac,
        lr_zaehler,
        lr_nenner,
        kennung,
    } = v;
    let (w, shifts) = gewicht_aus_master(master, in_features, master_frac);
    let y = linear_w8a16(x, &w, in_features, &shifts, act_frac_bits, out_frac_bits);
    assert_eq!(y.len(), ziel.len(), "schritt_auf_linear: Ziel passt nicht zur Ausgabe");

    let mut abstand = 0i64;
    let mut g: Vec<Grad> = Vec::with_capacity(y.len());
    for (a, b) in y.iter().zip(ziel) {
        let d = i64::from(*a) - i64::from(*b);
        abstand += d * d;
        // Faktor zwei des quadratischen Abstands; die Lernrate traegt
        // ihn ohnehin mit, aber er gehoert in den Gradienten und nicht
        // in eine stillschweigend halbierte Rate.
        g.push((2 * d).clamp(i64::from(Grad::MIN), i64::from(Grad::MAX)) as Grad);
    }

    let (_gx, gw) = linear_backward(
        &g,
        x,
        &w,
        in_features,
        &shifts,
        out_frac_bits,
        act_frac_bits,
    );
    // ⚑ `linear_backward` liefert `dL/dW` als `i64` je Gewicht. Der
    // Optimierer nimmt `Grad = i32`; gesaettigt statt umlaufend, denn
    // ein umgelaufener Gradient zeigt in die **Gegenrichtung**.
    let gw32: Vec<Grad> = gw
        .iter()
        .map(|v| (*v).clamp(i64::from(Grad::MIN), i64::from(Grad::MAX)) as Grad)
        .collect();
    schritt(master, &gw32, kennung, lr_zaehler, lr_nenner);
    abstand
}

/// Die Zahlen, mit denen ein MLP-Schritt gerechnet wird.
///
/// ⚑ **Zusammengefasst, aus demselben Grund wie [`Schrittvorgaben`]:**
/// Bei einer Reihe gleichartiger `u8` faellt eine Vertauschung nicht
/// auf, und hier sind es sieben.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mlpvorgaben {
    /// Breite des Eingangs.
    pub hidden_size: usize,
    /// Breite der inneren Ebene.
    pub intermediate_size: usize,
    /// Bruchstellen des Eingangs.
    pub act_frac: u8,
    /// Bruchstellen der Gate-Projektion.
    pub gate_frac: u8,
    /// Bruchstellen der Up-Projektion.
    pub up_frac: u8,
    /// Bruchstellen des Produkts, also des Eingangs von `down_proj`.
    pub down_in_frac: u8,
    /// Bruchstellen der Ausgabe.
    pub aus_frac: u8,
    /// Bruchstellen des Masters.
    ///
    /// ⚑ **Die Skala, auf der die Mastergewichte stehen** (Fund 174).
    /// Sie ist eine Festlegung des Aufrufers und keine Folge der Zahlen:
    /// Aus ihr und der Zeilenverschiebung folgt der reale Wert, und
    /// solange sie feststeht, bleibt er stetig, wenn die Gewichte
    /// wachsen.
    pub master_frac: u8,
    /// Eingangsdomaene der Silu-Tabelle.
    pub silu_in_frac: u8,
    /// Nullpunktverschiebung der Silu-Tabelle.
    pub silu_lut_offset: i16,
    /// Bruchstellen der Silu-Ausgabe.
    pub silu_out_frac: u8,
    /// Zaehler der Lernrate.
    pub lr_zaehler: i64,
    /// Nenner der Lernrate.
    pub lr_nenner: i64,
    /// Ebene, Schritt und Indexversatz fuer das stochastische Runden.
    pub kennung: Schrittkennung,
}

/// Ein vollstaendiger Schritt auf einem **MLP-Block**: Gate, Up, Silu,
/// Produkt, Down.
///
/// # ⚑ Warum der Block und nicht die drei Projektionen einzeln
///
/// [`schritt_auf_linear`] schliesst den Kreis fuer **eine** lineare
/// Ebene. Was dort nicht vorkommt, ist die Stelle, an der es in einem
/// Netz bricht: **die Skala zwischen zwei Kernen.** Der MLP-Block hat
/// davon vier hintereinander (Gate nach Silu, Silu mal Up, Produkt nach
/// Down, Down in die Ausgabe), und jede einzelne ist fuer sich richtig,
/// waehrend die Kette in die falsche Richtung laufen kann.
///
/// # ⚑ Ein Gradientenbus, und er liegt auf `down_in_frac`
///
/// Zwischen den Kernen wird **jeder** Gradient auf dieselben
/// Bruchstellen gebracht. Die Alternative waere, jede Stufe auf ihrer
/// natuerlichen Skala zu lassen; dann traegt jede Uebergabe eine eigene
/// Umrechnung, und **die Fehler stecken genau in diesen Umrechnungen.**
/// Eine gemeinsame Skala macht sie zu einer einzigen Entscheidung.
///
/// # ⚑ Die Produktregel ist die Stelle, die sich nicht ansehen laesst
///
/// `h = silu(gate) · up` heisst rueckwaerts: Der Gradient nach `up` ist
/// `g_h · silu(gate)`, der nach der Aktivierung `g_h · up`. **Beide
/// brauchen den jeweils anderen Faktor**, und beide muessen aus
/// **demselben** Durchlauf stammen. Deshalb nimmt diese Funktion die
/// Spur und rechnet nicht neu.
///
/// Gibt den quadratischen Abstand **vor** dem Schritt zurueck.
#[allow(clippy::too_many_arguments)]
pub fn schritt_auf_mlp(
    gate_master: &mut [Master],
    up_master: &mut [Master],
    down_master: &mut [Master],
    x: &[i16],
    ziel: &[i16],
    silu_lut: &[i16],
    grad_lut: &[i16],
    v: Mlpvorgaben,
) -> i64 {
    let (abstand, gr) =
        gradienten_des_mlp(gate_master, up_master, down_master, x, ziel, silu_lut, grad_lut, v);

    // ⚑ **Drei verschiedene Indexversaetze**, sonst bekaemen drei
    // Gewichte an derselben Stelle denselben Wuerfel, und das
    // stochastische Runden waere zwischen ihnen korreliert.
    let k = v.kennung;
    schritt(gate_master, &gr.gate, k, v.lr_zaehler, v.lr_nenner);
    schritt(
        up_master,
        &gr.up,
        Schrittkennung { index_versatz: k.index_versatz + gate_master.len() as u64, ..k },
        v.lr_zaehler,
        v.lr_nenner,
    );
    schritt(
        down_master,
        &gr.down,
        Schrittkennung {
            index_versatz: k.index_versatz + (gate_master.len() + up_master.len()) as u64,
            ..k
        },
        v.lr_zaehler,
        v.lr_nenner,
    );
    abstand
}

/// Die Gradienten eines MLP-Blocks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mlpgradienten {
    /// `dL/dW` der Gate-Projektion, in Zeilen zu `hidden_size`.
    pub gate: Vec<Grad>,
    /// `dL/dW` der Up-Projektion.
    pub up: Vec<Grad>,
    /// `dL/dW` der Down-Projektion, in Zeilen zu `intermediate_size`.
    pub down: Vec<Grad>,
    /// `dL/dx` nach dem **Eingang** des Blocks, auf `act_frac`.
    ///
    /// # ⚑ Warum er hier steht, obwohl der Blockschritt ihn nicht braucht
    ///
    /// Ein Block für sich hängt an einem Ziel, und dann endet die Kette
    /// bei ihm. **In einer Ebene endet sie nicht:** Der Eingang des
    /// MLP-Blocks ist die Ausgabe einer Normierung, und deren Eingang
    /// ist der Residualstrom. Wer den Gradienten hier verwirft, kann die
    /// Ebene nicht zusammensetzen.
    ///
    /// ⚑ **Er ist die Summe über beide Eingangsprojektionen.** Gate und
    /// Up lesen **denselben** `x`; wer nur einen von beiden nimmt,
    /// halbiert den Gradienten und merkt es nicht, weil die Richtung
    /// stimmt.
    pub eingang: Vec<Grad>,
}

/// Rechnet Abstand und Gradienten, **ohne** fortzuschreiben.
///
/// # ⚑ Warum getrennt vom Schritt
///
/// **Ein Test, der nur den Abstand sieht, kann den Gradienten nicht
/// pruefen.** „Der Abstand sinkt" belegt, dass die Kette bergab laeuft,
/// und das tut sie auch mit einer falschen, aber zufaellig korrelierten
/// Richtung: Zwei Gegenproben (die beiden Gradientenaeste vertauscht,
/// der Gradientenvorrat auf der falschen Skala) blieben deshalb
/// **gruen**.
///
/// Sichtbare Gradienten erlauben die Pruefung, die es wirklich
/// entscheidet: **ein Gewicht gegen seinen Gradienten schieben muss den
/// Abstand senken, mit ihm muss es ihn heben.**
#[allow(clippy::too_many_arguments)]
pub fn gradienten_des_mlp(
    gate_master: &[Master],
    up_master: &[Master],
    down_master: &[Master],
    x: &[i16],
    ziel: &[i16],
    silu_lut: &[i16],
    grad_lut: &[i16],
    v: Mlpvorgaben,
) -> (i64, Mlpgradienten) {
    let (wg, sg) = gewicht_aus_master(gate_master, v.hidden_size, v.master_frac);
    let (wu, su) = gewicht_aus_master(up_master, v.hidden_size, v.master_frac);
    let (wd, sd) = gewicht_aus_master(down_master, v.intermediate_size, v.master_frac);
    let aus_frac = vec![v.aus_frac; v.hidden_size];

    let mut spur = Mlpspur::default();
    let y = mlp_int_mit_spur(
        x, &wg, &wu, &wd, v.hidden_size, v.intermediate_size, &sg, &su, &sd, silu_lut,
        v.act_frac, v.gate_frac, v.up_frac, v.down_in_frac, v.silu_in_frac,
        v.silu_lut_offset, v.silu_out_frac, &aus_frac, Some(&mut spur),
    );
    assert_eq!(y.len(), ziel.len(), "schritt_auf_mlp: Ziel passt nicht zur Ausgabe");

    let mut abstand = 0i64;
    let mut g: Vec<Grad> = Vec::with_capacity(y.len());
    for (a, b) in y.iter().zip(ziel) {
        let d = i64::from(*a) - i64::from(*b);
        abstand += d * d;
        g.push((2 * d).clamp(i64::from(Grad::MIN), i64::from(Grad::MAX)) as Grad);
    }
    (abstand, gradienten_des_mlp_aus_gradient(&g, x, &spur, &wg, &sg, &wu, &su, &wd, &sd, silu_lut, grad_lut, v))
}

/// Die Gradienten eines MLP-Blocks aus einem **eingehenden** Gradienten.
///
/// # ⚑ Warum es diese Fassung gibt
///
/// [`gradienten_des_mlp`] rechnet den Verlustgradienten selbst aus, aus
/// einem Ziel. **In einer Ebene gibt es kein Ziel je Block:** Der
/// MLP-Block bekommt seinen Gradienten von der Residualaddition über
/// ihm. Wer die Ebene aus der Zielfassung zusammensetzen wollte, müsste
/// sich ein Ziel ausdenken, das den gewünschten Gradienten erzeugt, und
/// das ist eine Rückrechnung, die nichts hinzufügt und alles verdecken
/// kann.
///
/// `g_aus` liegt auf dem Bus, also auf `aus_frac`, und `spur` stammt aus
/// **demselben** Vorwärtspass wie die Gewichte.
#[allow(clippy::too_many_arguments)]
pub fn gradienten_des_mlp_aus_gradient(
    g: &[Grad],
    x: &[i16],
    spur: &Mlpspur,
    wg: &[i8],
    sg: &[u8],
    wu: &[i8],
    su: &[u8],
    wd: &[i8],
    sd: &[u8],
    silu_lut: &[i16],
    grad_lut: &[i16],
    v: Mlpvorgaben,
) -> Mlpgradienten {
    let (wg, sg, wu, su, wd, sd) = (wg, sg, wu, su, wd, sd);

    // 1. Durch `down_proj`: Gradient nach dem Produkt.
    let (g_h, gw_down) = linear_backward(
        g, &spur.h, wd, v.intermediate_size, sd, v.aus_frac, v.down_in_frac,
    );

    // 2. Die Produktregel. ⚑ `silu(gate)` wird hier aus der Spur
    //    nachgeschlagen und nicht mitgefuehrt: Ein Tabellenzugriff ist
    //    billiger als ein weiterer Vektor, und er ist **derselbe**
    //    Zugriff wie im Vorwaertspass, also dieselbe Zahl.
    let mut g_aktiv: Vec<Grad> = Vec::with_capacity(v.intermediate_size);
    let mut g_up: Vec<Grad> = Vec::with_capacity(v.intermediate_size);
    for ((gh, gate_i), up_i) in g_h.iter().zip(spur.gate.iter()).zip(spur.up.iter()) {
        let dom = rescale(i32::from(*gate_i), v.gate_frac, v.silu_in_frac);
        let aktiv = i64::from(lut_lookup(clamp_i16(dom), silu_lut, 0, v.silu_lut_offset));
        let gh = i64::from(*gh);
        g_aktiv.push(begrenze(rescale_i64(
            gh * i64::from(*up_i),
            v.down_in_frac + v.up_frac,
            v.down_in_frac,
        )));
        g_up.push(begrenze(rescale_i64(
            gh * aktiv,
            v.down_in_frac + v.silu_out_frac,
            v.down_in_frac,
        )));
    }

    // 3. Durch Silu.
    let g_gate = silu_backward(
        &g_aktiv,
        &spur.gate,
        grad_lut,
        v.gate_frac,
        v.silu_in_frac,
        v.silu_lut_offset,
        silu_grad_frac(v.silu_in_frac, v.silu_out_frac),
        v.down_in_frac,
        v.down_in_frac,
    );

    // 4. Durch die beiden Eingangsprojektionen.
    let (gx_gate, gw_gate) =
        linear_backward(&g_gate, x, wg, v.hidden_size, sg, v.down_in_frac, v.act_frac);
    let (gx_up, gw_up) =
        linear_backward(&g_up, x, wu, v.hidden_size, su, v.down_in_frac, v.act_frac);
    // ⚑ **Summe und nicht einer von beiden.** Gate und Up lesen
    // denselben Eingang, also bekommt er beide Beitraege. In `i64`
    // addiert und **einmal** gesaettigt.
    let eingang: Vec<Grad> = gx_gate
        .iter()
        .zip(gx_up.iter())
        .map(|(a, b)| begrenze(i64::from(*a) + i64::from(*b)))
        .collect();

    Mlpgradienten {
        gate: nach_grad(&gw_gate),
        up: nach_grad(&gw_up),
        down: nach_grad(&gw_down),
        eingang,
    }
}

/// Die Vorspannungen der drei Eingangsprojektionen.
///
/// # ⚑ Ein Typ und nicht drei Argumentpaare
///
/// Q, K und V bekommen bei manchen Modellen eine Vorspannung und bei
/// anderen keine, aber nie eine halbe Auswahl davon. Als **ein**
/// `Option` gibt es entweder alle drei oder keine, und der Aufrufer
/// kann die drei Paare nicht gegeneinander vertauschen, ohne dass der
/// Feldname danebensteht.
///
/// ⚑ **Sie werden nicht fortgeschrieben.** Eine Vorspannung liegt als
/// `i16` mit einer Skala **je Element** vor, ein Gewicht als `i8` mit
/// einer Skala je Zeile; ein Master für die eine ist etwas anderes als
/// ein Master für das andere. Dieser Schritt hält sie fest, und das
/// steht hier, weil ein stillschweigend eingefrorener Parameter sonst
/// als „lernt nicht" auffiele.
#[derive(Debug, Clone, Copy)]
pub struct Vorspannungen<'a> {
    /// Werte der Q-Vorspannung, Länge `num_heads · head_dim`.
    pub q: &'a [i16],
    /// Skala je Element der Q-Vorspannung.
    pub q_skalen: &'a [u8],
    /// Werte der K-Vorspannung, Länge `num_kv_heads · head_dim`.
    pub k: &'a [i16],
    /// Skala je Element der K-Vorspannung.
    pub k_skalen: &'a [u8],
    /// Werte der V-Vorspannung.
    pub v: &'a [i16],
    /// Skala je Element der V-Vorspannung.
    pub v_skalen: &'a [u8],
}

/// Die Zahlen, mit denen ein Schritt auf dem Aufmerksamkeitsblock
/// gerechnet wird.
///
/// ⚑ **Zusammengefasst, aus demselben Grund wie [`Mlpvorgaben`]**, und
/// hier noch dringender: Es sind acht Skalen, alle `u8`, und
/// `q_frac`/`k_frac` gehen in **verschiedene** Schiebeweiten ein.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aufmerksamkeitsvorgaben {
    /// Breite des Residualstroms.
    pub hidden_size: usize,
    /// Zahl der Abfrageköpfe.
    pub num_heads: usize,
    /// Zahl der Schlüssel- und Wertköpfe. Bei gruppierter Aufmerksamkeit
    /// kleiner als [`Self::num_heads`].
    pub num_kv_heads: usize,
    /// Breite eines Kopfes.
    pub head_dim: usize,
    /// Bruchstellen des Eingangs, also des normierten Residualstroms.
    pub act_frac: u8,
    /// Bruchstellen der Q-Projektion.
    pub q_frac: u8,
    /// Bruchstellen der K-Projektion.
    pub k_frac: u8,
    /// Bruchstellen der V-Projektion.
    pub v_frac: u8,
    /// Bruchstellen des Eingangs der Ausgabeprojektion.
    pub attn_out_frac: u8,
    /// Bruchstellen der Blockausgabe.
    pub aus_frac: u8,
    /// Bruchstellen des Masters.
    ///
    /// ⚑ **Die Skala, auf der die Mastergewichte stehen** (Fund 174).
    /// Sie ist eine Festlegung des Aufrufers und keine Folge der Zahlen:
    /// Aus ihr und der Zeilenverschiebung folgt der reale Wert, und
    /// solange sie feststeht, bleibt er stetig, wenn die Gewichte
    /// wachsen.
    pub master_frac: u8,
    /// Bruchstellen der Punktzahlen vor dem Softmax.
    pub score_frac: u8,
    /// Bruchstellen der Wahrscheinlichkeiten.
    pub prob_frac: u8,
    /// Eingangsraster der exp-Tabelle.
    pub exp_input_frac: u8,
    /// Bruchstellen der Sinus- und Kosinustabelle.
    pub rope_frac: u8,
    /// Die Position der **ersten** übergebenen Stelle.
    ///
    /// ⚑ **Ein Abschnitt beginnt nicht bei null.** Wer über einen
    /// Korpus trainiert, schneidet Abschnitte heraus, und ihre
    /// Positionen sind die im Text und nicht die im Abschnitt. Bei
    /// Position null ist die Drehung die Einheit; ein Abschnitt, der
    /// immer dort begänne, liesse genau den Fall aus, in dem RoPE
    /// überhaupt etwas tut.
    pub positionsversatz: usize,
    /// Zähler der Lernrate.
    pub lr_zaehler: i64,
    /// Nenner der Lernrate.
    pub lr_nenner: i64,
    /// Ebene, Schritt und Indexversatz für das stochastische Runden.
    pub kennung: Schrittkennung,
}

/// Die QK-Normierung eines Blocks (Qwen3), falls das Modell sie hat.
///
/// # ⛑ Warum sie bis zum 2026-09-08 fehlte
///
/// Sie stand **nur im Vorwaertspfad** (`model.rs`). Der Trainingspfad
/// rechnete die Aufmerksamkeit ohne sie, und **nichts pruefte das**:
/// Ein Lauf auf einem Qwen3-Artefakt lief durch und trainierte gegen
/// eine andere Aufmerksamkeit als die Inferenz.
///
/// ⚑ **Die Gammas sind nicht trainierbar**, und das ist eine
/// Festlegung: Sie sind `head_dim` Zahlen je Ebene, und ein
/// zusaetzlicher Meister verschoebe die Indizes des stochastischen
/// Rundens. Zwei Knoten mit verschiedenen Fassungen rechneten dann
/// verschiedene Deltas und haetten beide recht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QkNormVorgaben<'a> {
    /// Gamma fuer Q, je Kopfdimension.
    pub q_gamma: &'a [i8],
    /// Die Zeilenskalen dazu.
    pub q_gamma_shifts: &'a [u8],
    /// Gamma fuer K.
    pub k_gamma: &'a [i8],
    /// Die Zeilenskalen dazu.
    pub k_gamma_shifts: &'a [u8],
    /// Die Ausgangsskala von Q nach der Normierung.
    ///
    /// ⚑ **Sie ersetzt `q_frac` fuer alles danach**, also fuer RoPE
    /// und fuer die Punktzahlskala. Wer das vergisst, rechnet die
    /// Aufmerksamkeit um Zweierpotenzen daneben.
    pub q_out_frac: u8,
    /// Dasselbe fuer K.
    pub k_out_frac: u8,
    /// Die Nachschlagetabelle der Kehrwurzel.
    pub rsqrt_lut: &'a [i16],
    /// Ihre Eingangsschiebung.
    pub rsqrt_input_shift: u8,
    /// Ihre Ausgangsskala.
    pub rsqrt_output_frac: u8,
}

/// Die Gradienten eines Aufmerksamkeitsblocks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Aufmerksamkeitsgradienten {
    /// `dL/dW` der Q-Projektion, in Zeilen zu `hidden_size`.
    pub q: Vec<Grad>,
    /// `dL/dW` der K-Projektion.
    pub k: Vec<Grad>,
    /// `dL/dW` der V-Projektion.
    pub v: Vec<Grad>,
    /// `dL/dW` der Ausgabeprojektion, in Zeilen zu `num_heads · head_dim`.
    pub o: Vec<Grad>,
    /// `dL/dx` nach dem **Eingang** des Blocks, **eine Zeile je
    /// Position**, auf `act_frac`.
    ///
    /// ⚑ **Je Position eine Zeile und nicht eine Summe.** Der Eingang
    /// des Blocks ist eine Folge; jede Position hat ihren eigenen
    /// Gradienten, und die Ebene braucht sie einzeln, weil dahinter eine
    /// Normierung je Position steht. **Summiert wird nur über die drei
    /// Projektionen**, denn Q, K und V lesen dieselbe Zeile.
    pub eingang: Vec<Vec<Grad>>,
}

/// Ein vollständiger Schritt auf einem **Aufmerksamkeitsblock**: Q, K,
/// V, RoPE, Softmax, Kopfgewichtung, Ausgabeprojektion.
///
/// # ⚑ Warum eine Folge und nicht eine Position
///
/// [`schritt_auf_mlp`] rechnet auf **einer** Position, und das ist dort
/// richtig: Der Feedforward-Zweig sieht jede Position einzeln. Der
/// Aufmerksamkeitsblock tut das nicht.
///
/// ⛑ **Auf einer einzelnen Position ist dieser Schritt eine
/// Nullmessung.** Bei einer einzigen Position gibt es genau einen
/// Schlüssel, der Softmax liefert exakt `1`, und seine Ableitung
/// `p · (g − ⟨g, p⟩)` ist damit exakt **null**: Q und K bekämen keinen
/// Gradienten, und ein Test, der den Abstand fallen sieht, hätte
/// ausschliesslich V und die Ausgabeprojektion geprüft. Die Folge ist
/// deshalb kein Komfort, sondern die Bedingung dafür, dass hier
/// überhaupt etwas geprüft wird.
///
/// # ⚑ Wo die Gradienten zusammenlaufen
///
/// Bei gruppierter Aufmerksamkeit lesen mehrere Abfrageköpfe denselben
/// Schlüsselkopf, und jede Abfrage liest **jede** frühere Position.
/// `dL/dk` und `dL/dv` einer Position sind deshalb **Summen** über alle
/// Abfrageköpfe der Gruppe und über alle späteren Abfragen; `dL/dq`
/// nicht, es hat genau einen Beitrag. Wer hier zuweist statt zu
/// addieren, verliert alle Beiträge ausser dem letzten, und das fällt
/// nur als langsames Lernen auf.
///
/// # ⚑ Der Gradientenbus liegt auf `attn_out_frac`
///
/// Dieselbe Wahl wie im MLP-Block, wo er auf `down_in_frac` liegt: die
/// Skala des Eingangs der letzten Matrix. Jeder Gradient zwischen zwei
/// Kernen trägt sie, und damit ist jede Umrechnung **eine** Entscheidung
/// statt einer je Übergabe.
///
/// **Was das kostet, und es gehört benannt:** `dL/dW` einer Matrix
/// entsteht als `Gradient · Eingang` und trägt deshalb den Faktor
/// `2^(Bus + Eingangsskala)`. Für Q, K und V ist das
/// `2^(attn_out_frac + act_frac)`, für die Ausgabeprojektion
/// `2^(aus_frac + attn_out_frac)`. Eine gemeinsame Lernrate wirkt auf
/// die vier Matrizen also um `2^(aus_frac − act_frac)` verschieden.
/// Der MLP-Block hat dieselbe Asymmetrie zwischen seinen drei Matrizen.
///
/// # Was hier nicht steht
///
/// Die **Normierung der Köpfe** vor RoPE, die manche Modelle haben, und
/// die **Vorspannungen als lernbare Grösse** (siehe
/// [`Vorspannungen`]). Beides ist eigene Arbeit.
///
/// Gibt den quadratischen Abstand **vor** dem Schritt zurück.
#[allow(clippy::too_many_arguments)]
pub fn schritt_auf_aufmerksamkeit(
    q_master: &mut [Master],
    k_master: &mut [Master],
    v_master: &mut [Master],
    o_master: &mut [Master],
    x: &[Vec<i16>],
    ziel: &[Vec<i16>],
    vorspannungen: Option<Vorspannungen<'_>>,
    cos_lut: &[i16],
    sin_lut: &[i16],
    exp_lut: &[i16],
    v: Aufmerksamkeitsvorgaben,
) -> i64 {
    let (abstand, gr) = gradienten_der_aufmerksamkeit(
        q_master, k_master, v_master, o_master, x, ziel, vorspannungen, cos_lut, sin_lut,
        exp_lut, v,
    );

    // ⚑ **Vier verschiedene Indexversaetze.** Ohne sie bekaemen vier
    // Gewichte an derselben Stelle denselben Wuerfel, und das
    // stochastische Runden waere zwischen ihnen korreliert.
    let k = v.kennung;
    let mut versatz = k.index_versatz;
    for (m, g) in [
        (&mut *q_master, &gr.q),
        (&mut *k_master, &gr.k),
        (&mut *v_master, &gr.v),
        (&mut *o_master, &gr.o),
    ] {
        let laenge = m.len() as u64;
        schritt(
            m,
            g,
            Schrittkennung { index_versatz: versatz, ..k },
            v.lr_zaehler,
            v.lr_nenner,
        );
        versatz += laenge;
    }
    abstand
}

/// Rechnet Abstand und Gradienten des Aufmerksamkeitsblocks, **ohne**
/// fortzuschreiben.
///
/// ⚑ **Getrennt vom Schritt, aus demselben Grund wie
/// [`gradienten_des_mlp`]:** „Der Abstand sinkt" belegt nur, dass die
/// Kette bergab läuft, und das tut sie auch mit einer falschen, aber
/// zufällig korrelierten Richtung. Sichtbare Gradienten erlauben den
/// Vergleich mit der numerischen Ableitung und den **Grössenvergleich
/// zwischen den Ästen**, und der ist hier die eigentliche Prüfung:
/// Q und K sind strukturell symmetrisch, ihre Gradienten müssen
/// derselben Grössenordnung angehören.
#[allow(clippy::too_many_arguments)]
/// Die Übertragungsform der vier Gewichtsmatrizen eines
/// Aufmerksamkeitsblocks.
///
/// ⚑ **Ein Typ und nicht acht Argumente.** Vier Paare aus `i8`-Werten
/// und Skalen sind acht gleichartige Scheiben; vertauscht man Q und K,
/// rechnet der Block weiter und liefert Zahlen.
#[derive(Debug, Clone, Copy)]
pub struct Aufmerksamkeitsgewichte<'a> {
    /// Q-Projektion, Zeilen zu `hidden_size`.
    pub q: &'a [i8],
    /// Eine Verschiebung je Zeile der Q-Projektion.
    pub q_skalen: &'a [u8],
    /// K-Projektion.
    pub k: &'a [i8],
    /// Eine Verschiebung je Zeile der K-Projektion.
    pub k_skalen: &'a [u8],
    /// V-Projektion.
    pub v: &'a [i8],
    /// Eine Verschiebung je Zeile der V-Projektion.
    pub v_skalen: &'a [u8],
    /// Ausgabeprojektion, Zeilen zu `num_heads · head_dim`.
    pub o: &'a [i8],
    /// Eine Verschiebung je Zeile der Ausgabeprojektion.
    pub o_skalen: &'a [u8],
}

/// Was der Vorwärtspass des Aufmerksamkeitsblocks hinterlässt.
///
/// ⚑ **Alles, was der Rückwärtspass braucht, und nichts sonst.** Die
/// Werte liegen so vor, wie die Kerne sie gesehen haben: Q und K
/// **nach** der Drehung, V ohne, die Wahrscheinlichkeiten auf ihrer
/// eigenen Skala.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Aufmerksamkeitsspur {
    /// Q je Position und Abfragekopf, nach RoPE.
    pub q: Vec<Vec<Vec<i16>>>,
    /// K je Position und Schlüsselkopf, nach RoPE.
    pub k: Vec<Vec<Vec<i16>>>,
    /// V je Position und Schlüsselkopf, **ohne** RoPE.
    pub v: Vec<Vec<Vec<i16>>>,
    /// Die Wahrscheinlichkeiten je Position und Abfragekopf.
    pub wahrscheinlichkeiten: Vec<Vec<Vec<i32>>>,
    /// Der Eingang der Ausgabeprojektion je Position, bereits auf
    /// `attn_out_frac` umskaliert.
    pub attn_aus: Vec<Vec<i16>>,
    /// Die Ausgabe des Blocks je Position.
    pub y: Vec<Vec<i16>>,
    /// Q je Position und Kopf **vor** der QK-Normierung, leer ohne sie.
    ///
    /// ⚑ Der Rueckwaertspass durch eine Normierung braucht ihren
    /// **Eingang**, nicht ihre Ausgabe; aus der Ausgabe laesst er sich
    /// nicht zurueckrechnen.
    pub q_vor_norm: Vec<Vec<Vec<i16>>>,
    /// Dasselbe fuer K.
    pub k_vor_norm: Vec<Vec<Vec<i16>>>,
    /// Die Normspur je Position und Kopf fuer Q, leer ohne Normierung.
    ///
    /// ⚑ `r` ist **nachgeschlagen** und nicht nachrechenbar, ohne den
    /// Vorwaertspass zu wiederholen.
    pub q_normspur: Vec<Vec<crate::rmsnorm::Rmsnormspur>>,
    /// Dasselbe fuer K.
    pub k_normspur: Vec<Vec<crate::rmsnorm::Rmsnormspur>>,
}

/// Der Vorwärtspass des Aufmerksamkeitsblocks über eine Folge.
///
/// # ⚑ Warum das eine eigene, öffentliche Funktion ist
///
/// **Es ist eine zweite Umsetzung**, und das ist in diesem Projekt
/// sonst verboten. Beim MLP-Block gab es sie nicht: Dort ruft der
/// Trainingsschritt denselben Kern, den auch die Inferenz ruft. Für die
/// Aufmerksamkeit gibt es keinen solchen Kern, sie steht im
/// Vorwärtspass der Laufzeit ausgeschrieben, und dort über **eine**
/// Position mit Zwischenspeicher statt über eine Folge.
///
/// ⚑ **Deshalb steht sie hier für sich und nicht im Rumpf des
/// Gradienten:** Nur so lässt sie sich gegen die aufgezeichneten
/// Zwischenwerte eines echten Vorwärtspasses stellen und **byteweise**
/// vergleichen. Eine zweite Wahrheit über den Rechenpfad ist nur
/// erträglich, solange ein Test sie an die erste bindet.
#[allow(clippy::too_many_arguments)]
pub fn vorwaerts_der_aufmerksamkeit(
    g: Aufmerksamkeitsgewichte<'_>,
    x: &[Vec<i16>],
    vorspannungen: Option<Vorspannungen<'_>>,
    cos_lut: &[i16],
    sin_lut: &[i16],
    exp_lut: &[i16],
    aus_skalen: Option<&[u8]>,
    qkn: Option<QkNormVorgaben<'_>>,
    v: Aufmerksamkeitsvorgaben,
) -> Aufmerksamkeitsspur {
    let hs = v.hidden_size;
    let hd = v.head_dim;
    let q_breite = v.num_heads * hd;
    let t_len = x.len();
    assert!(t_len > 0, "vorwaerts_der_aufmerksamkeit: leere Folge");
    assert_eq!(
        v.num_heads % v.num_kv_heads,
        0,
        "vorwaerts_der_aufmerksamkeit: {} Abfragekoepfe teilen sich nicht auf {} Schluesselkoepfe",
        v.num_heads,
        v.num_kv_heads
    );
    let gruppe = v.num_heads / v.num_kv_heads;
    let halb = hd / 2;
    assert_eq!(2 * halb, hd, "vorwaerts_der_aufmerksamkeit: head_dim muss gerade sein");
    let n_pos = cos_lut.len() / halb;
    assert!(n_pos > 0, "vorwaerts_der_aufmerksamkeit: die Kosinustabelle ist zu kurz");

    let score_mult = inv_sqrt_q15(hd);
    // Vorwaerts bringt diese Weite das Skalarprodukt auf die
    // Punktzahlskala. ⚑ **Rueckwaerts gilt sie nicht**, dort hat jeder
    // der beiden Ausgaenge seine eigene; siehe `attention_backward`.
    // ⚑ **Nach der QK-Normierung gilt ihre Ausgangsskala**, nicht die
    // der Projektion. Wer das vergisst, rechnet die Aufmerksamkeit um
    // Zweierpotenzen daneben, und zwar ohne dass etwas saettigt oder
    // ueberlaeuft: Es kommt eine andere, plausible Verteilung heraus.
    let (q_wirk, k_wirk) = match &qkn {
        Some(n) => (n.q_out_frac, n.k_out_frac),
        None => (v.q_frac, v.k_frac),
    };
    let score_shift = (q_wirk as u16 + k_wirk as u16 + 15)
        .saturating_sub(v.score_frac as u16) as u8;
    let lut_shift = v.score_frac.saturating_sub(v.exp_input_frac);

    // ---- Erste Haelfte: Q, K, V je Position ----
    let mut spur = Aufmerksamkeitsspur::default();
    for (t, xt) in x.iter().enumerate() {
        assert_eq!(xt.len(), hs, "vorwaerts_der_aufmerksamkeit: Position {t} hat die falsche Breite");
        let mut q_flat = linear_w8a16(xt, g.q, hs, g.q_skalen, v.act_frac, v.q_frac);
        let mut k_flat = linear_w8a16(xt, g.k, hs, g.k_skalen, v.act_frac, v.k_frac);
        let mut v_flat = linear_w8a16(xt, g.v, hs, g.v_skalen, v.act_frac, v.v_frac);
        if let Some(b) = vorspannungen {
            add_bias_i16(&mut q_flat, b.q, b.q_skalen, v.q_frac);
            add_bias_i16(&mut k_flat, b.k, b.k_skalen, v.k_frac);
            add_bias_i16(&mut v_flat, b.v, b.v_skalen, v.v_frac);
        }
        let idx = (v.positionsversatz + t) % n_pos;
        let cos_row = &cos_lut[idx * halb..(idx + 1) * halb];
        let sin_row = &sin_lut[idx * halb..(idx + 1) * halb];
        // ⚑ **Koepfe zuerst, dann normieren, dann RoPE**, und die
        // Reihenfolge ist Teil des Ausfuehrungsprofils: Das
        // Referenzmodell normiert vor der Drehung, und RoPE selbst ist
        // skaleninvariant, reicht die neue Skala also unveraendert
        // weiter.
        let mut q_koepfe: Vec<Vec<i16>> =
            (0..v.num_heads).map(|h| q_flat[h * hd..(h + 1) * hd].to_vec()).collect();
        let mut k_koepfe: Vec<Vec<i16>> =
            (0..v.num_kv_heads).map(|h| k_flat[h * hd..(h + 1) * hd].to_vec()).collect();
        if let Some(n) = &qkn {
            spur.q_vor_norm.push(q_koepfe.clone());
            spur.k_vor_norm.push(k_koepfe.clone());
            spur.q_normspur.push(crate::rmsnorm::qk_norm_heads_mit_spur(
                &mut q_koepfe,
                v.q_frac,
                n.q_gamma,
                n.q_gamma_shifts,
                n.rsqrt_lut,
                n.rsqrt_input_shift,
                n.rsqrt_output_frac,
                n.q_out_frac,
            ));
            spur.k_normspur.push(crate::rmsnorm::qk_norm_heads_mit_spur(
                &mut k_koepfe,
                v.k_frac,
                n.k_gamma,
                n.k_gamma_shifts,
                n.rsqrt_lut,
                n.rsqrt_input_shift,
                n.rsqrt_output_frac,
                n.k_out_frac,
            ));
        }
        spur.q.push(
            q_koepfe
                .iter()
                .map(|kopf| rotate_half_split_i16(kopf, cos_row, sin_row, v.rope_frac))
                .collect(),
        );
        spur.k.push(
            k_koepfe
                .iter()
                .map(|kopf| rotate_half_split_i16(kopf, cos_row, sin_row, v.rope_frac))
                .collect(),
        );
        // ⚑ **V wird nicht gedreht.** RoPE traegt die Position in das
        // Skalarprodukt, und V geht dort nicht ein.
        spur.v.push((0..v.num_kv_heads).map(|h| v_flat[h * hd..(h + 1) * hd].to_vec()).collect());
    }

    // ---- Zweite Haelfte: Aufmerksamkeit und Ausgabe ----
    for (t, q_t) in spur.q.iter().enumerate() {
        let mut zeile = vec![0i16; q_breite];
        let mut je_kopf: Vec<Vec<i32>> = Vec::with_capacity(v.num_heads);
        for (h, q_kopf) in q_t.iter().enumerate() {
            let kv = h / gruppe;
            let k_seq: Vec<Vec<i16>> = (0..=t).map(|j| spur.k[j][kv].clone()).collect();
            let v_seq: Vec<Vec<i16>> = (0..=t).map(|j| spur.v[j][kv].clone()).collect();
            let maske = vec![vec![true; t + 1]];
            let mut kopfspur: Vec<Vec<i32>> = Vec::new();
            let kopf = attention_int_mit_spur(
                std::slice::from_ref(q_kopf), &k_seq, &v_seq, &maske, score_mult, score_shift,
                exp_lut, lut_shift, v.prob_frac, Some(&mut kopfspur),
            );
            zeile[h * hd..(h + 1) * hd].copy_from_slice(&kopf[0]);
            je_kopf.push(kopfspur.into_iter().next().expect("eine Zeile je Abfrage"));
        }
        spur.wahrscheinlichkeiten.push(je_kopf);
        // Die Ausgabe traegt die Skala von V; die Ausgabeprojektion
        // erwartet ihre eigene.
        for w in zeile.iter_mut() {
            *w = clamp_i16(rescale(i32::from(*w), v.v_frac, v.attn_out_frac));
        }
        // ⚑ **Wahlweise eine Skala oder eine je Kanal.** Allein
        // geprueft traegt der Block eine; **in einer Ebene addiert seine
        // Ausgabe direkt in den Residualstrom**, und der traegt seit
        // Fund 20 eine Skala je Kanal. Gemessen an Qwen2.5-0,5B spannen
        // sie auf Ebene 12 von vier bis fuenfzehn: Mit einer einzigen
        // Zahl waere die Ebene nicht bitgleich zur Laufzeit.
        let y = match aus_skalen {
            Some(sk) => crate::linear::linear_w8a16_pc(
                &zeile, g.o, q_breite, g.o_skalen, v.attn_out_frac, sk,
            ),
            None => linear_w8a16(&zeile, g.o, q_breite, g.o_skalen, v.attn_out_frac, v.aus_frac),
        };
        spur.y.push(y);
        spur.attn_aus.push(zeile);
    }
    spur
}

#[allow(clippy::too_many_arguments)]
pub fn gradienten_der_aufmerksamkeit(
    q_master: &[Master],
    k_master: &[Master],
    v_master: &[Master],
    o_master: &[Master],
    x: &[Vec<i16>],
    ziel: &[Vec<i16>],
    vorspannungen: Option<Vorspannungen<'_>>,
    cos_lut: &[i16],
    sin_lut: &[i16],
    exp_lut: &[i16],
    v: Aufmerksamkeitsvorgaben,
) -> (i64, Aufmerksamkeitsgradienten) {
    let hs = v.hidden_size;
    let hd = v.head_dim;
    let q_breite = v.num_heads * hd;
    let t_len = x.len();
    assert_eq!(ziel.len(), t_len, "gradienten_der_aufmerksamkeit: Ziel passt nicht zur Folge");

    let (wq, sq) = gewicht_aus_master(q_master, hs, v.master_frac);
    let (wk, sk) = gewicht_aus_master(k_master, hs, v.master_frac);
    let (wv, sv) = gewicht_aus_master(v_master, hs, v.master_frac);
    let (wo, so) = gewicht_aus_master(o_master, q_breite, v.master_frac);
    let spur = vorwaerts_der_aufmerksamkeit(
        Aufmerksamkeitsgewichte {
            q: &wq,
            q_skalen: &sq,
            k: &wk,
            k_skalen: &sk,
            v: &wv,
            v_skalen: &sv,
            o: &wo,
            o_skalen: &so,
        },
        x,
        vorspannungen,
        cos_lut,
        sin_lut,
        exp_lut,
        None,
        // ⚑ Ohne QK-Normierung: Modelle mit ihr lehnt
        // `Shardgewichte::aus_modell` ab, bis der Rueckwaertspass sie
        // traegt.
        None,
        v,
    );
    let mut abstand = 0i64;
    let mut g_aus: Vec<Vec<Grad>> = Vec::with_capacity(t_len);
    for (t, zt) in ziel.iter().enumerate() {
        assert_eq!(
            zt.len(), hs,
            "gradienten_der_aufmerksamkeit: Ziel {t} hat die falsche Breite"
        );
        let mut zeile: Vec<Grad> = Vec::with_capacity(hs);
        for (a, b) in spur.y[t].iter().zip(zt.iter()) {
            let d = i64::from(*a) - i64::from(*b);
            abstand += d * d;
            zeile.push(begrenze(2 * d));
        }
        g_aus.push(zeile);
    }
    (
        abstand,
        gradienten_der_aufmerksamkeit_aus_gradient(
            &g_aus, x, &spur,
            Aufmerksamkeitsgewichte {
                q: &wq, q_skalen: &sq, k: &wk, k_skalen: &sk,
                v: &wv, v_skalen: &sv, o: &wo, o_skalen: &so,
            },
            cos_lut,
            sin_lut,
            // ⚑ Ohne QK-Normierung, siehe den Vorwaertspfad.
            None,
            v,

        ),
    )
}

/// Die Gradienten eines Aufmerksamkeitsblocks aus einem **eingehenden**
/// Gradienten.
///
/// # ⚑ Warum es diese Fassung gibt
///
/// Wie bei [`gradienten_des_mlp_aus_gradient`]: **In einer Ebene gibt es
/// kein Ziel je Block.** Der Aufmerksamkeitsblock bekommt seinen
/// Gradienten von der Residualaddition über ihm, und ein Ziel, das
/// gerade diesen Gradienten erzeugte, wäre eine Rückrechnung, die nichts
/// hinzufügt.
///
/// `g_aus` liegt auf dem Bus, **eine Zeile je Position**, und `spur`
/// stammt aus demselben Vorwärtspass wie die Gewichte.
#[allow(clippy::too_many_arguments)]
pub fn gradienten_der_aufmerksamkeit_aus_gradient(
    g_aus: &[Vec<Grad>],
    x: &[Vec<i16>],
    spur: &Aufmerksamkeitsspur,
    gew: Aufmerksamkeitsgewichte<'_>,
    cos_lut: &[i16],
    sin_lut: &[i16],
    qkn: Option<QkNormVorgaben<'_>>,
    v: Aufmerksamkeitsvorgaben,
) -> Aufmerksamkeitsgradienten {
    let hs = v.hidden_size;
    let hd = v.head_dim;
    let q_breite = v.num_heads * hd;
    let kv_breite = v.num_kv_heads * hd;
    let t_len = x.len();
    let gruppe = v.num_heads / v.num_kv_heads.max(1);
    let halb = hd / 2;
    let n_pos = cos_lut.len() / halb.max(1);
    let (wq, sq, wk, sk, wv, sv, wo, so) =
        (gew.q, gew.q_skalen, gew.k, gew.k_skalen, gew.v, gew.v_skalen, gew.o, gew.o_skalen);
    let score_mult = inv_sqrt_q15(hd);

    let mut gw_q = vec![0i64; wq.len()];
    let mut gw_k = vec![0i64; wk.len()];
    let mut gw_v = vec![0i64; wv.len()];
    let mut gw_o = vec![0i64; wo.len()];
    // ⚑ **Summen, keine Zuweisungen.** `dL/dk` und `dL/dv` einer
    // Position bekommen Beitraege von jedem Abfragekopf der Gruppe und
    // von jeder spaeteren Abfrage. In `i64` gesammelt und **einmal** am
    // Ende gesaettigt: Eine Saettigung je Beitrag verschoebe die Summe.
    let mut g_k_roped = vec![vec![vec![0i64; hd]; v.num_kv_heads]; t_len];
    let mut g_v_roh = vec![vec![vec![0i64; hd]; v.num_kv_heads]; t_len];
    let mut g_q_roped = vec![vec![vec![0i64; hd]; v.num_heads]; t_len];

    let skalen = Aufmerksamkeitsskalen {
        score_mult,
        score_mult_frac: 15,
        q_frac: v.q_frac,
        k_frac: v.k_frac,
        v_frac: v.v_frac,
        prob_frac: v.prob_frac,
    };

    for (t, g_y) in g_aus.iter().enumerate() {
        // Durch die Ausgabeprojektion. Der Ausgang liegt auf dem Bus.
        let (g_attn, teil_o) = linear_backward(
            g_y, &spur.attn_aus[t], wo, q_breite, so, v.aus_frac, v.attn_out_frac,
        );
        for (ziel_w, teil) in gw_o.iter_mut().zip(teil_o.iter()) {
            *ziel_w += *teil;
        }

        // ⚑ **Die Umskalierung von der V-Skala auf die Eingangsskala
        // der Ausgabeprojektion braucht hier nichts.** Sie aendert die
        // Darstellung und nicht den Wert; ein Gradient auf dem Bus
        // traegt `dL/d(realer Wert)` und ist deshalb derselbe.
        for h in 0..v.num_heads {
            let kv = h / gruppe;
            let k_seq: Vec<Vec<i16>> = (0..=t).map(|j| spur.k[j][kv].clone()).collect();
            let v_seq: Vec<Vec<i16>> = (0..=t).map(|j| spur.v[j][kv].clone()).collect();
            let maske = vec![true; t + 1];
            let (gq, gk, gv) = attention_backward(
                &g_attn[h * hd..(h + 1) * hd],
                &spur.q[t][h],
                &k_seq,
                &v_seq,
                &spur.wahrscheinlichkeiten[t][h],
                &maske,
                skalen,
            );
            for d in 0..hd {
                g_q_roped[t][h][d] += i64::from(gq[d]);
            }
            for (j, (zk, zv)) in gk.iter().zip(gv.iter()).enumerate() {
                for d in 0..hd {
                    g_k_roped[j][kv][d] += i64::from(zk[d]);
                    g_v_roh[j][kv][d] += i64::from(zv[d]);
                }
            }
        }
    }

    // Durch RoPE zurueck und dann durch die drei Eingangsprojektionen.
    let mut eingang: Vec<Vec<Grad>> = Vec::with_capacity(t_len);
    for t in 0..t_len {
        let idx = (v.positionsversatz + t) % n_pos;
        let cos_row = &cos_lut[idx * halb..(idx + 1) * halb];
        let sin_row = &sin_lut[idx * halb..(idx + 1) * halb];

        let mut g_q_flat: Vec<Grad> = Vec::with_capacity(q_breite);
        for roh in &g_q_roped[t] {
            let kopf: Vec<Grad> = roh.iter().copied().map(begrenze).collect();
            g_q_flat.extend(rope_backward(&kopf, cos_row, sin_row, v.rope_frac));
        }
        let mut g_k_flat: Vec<Grad> = Vec::with_capacity(kv_breite);
        let mut g_v_flat: Vec<Grad> = Vec::with_capacity(kv_breite);
        for (roh_k, roh_v) in g_k_roped[t].iter().zip(g_v_roh[t].iter()) {
            let kopf: Vec<Grad> = roh_k.iter().copied().map(begrenze).collect();
            g_k_flat.extend(rope_backward(&kopf, cos_row, sin_row, v.rope_frac));
            g_v_flat.extend(roh_v.iter().copied().map(begrenze));
        }

        // ⚑ **Und zurueck durch die QK-Normierung**, falls das Modell
        // sie hat (Qwen3). Sie sitzt im Vorwaertspfad **vor** RoPE,
        // also hier **danach**: Erst die Drehung zurueck, dann die
        // Normierung.
        //
        // ⛑ **Der Gradient auf die Gammas wird verworfen**, siehe
        // `qk_norm_heads_backward`. Sie sind `head_dim` Zahlen je
        // Ebene, und ein zusaetzlicher Meister verschoebe die Indizes
        // des stochastischen Rundens.
        if let Some(n) = &qkn {
            let je_kopf = |flach: &[Grad], anzahl: usize| -> Vec<Vec<Grad>> {
                (0..anzahl).map(|h| flach[h * hd..(h + 1) * hd].to_vec()).collect()
            };
            let inv_n = crate::rmsnorm::inv_n_q20(hd);
            let gq = crate::backward::qk_norm_heads_backward(
                &je_kopf(&g_q_flat, v.num_heads),
                &spur.q_vor_norm[t],
                &spur.q_normspur[t],
                v.q_frac,
                n.q_gamma,
                n.q_gamma_shifts,
                v.attn_out_frac,
                v.attn_out_frac,
                inv_n,
            );
            g_q_flat = gq.concat();
            let gk = crate::backward::qk_norm_heads_backward(
                &je_kopf(&g_k_flat, v.num_kv_heads),
                &spur.k_vor_norm[t],
                &spur.k_normspur[t],
                v.k_frac,
                n.k_gamma,
                n.k_gamma_shifts,
                v.attn_out_frac,
                v.attn_out_frac,
                inv_n,
            );
            g_k_flat = gk.concat();
        }

        // ⚑ **Der Eingangsgradient ist die Summe ueber Q, K und V**,
        // denn alle drei lesen dieselbe Zeile. In `i64` addiert und
        // **einmal** gesaettigt.
        let mut gx_summe = vec![0i64; hs];
        for (g, w, sh, ziel_w) in [
            (&g_q_flat, wq, sq, &mut gw_q),
            (&g_k_flat, wk, sk, &mut gw_k),
            (&g_v_flat, wv, sv, &mut gw_v),
        ] {
            let (gx, teil) =
                linear_backward(g, &x[t], w, hs, sh, v.attn_out_frac, v.act_frac);
            for (z, p) in ziel_w.iter_mut().zip(teil.iter()) {
                *z += *p;
            }
            for (z, p) in gx_summe.iter_mut().zip(gx.iter()) {
                *z += i64::from(*p);
            }
        }
        eingang.push(gx_summe.into_iter().map(begrenze).collect());
    }

    Aufmerksamkeitsgradienten {
        q: nach_grad(&gw_q),
        k: nach_grad(&gw_k),
        v: nach_grad(&gw_v),
        o: nach_grad(&gw_o),
        eingang,
    }
}

// ---------------------------------------------------------------------------
// Die ganze Transformer-Ebene
// ---------------------------------------------------------------------------

/// Die Gewichte einer ganzen Transformer-Ebene, in Übertragungsform.
#[derive(Debug, Clone, Copy)]
pub struct Ebenengewichte<'a> {
    /// Die vier Matrizen des Aufmerksamkeitsblocks.
    pub aufmerksamkeit: Aufmerksamkeitsgewichte<'a>,
    /// Gate-Projektion des Feedforward-Zweigs.
    pub gate: &'a [i8],
    /// Eine Verschiebung je Zeile der Gate-Projektion.
    pub gate_skalen: &'a [u8],
    /// Up-Projektion.
    pub up: &'a [i8],
    /// Eine Verschiebung je Zeile der Up-Projektion.
    pub up_skalen: &'a [u8],
    /// Down-Projektion.
    pub down: &'a [i8],
    /// Eine Verschiebung je Zeile der Down-Projektion.
    pub down_skalen: &'a [u8],
    /// Gamma der ersten Normierung, eine Skala **je Element**.
    pub gamma_ein: &'a [i8],
    /// Die Skalen dazu.
    pub gamma_ein_skalen: &'a [u8],
    /// Gamma der zweiten Normierung.
    pub gamma_mitte: &'a [i8],
    /// Die Skalen dazu.
    pub gamma_mitte_skalen: &'a [u8],
}

/// Die Zahlen einer ganzen Ebene.
///
/// ⚑ **Die Ausgabeskalen der beiden Blöcke stehen hier nicht.** Sie
/// folgen aus den Residualskalen (`min` der beiden benachbarten, Fund 31)
/// und werden **hergestellt statt geprüft**: Wer sie zusätzlich angeben
/// dürfte, könnte sie falsch angeben, und dann rechnete der Block gegen
/// eine andere Skala als die Addition darüber.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ebenenvorgaben<'a> {
    /// Die Vorgaben des Aufmerksamkeitsblocks. `aus_frac` wird
    /// überschrieben.
    pub aufmerksamkeit: Aufmerksamkeitsvorgaben,
    /// Die Vorgaben des Feedforward-Blocks. `aus_frac` wird
    /// überschrieben.
    pub mlp: Mlpvorgaben,
    /// Die QK-Normierung (Qwen3), falls die Ebene sie hat.
    ///
    /// ⚑ **`None` ist der bisherige Weg**, und dass er unveraendert
    /// bleibt, ist die Zusicherung, an der die Bitgleichheit aller
    /// schon trainierten Modelle haengt.
    pub qk_norm: Option<QkNormVorgaben<'a>>,
    /// Bruchstellen des Residualstroms beim Eintritt, **je Kanal**.
    ///
    /// ⚑ **Je Kanal und nicht eine Zahl** (Fund 20). Gemessen an
    /// Qwen2.5-0,5B spannen sie auf Ebene 12 von vier bis fünfzehn; mit
    /// einer einzigen Zahl wäre die Ebene nicht bitgleich zur Laufzeit,
    /// und der Vergleich mit dem Mitschnitt hielte zwei verschiedene
    /// Funktionen gegeneinander.
    pub residual_in_frac: &'a [u8],
    /// Bruchstellen des Residualstroms nach der Aufmerksamkeit, je Kanal.
    pub residual_mid_frac: &'a [u8],
    /// Bruchstellen der Ebenenausgabe, je Kanal.
    pub aus_frac: &'a [u8],
    /// Eingangsverschiebung der rsqrt-Tabelle.
    pub rsqrt_input_shift: u8,
    /// Ausgangsskala der rsqrt-Tabelle.
    pub rsqrt_output_frac: u8,
    /// `2^20 / hidden_size`.
    pub inv_n_q20: i64,
}

impl Ebenenvorgaben<'_> {
    /// Die Akkumulationsskala der ersten Residualaddition, je Kanal.
    ///
    /// ⚑ **Die gröbere der beiden, nicht die feinere** (Fund 31). Die
    /// Summanden müssen beide hineinpassen; auf einer feinen Skala
    /// klemmte der Blockbeitrag, bevor er überhaupt addiert wird.
    pub fn acc_attn(&self) -> Vec<u8> {
        paarweise_min(self.residual_in_frac, self.residual_mid_frac)
    }

    /// Die Akkumulationsskala der zweiten Residualaddition, je Kanal.
    pub fn acc_mlp(&self) -> Vec<u8> {
        paarweise_min(self.residual_mid_frac, self.aus_frac)
    }

    /// Die Blockvorgaben mit den hergestellten **Gradientenbussen**.
    ///
    /// ⚑ **Der Bus ist eine Zahl, die Ausgabeskala eine je Kanal, und
    /// das sind zwei verschiedene Dinge.** Vorwärts addiert der Block in
    /// den Residualstrom, also braucht er dessen Skala je Kanal.
    /// Rückwärts nimmt `linear_backward` **eine** Skala für den
    /// eingehenden Gradienten; die Ebene rechnet ihn deshalb an der
    /// Blockgrenze um.
    ///
    /// ⚑ **Genommen wird das Maximum**, denn dann ist jede Umrechnung
    /// von `acc[i]` auf den Bus ein **Linksschieben und damit exakt**.
    /// Mit dem Minimum verlöre jeder feinere Kanal Stellen, bevor der
    /// Block ihn überhaupt sieht.
    pub fn bloecke(&self) -> (Aufmerksamkeitsvorgaben, Mlpvorgaben) {
        let mut a = self.aufmerksamkeit;
        a.aus_frac = self.acc_attn().into_iter().max().unwrap_or(0);
        let mut m = self.mlp;
        m.aus_frac = self.acc_mlp().into_iter().max().unwrap_or(0);
        (a, m)
    }
}

/// Das kanalweise Minimum zweier Skalenvektoren.
fn paarweise_min(a: &[u8], b: &[u8]) -> Vec<u8> {
    assert_eq!(a.len(), b.len(), "paarweise_min: die Skalenvektoren sind ungleich lang");
    a.iter().zip(b.iter()).map(|(x, y)| *x.min(y)).collect()
}

/// Was der Vorwärtspass einer Ebene hinterlässt.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ebenenspur {
    /// Der normierte Eingang je Position: das `x` des
    /// Aufmerksamkeitsblocks.
    pub norm_ein: Vec<Vec<i16>>,
    /// Die Spur der ersten Normierung je Position.
    pub norm_ein_spur: Vec<crate::rmsnorm::Rmsnormspur>,
    /// Was der Aufmerksamkeitsblock hinterlassen hat.
    pub aufmerksamkeit: Aufmerksamkeitsspur,
    /// Der Residualstrom nach der ersten Addition.
    pub residual: Vec<Vec<i16>>,
    /// Der zweite normierte Strom: das `x` des Feedforward-Blocks.
    pub norm_mitte: Vec<Vec<i16>>,
    /// Die Spur der zweiten Normierung je Position.
    pub norm_mitte_spur: Vec<crate::rmsnorm::Rmsnormspur>,
    /// Was der Feedforward-Block hinterlassen hat, je Position.
    pub mlp: Vec<Mlpspur>,
    /// Die Ausgabe der Ebene je Position.
    pub y: Vec<Vec<i16>>,
}

/// Die Gradienten einer ganzen Ebene.
///
/// ⚑ **Die beiden Gammas fehlen, und zwar ausdrücklich.** Ein Gamma
/// liegt als `i8` mit einer Skala **je Element** vor, ein Gewicht als
/// `i8` mit einer Skala je **Zeile**; ein Master für das eine ist etwas
/// anderes als für das andere. `rmsnorm_backward` rechnet den
/// Gamma-Gradienten aus, dieser Schritt verwirft ihn, und die
/// Normierungen bleiben damit Konstanten, genau wie die Vorspannungen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ebenengradienten {
    /// Die vier Matrizen des Aufmerksamkeitsblocks.
    pub aufmerksamkeit: Aufmerksamkeitsgradienten,
    /// Die drei Matrizen des Feedforward-Blocks, **summiert über alle
    /// Positionen**.
    pub mlp: Mlpgradienten,
    /// `dL/dx` nach dem Eingang der Ebene, eine Zeile je Position, auf
    /// `residual_in_frac`.
    pub eingang: Vec<Vec<Grad>>,
}

/// Der Vorwärtspass einer ganzen Transformer-Ebene über eine Folge.
///
/// Erste Normierung, Aufmerksamkeitsblock, Residualaddition, zweite
/// Normierung, Feedforward-Block, Residualaddition.
///
/// # ⚑ Warum die Ebene und nicht die beiden Blöcke nebeneinander
///
/// Zwischen den Blöcken liegen die Stellen, an denen ein Netz bricht und
/// die kein Blocktest sieht: die beiden **Normierungen** (deren
/// Rückwärtspass von *allen* Kanälen abhängt) und die beiden
/// **Residualadditionen**, die rückwärts **Verzweigungen** sind. Der
/// Gradient läuft dort in **beide** Zweige, und wer einen davon
/// vergisst, bekommt einen Gradienten, der bergab zeigt und trotzdem
/// falsch ist.
#[allow(clippy::too_many_arguments)]
pub fn vorwaerts_der_ebene(
    g: Ebenengewichte<'_>,
    hidden: &[Vec<i16>],
    vorspannungen: Option<Vorspannungen<'_>>,
    t: Ebenentabellen<'_>,
    v: Ebenenvorgaben<'_>,
) -> Ebenenspur {
    let hs = v.aufmerksamkeit.hidden_size;
    let (a_vorgaben, m_vorgaben) = v.bloecke();
    let acc_attn = v.acc_attn();
    let acc_mlp = v.acc_mlp();
    let mut spur = Ebenenspur::default();

    // 1. Erste Normierung je Position.
    for h in hidden.iter() {
        assert_eq!(h.len(), hs, "vorwaerts_der_ebene: falsche Breite im Residualstrom");
        let mut ns = crate::rmsnorm::Rmsnormspur::Leer;
        let n = crate::rmsnorm::rmsnorm_i16_mit_spur(
            h, v.residual_in_frac, g.gamma_ein, g.gamma_ein_skalen, t.rsqrt,
            v.rsqrt_input_shift, v.rsqrt_output_frac, v.inv_n_q20,
            a_vorgaben.act_frac, Some(&mut ns),
        );
        spur.norm_ein.push(n);
        spur.norm_ein_spur.push(ns);
    }

    // 2. Der Aufmerksamkeitsblock ueber die ganze Folge.
    spur.aufmerksamkeit = vorwaerts_der_aufmerksamkeit(
        g.aufmerksamkeit, &spur.norm_ein, vorspannungen, t.cos, t.sin, t.exp,
        Some(&acc_attn),
        v.qk_norm,
        a_vorgaben,
    );

    // 3. Erste Residualaddition, zweite Normierung, Feedforward,
    //    zweite Residualaddition.
    for (nr, h) in hidden.iter().enumerate() {
        let o_aus = &spur.aufmerksamkeit.y[nr];
        let mut residual = vec![0i16; hs];
        for i in 0..hs {
            let r = rescale_i64(i64::from(h[i]), v.residual_in_frac[i], acc_attn[i]);
            let summe = r + i64::from(o_aus[i]);
            residual[i] =
                clamp_i16_von_i64(rescale_i64(summe, acc_attn[i], v.residual_mid_frac[i]));
        }

        let mut ns = crate::rmsnorm::Rmsnormspur::Leer;
        let norm = crate::rmsnorm::rmsnorm_i16_mit_spur(
            &residual, v.residual_mid_frac, g.gamma_mitte, g.gamma_mitte_skalen, t.rsqrt,
            v.rsqrt_input_shift, v.rsqrt_output_frac, v.inv_n_q20,
            m_vorgaben.act_frac, Some(&mut ns),
        );

        let mut ms = Mlpspur::default();
        let mlp_out = mlp_int_mit_spur(
            &norm, g.gate, g.up, g.down, m_vorgaben.hidden_size, m_vorgaben.intermediate_size,
            g.gate_skalen, g.up_skalen, g.down_skalen, t.silu,
            m_vorgaben.act_frac, m_vorgaben.gate_frac, m_vorgaben.up_frac,
            m_vorgaben.down_in_frac, m_vorgaben.silu_in_frac, m_vorgaben.silu_lut_offset,
            m_vorgaben.silu_out_frac, &acc_mlp, Some(&mut ms),
        );

        let mut y = vec![0i16; hs];
        for i in 0..hs {
            let r = rescale_i64(i64::from(residual[i]), v.residual_mid_frac[i], acc_mlp[i]);
            let summe = r + i64::from(mlp_out[i]);
            y[i] = clamp_i16_von_i64(rescale_i64(summe, acc_mlp[i], v.aus_frac[i]));
        }

        spur.residual.push(residual);
        spur.norm_mitte.push(norm);
        spur.norm_mitte_spur.push(ns);
        spur.mlp.push(ms);
        spur.y.push(y);
    }
    spur
}

/// Rechnet Abstand und Gradienten einer ganzen Ebene.
///
/// # ⚑ Die Residualaddition ist rückwärts eine Verzweigung
///
/// Vorwärts ist `out = residual + block`. Rückwärts bekommt **jeder der
/// beiden Summanden den vollen Gradienten**, denn beide gehen mit dem
/// Faktor eins ein. Wer nur den Blockzweig bedient, verliert den Weg,
/// auf dem der Gradient an der Ebene **vorbei** nach unten läuft, und
/// genau dieser Weg ist der Grund, warum tiefe Netze überhaupt
/// trainierbar sind.
///
/// ⚑ **Die Busse stehen ausgeschrieben.** Jeder Gradient trägt
/// `dL/dZ` nach dem realen Wert, dargestellt auf einer Skala, und
/// zwischen zwei Kernen wird sie mit `rescale_i64` gewechselt. Die
/// Wechsel sind hier benannt, statt sich aus den Argumenten zu ergeben:
/// Es sind sechs Stück, und sie waren die Fehlerquelle der Funde 173 und
/// 175.
#[allow(clippy::too_many_arguments)]
pub fn gradienten_der_ebene(
    g: Ebenengewichte<'_>,
    spur: &Ebenenspur,
    hidden: &[Vec<i16>],
    ziel: &[Vec<i16>],
    t: Ebenentabellen<'_>,
    v: Ebenenvorgaben<'_>,
) -> (i64, Ebenengradienten) {
    let hs = v.aufmerksamkeit.hidden_size;
    let t_len = hidden.len();
    assert_eq!(ziel.len(), t_len, "gradienten_der_ebene: Ziel passt nicht zur Folge");

    // Der Verlustgradient am Ausgang der Ebene, auf `aus_frac`.
    let mut abstand = 0i64;
    let mut g_aus: Vec<Vec<Grad>> = Vec::with_capacity(t_len);
    for (nr, zt) in ziel.iter().enumerate() {
        assert_eq!(zt.len(), hs, "gradienten_der_ebene: Ziel {nr} hat die falsche Breite");
        let mut zeile = Vec::with_capacity(hs);
        for (a, b) in spur.y[nr].iter().zip(zt.iter()) {
            let d = i64::from(*a) - i64::from(*b);
            abstand += d * d;
            zeile.push(begrenze(2 * d));
        }
        g_aus.push(zeile);
    }
    (abstand, gradienten_der_ebene_aus_gradient(g, spur, hidden, &g_aus, t, v))
}

/// Die Gradienten einer Ebene aus einem **eingehenden** Gradienten.
///
/// # ⚑ Warum es diese Fassung gibt
///
/// [`gradienten_der_ebene`] rechnet den Verlustgradienten selbst aus,
/// aus einem Ziel und einem quadratischen Abstand. **In einem
/// Trainingslauf gibt es kein Ziel je Ebene:** Die letzte Ebene bekommt
/// ihren Gradienten vom Kopf, jede andere von der Ebene darüber. Wer die
/// Schleife aus der Zielfassung zusammensetzen wollte, müsste sich je
/// Ebene ein Ziel ausdenken, das gerade den gewünschten Gradienten
/// erzeugt.
///
/// `g_aus` liegt **je Kanal** auf `aus_frac`, eine Zeile je Position.
#[allow(clippy::too_many_arguments)]
pub fn gradienten_der_ebene_aus_gradient(
    g: Ebenengewichte<'_>,
    spur: &Ebenenspur,
    hidden: &[Vec<i16>],
    g_aus: &[Vec<Grad>],
    t: Ebenentabellen<'_>,
    v: Ebenenvorgaben<'_>,
) -> Ebenengradienten {
    let (a_vorgaben, m_vorgaben) = v.bloecke();
    let t_len = hidden.len();
    assert_eq!(g_aus.len(), t_len, "gradienten_der_ebene: Gradient passt nicht zur Folge");

    let a_bus = a_vorgaben.aus_frac;
    let m_bus = m_vorgaben.aus_frac;
    let mid_bus = v.residual_mid_frac.iter().copied().max().unwrap_or(0);
    let in_bus = v.residual_in_frac.iter().copied().max().unwrap_or(0);
    let mut mlp_gesamt = Mlpgradienten {
        gate: vec![0; g.gate.len()],
        up: vec![0; g.up.len()],
        down: vec![0; g.down.len()],
        eingang: Vec::new(),
    };
    let mut g_residual: Vec<Vec<Grad>> = Vec::with_capacity(t_len);

    for (nr, g_y) in g_aus.iter().enumerate() {
        // Zweite Residualaddition: **beide** Zweige bekommen `g_aus`.
        // Der Bus wechselt dabei von `aus_frac` auf die Skala, die der
        // jeweilige Empfaenger erwartet.
        // ⚑ **Je Kanal herein, eine Skala hinaus.** Der Verlustgradient
        // liegt auf der Ausgabeskala des jeweiligen Kanals; der Block
        // rechnet auf **einem** Bus.
        let g_mlp: Vec<Grad> = g_y
            .iter()
            .enumerate()
            .map(|(i, x)| begrenze(rescale_i64(i64::from(*x), v.aus_frac[i], m_bus)))
            .collect();
        let teil = gradienten_des_mlp_aus_gradient(
            &g_mlp, &spur.norm_mitte[nr], &spur.mlp[nr],
            g.gate, g.gate_skalen, g.up, g.up_skalen, g.down, g.down_skalen,
            t.silu, t.silu_grad, m_vorgaben,
        );
        for (z, p) in mlp_gesamt.gate.iter_mut().zip(teil.gate.iter()) {
            *z = begrenze(i64::from(*z) + i64::from(*p));
        }
        for (z, p) in mlp_gesamt.up.iter_mut().zip(teil.up.iter()) {
            *z = begrenze(i64::from(*z) + i64::from(*p));
        }
        for (z, p) in mlp_gesamt.down.iter_mut().zip(teil.down.iter()) {
            *z = begrenze(i64::from(*z) + i64::from(*p));
        }

        // Durch die zweite Normierung: von `norm_mlp_frac` auf
        // `residual_mid_frac`.
        let g_norm = normrueckwaerts(
            &teil.eingang, &spur.residual[nr], v.residual_mid_frac, g.gamma_mitte,
            g.gamma_mitte_skalen, &spur.norm_mitte_spur[nr], v.inv_n_q20,
            m_vorgaben.act_frac, mid_bus,
        );
        // Der zweite Zweig der Addition, ebenfalls auf `residual_mid_frac`.
        // Beide Zweige der Addition, beide auf die Kanalskala des
        // Residualstroms gebracht.
        let zeile: Vec<Grad> = g_y
            .iter()
            .zip(g_norm.iter())
            .enumerate()
            .map(|(i, (a, b))| {
                begrenze(
                    rescale_i64(i64::from(*a), v.aus_frac[i], v.residual_mid_frac[i])
                        + rescale_i64(i64::from(*b), mid_bus, v.residual_mid_frac[i]),
                )
            })
            .collect();
        g_residual.push(zeile);
    }
    mlp_gesamt.eingang = Vec::new();

    // Durch den Aufmerksamkeitsblock, mit dem Bus auf `acc_attn`.
    let g_attn: Vec<Vec<Grad>> = g_residual
        .iter()
        .map(|z| {
            z.iter()
                .enumerate()
                .map(|(i, x)| begrenze(rescale_i64(i64::from(*x), v.residual_mid_frac[i], a_bus)))
                .collect()
        })
        .collect();
    let a_grad = gradienten_der_aufmerksamkeit_aus_gradient(
        &g_attn, &spur.norm_ein, &spur.aufmerksamkeit, g.aufmerksamkeit, t.cos, t.sin,
        // ⚑ Ohne QK-Normierung, siehe den Vorwaertspfad.
        None,
        a_vorgaben,
    );

    // Durch die erste Normierung und die erste Residualaddition.
    let mut eingang: Vec<Vec<Grad>> = Vec::with_capacity(t_len);
    for nr in 0..t_len {
        let g_norm = normrueckwaerts(
            &a_grad.eingang[nr], &hidden[nr], v.residual_in_frac, g.gamma_ein,
            g.gamma_ein_skalen, &spur.norm_ein_spur[nr], v.inv_n_q20,
            a_vorgaben.act_frac, in_bus,
        );
        let zeile: Vec<Grad> = g_residual[nr]
            .iter()
            .zip(g_norm.iter())
            .enumerate()
            .map(|(i, (a, b))| {
                begrenze(
                    rescale_i64(i64::from(*a), v.residual_mid_frac[i], v.residual_in_frac[i])
                        + rescale_i64(i64::from(*b), in_bus, v.residual_in_frac[i]),
                )
            })
            .collect();
        eingang.push(zeile);
    }

    Ebenengradienten { aufmerksamkeit: a_grad, mlp: mlp_gesamt, eingang }
}

/// Die sechs Tabellen, die eine Ebene braucht.
///
/// ⚑ **Ein Typ und kein Tupel.** Sechs `&[i16]` hintereinander sind
/// sechs vertauschbare Scheiben: Wer Sinus und Kosinus vertauscht,
/// bekommt keine Fehlermeldung, sondern eine Ebene, die eine andere
/// Funktion rechnet. Mit Feldnamen steht die Zuordnung an der
/// Aufrufstelle.
#[derive(Debug, Clone, Copy)]
pub struct Ebenentabellen<'a> {
    /// Kosinus je Position, flach `[max_pos, head_dim/2]`.
    pub cos: &'a [i16],
    /// Sinus, ebenso.
    pub sin: &'a [i16],
    /// Die Exponentialtabelle des Softmax.
    pub exp: &'a [i16],
    /// Die Tabelle des Kehrwerts der Wurzel.
    pub rsqrt: &'a [i16],
    /// Die Silu-Tabelle des Vorwärtspasses.
    pub silu: &'a [i16],
    /// Ihr Gradientenvorrat, **aus derselben Tabelle abgeleitet**.
    pub silu_grad: &'a [i16],
}

/// Die beiden Normierungsgewichte einer Ebene.
///
/// ⚑ **Ebenfalls ein Typ.** Zwei Paare aus Werten und Skalen sind vier
/// Scheiben; die erste Normierung sitzt vor der Aufmerksamkeit, die
/// zweite vor dem Feedforward, und ein Tausch fiele nur als schlechteres
/// Ergebnis auf.
#[derive(Debug, Clone, Copy)]
pub struct Normierungsgewichte<'a> {
    /// Gamma der Normierung **vor** dem Aufmerksamkeitsblock.
    pub ein: &'a [i8],
    /// Eine Skala je Element.
    pub ein_skalen: &'a [u8],
    /// Gamma der Normierung **vor** dem Feedforward-Block.
    pub mitte: &'a [i8],
    /// Eine Skala je Element.
    pub mitte_skalen: &'a [u8],
}

/// Ein vollständiger Schritt auf einer **ganzen Transformer-Ebene**:
/// beide Normierungen, beide Blöcke, beide Residualadditionen.
///
/// Gibt den quadratischen Abstand **vor** dem Schritt zurück.
///
/// ⚑ **Sieben Matrizen, sieben Indexversätze.** Ohne sie bekämen
/// Gewichte an derselben Stelle denselben Würfel, und das stochastische
/// Runden wäre zwischen ihnen korreliert.
///
/// ⚑ **Die beiden Gammas bewegen sich nicht.** Ein Gamma trägt eine
/// Skala je Element, ein Gewicht eine je Zeile; ein Master für das eine
/// ist etwas anderes als für das andere. Das steht hier, weil ein
/// stillschweigend eingefrorener Parameter sonst als „lernt nicht"
/// auffiele.
#[allow(clippy::too_many_arguments)]
pub fn schritt_auf_ebene(
    q_master: &mut [Master],
    k_master: &mut [Master],
    v_master: &mut [Master],
    o_master: &mut [Master],
    gate_master: &mut [Master],
    up_master: &mut [Master],
    down_master: &mut [Master],
    gammas: Normierungsgewichte<'_>,
    hidden: &[Vec<i16>],
    ziel: &[Vec<i16>],
    vorspannungen: Option<Vorspannungen<'_>>,
    t: Ebenentabellen<'_>,
    v: Ebenenvorgaben<'_>,
) -> i64 {
    let hs = v.aufmerksamkeit.hidden_size;
    let q_breite = v.aufmerksamkeit.num_heads * v.aufmerksamkeit.head_dim;
    let mf = v.aufmerksamkeit.master_frac;

    let (wq, sq) = gewicht_aus_master(q_master, hs, mf);
    let (wk, sk) = gewicht_aus_master(k_master, hs, mf);
    let (wv, sv) = gewicht_aus_master(v_master, hs, mf);
    let (wo, so) = gewicht_aus_master(o_master, q_breite, mf);
    let (wg, sg) = gewicht_aus_master(gate_master, hs, v.mlp.master_frac);
    let (wu, su) = gewicht_aus_master(up_master, hs, v.mlp.master_frac);
    let (wd, sd) = gewicht_aus_master(down_master, v.mlp.intermediate_size, v.mlp.master_frac);

    let gew = Ebenengewichte {
        aufmerksamkeit: Aufmerksamkeitsgewichte {
            q: &wq, q_skalen: &sq, k: &wk, k_skalen: &sk,
            v: &wv, v_skalen: &sv, o: &wo, o_skalen: &so,
        },
        gate: &wg, gate_skalen: &sg,
        up: &wu, up_skalen: &su,
        down: &wd, down_skalen: &sd,
        gamma_ein: gammas.ein,
        gamma_ein_skalen: gammas.ein_skalen,
        gamma_mitte: gammas.mitte,
        gamma_mitte_skalen: gammas.mitte_skalen,
    };
    let spur = vorwaerts_der_ebene(gew, hidden, vorspannungen, t, v);
    let (abstand, gr) = gradienten_der_ebene(gew, &spur, hidden, ziel, t, v);

    let k = v.aufmerksamkeit.kennung;
    let mut versatz = k.index_versatz;
    for (m, g) in [
        (&mut *q_master, &gr.aufmerksamkeit.q),
        (&mut *k_master, &gr.aufmerksamkeit.k),
        (&mut *v_master, &gr.aufmerksamkeit.v),
        (&mut *o_master, &gr.aufmerksamkeit.o),
        (&mut *gate_master, &gr.mlp.gate),
        (&mut *up_master, &gr.mlp.up),
        (&mut *down_master, &gr.mlp.down),
    ] {
        let laenge = m.len() as u64;
        schritt(
            m,
            g,
            Schrittkennung { index_versatz: versatz, ..k },
            v.aufmerksamkeit.lr_zaehler,
            v.aufmerksamkeit.lr_nenner,
        );
        versatz += laenge;
    }
    abstand
}

/// Der Rückwärtspass einer Normierung, mit der Spur aus dem
/// Vorwärtspass.
///
/// ⚑ **Ein leerer oder nur mit Nullen gefüllter Eingang hat keinen
/// Gradienten**, und die Spur sagt das als eigener Fall: Bei
/// [`crate::rmsnorm::Rmsnormspur::Null`] hängt die Ausgabe von keinem
/// Eingang ab.
#[allow(clippy::too_many_arguments)]
/// Der Rückwärtsweg durch eine RMS-Normierung, mit Skalenwechsel.
///
/// ⚑ **Öffentlich seit dem 2026-09-05**, weil die Gemischebene in der
/// Runtime gebaut wird und ihn braucht: Dort liegt die Materialisierung
/// der gewählten Experten, hier die Arithmetik. Ein Nachbau drüben wäre
/// eine zweite Wahrheit über einen heiklen Randfall, nämlich den
/// zweiten Zweig unten: **Eine leere Spur ergibt einen Nullgradienten
/// und keinen Absturz.**
pub fn normrueckwaerts(
    g: &[Grad],
    x: &[i16],
    x_shifts: &[u8],
    gamma: &[i8],
    gamma_skalen: &[u8],
    spur: &crate::rmsnorm::Rmsnormspur,
    inv_n_q20: i64,
    g_frac: u8,
    gx_frac: u8,
) -> Vec<Grad> {
    match spur {
        crate::rmsnorm::Rmsnormspur::Wert { r, norm_frac, ref_shift } => {
            let (gx, _ggamma) = crate::backward::rmsnorm_backward(
                g, x, x_shifts, gamma, gamma_skalen,
                crate::backward::Normskalen {
                    r: *r,
                    norm_frac: *norm_frac,
                    ref_shift: *ref_shift,
                    inv_n_q20,
                    g_frac,
                    gx_frac,
                },
            );
            gx
        }
        _ => vec![0; g.len()],
    }
}

/// Saettigt einen `i64`-Gradienten auf [`Grad`].
///
/// ⚑ **Gesaettigt statt umlaufend**, denn ein umgelaufener Gradient
/// zeigt in die **Gegenrichtung**, und der Schritt liefe dann bergauf.
///
/// ⚑ **Öffentlich seit dem 2026-09-05**, aus demselben Grund wie
/// [`normrueckwaerts`]: Wer in der Runtime `as Grad` schriebe statt zu
/// sättigen, bekäme genau den umlaufenden Gradienten, vor dem dieser
/// Kommentar warnt.
pub fn begrenze(v: i64) -> Grad {
    v.clamp(i64::from(Grad::MIN), i64::from(Grad::MAX)) as Grad
}

/// `dL/dW` kommt als `i64` je Gewicht; der Optimierer nimmt [`Grad`].
fn nach_grad(gw: &[i64]) -> Vec<Grad> {
    gw.iter().copied().map(begrenze).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Der Kreis ist geschlossen: Der Abstand sinkt.**
    ///
    /// Das ist die Aussage, die kein einzelner Golden-Vektor treffen
    /// kann. Jeder Kern kann fuer sich richtig sein und die Kette
    /// trotzdem in die falsche Richtung laufen, wenn eine Skala
    /// zwischen zweien nicht passt.
    #[test]
    fn der_abstand_sinkt_ueber_mehrere_schritte() {
        let in_features = 8usize;
        let zeilen = 4usize;
        // ⚑ **Ein Master steht in Rasterstufen, nicht in feinen
        // Einheiten.** Die erste Fassung dieses Tests schob ihn um
        // `FEIN_BITS` nach links; dann lagen die Werte bei fuenf
        // Millionen, `gewicht_aus_master` schob sie um sechzehn Stellen
        // zurueck, und ein Schritt von wenigen hundert war danach
        // unsichtbar. **Der Abstand blieb exakt stehen**, und der Test
        // hat damit eine Skalenverwechslung gefunden, die kein
        // Golden-Vektor je zeigen wuerde: `FEIN_BITS` sind
        // **unterhalb** der Rasterstufe und gehoeren in den Schritt,
        // nicht in die Darstellung.
        let mut master: Vec<Master> = (0..zeilen * in_features)
            .map(|i| ((i as i32 * 37) % 11 - 5) << 8)
            .collect();
        let x: Vec<i16> = (0..in_features).map(|i| ((i * 13) % 9) as i16 - 4).collect();
        let ziel: Vec<i16> = (0..zeilen).map(|i| (i as i16 + 1) * 40).collect();

        // ⚑ **Die Lernrate ist gerechnet, nicht geraten.** Der Schritt
        // ist `g · lr · 2^FEIN_BITS / nenner` in feinen Einheiten, also
        // `g · lr / nenner` in Rasterstufen. Mit Gradienten in der
        // Groessenordnung zweihundert und einem Ziel von etwa einer
        // Fuenftel Rasterstufe je Schritt folgt `nenner ≈ 1000`.
        //
        // ⛑ Mit `1/4` stieg der Abstand von 64 214 auf 80 089: Der
        // Schritt sprang ueber das Ziel hinaus. **Das ist kein Fehler
        // der Kerne, sondern eine Lernrate, die nicht zur Skala passt**,
        // und es ist genau die Sorte Fehler, die einzeln gepruefte
        // Kerne nicht zeigen koennen.
        let mut erster = 0i64;
        let mut letzter = 0i64;
        for s in 0..300u64 {
            let a = schritt_auf_linear(
                &mut master,
                &x,
                &ziel,
                Schrittvorgaben {
                    in_features,
                    act_frac_bits: 8,
                    out_frac_bits: 8,
                    // ⚑ **Acht Bruchstellen, und die Zahl ist
                    // gerechnet.** Der reale Wert eines Gewichts liegt
                    // bei fuenf, der Master also bei 1 280, und bis zum
                    // Betrag 32 512 bleibt Luft, bevor eine neunte
                    // Stelle wegfiele. Mit null Bruchstellen haette der
                    // Lauf nach wenigen Schritten den darstellbaren
                    // Bereich verlassen.
                    master_frac: 8,
                    // Der Schritt wirkt jetzt auf den Master und nicht
                    // auf die Rasterstufe: dieselbe Bewegung braucht
                    // den Faktor 2^8.
                    lr_zaehler: 256,
                    lr_nenner: 1_000,
                    kennung: Schrittkennung { ebene: 0, schritt: s, index_versatz: 0 },
                },
            );
            if s == 0 {
                erster = a;
            }
            letzter = a;
        }
        assert!(
            letzter < erster,
            "der Abstand stieg: {erster} auf {letzter}. Ein Kern kann fuer sich \
             richtig sein und die Kette trotzdem falsch herum laufen"
        );
    }

    /// Baut die Silu-Tabelle, wie ein Artefakt sie mitbringt.
    ///
    /// ⚑ **Hier nachgebaut und nicht geladen**, aus demselben Grund wie
    /// in `backward.rs`: Der Test soll die Kette pruefen und nicht die
    /// Verfuegbarkeit einer Datei. Gleitkomma ist hier erlaubt, weil es
    /// nur den Testaufbau baut und nie laeuft.
    fn silu_tabelle(min: i32, max: i32, in_frac: u8, out_frac: u8) -> Vec<i16> {
        (min..=max)
            .map(|x| {
                let xf = x as f64 / (1 << in_frac) as f64;
                let sg = 1.0 / (1.0 + (-xf).exp());
                (xf * sg * (1 << out_frac) as f64).round() as i16
            })
            .collect()
    }

    fn mlp_vorgaben(lr_nenner: i64, schritt: u64) -> Mlpvorgaben {
        Mlpvorgaben {
            hidden_size: 8,
            intermediate_size: 16,
            act_frac: 8,
            gate_frac: 8,
            up_frac: 8,
            down_in_frac: 8,
            aus_frac: 8,
            // ⚑ Acht, siehe `der_abstand_sinkt_ueber_mehrere_schritte`.
            master_frac: 8,
            silu_in_frac: 6,
            silu_lut_offset: 256,
            silu_out_frac: 12,
            lr_zaehler: 1,
            lr_nenner,
            kennung: Schrittkennung { ebene: 0, schritt, index_versatz: 0 },
        }
    }

    /// ⚑ **Der Kreis ist auch ueber einen ganzen MLP-Block geschlossen.**
    ///
    /// **Das ist die Aussage, die `der_abstand_sinkt_ueber_mehrere_schritte`
    /// nicht trifft.** Dort haengt eine einzige lineare Ebene an einem
    /// Ziel; hier liegen vier Skalenuebergaenge hintereinander (Gate nach
    /// Silu, Silu mal Up, Produkt nach Down, Down in die Ausgabe), und
    /// **genau dort bricht Ganzzahltraining**, nicht im einzelnen Kern.
    #[test]
    fn der_abstand_sinkt_auch_ueber_den_ganzen_mlp_block() {
        let v0 = mlp_vorgaben(1, 0);
        let (hs, is) = (v0.hidden_size, v0.intermediate_size);
        let lut = silu_tabelle(-256, 256, v0.silu_in_frac, v0.silu_out_frac);
        let grad_lut = crate::backward::silu_grad_aus_lut(&lut);

        let mut gate: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 29) % 13 - 6) << 8).collect();
        let mut up: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 17) % 11 - 5) << 8).collect();
        let mut down: Vec<Master> = (0..hs * is).map(|i| ((i as i32 * 23) % 9 - 4) << 8).collect();
        let x: Vec<i16> = (0..hs).map(|i| ((i * 13) % 9) as i16 - 4).collect();
        let ziel: Vec<i16> = (0..hs).map(|i| (i as i16 + 1) * 30).collect();

        let mut erster = 0i64;
        let mut letzter = 0i64;
        for s in 0..400u64 {
            let a = schritt_auf_mlp(
                &mut gate,
                &mut up,
                &mut down,
                &x,
                &ziel,
                &lut,
                &grad_lut,
                mlp_vorgaben(400, s),
            );
            if s == 0 {
                erster = a;
            }
            letzter = a;
        }
        assert!(
            letzter < erster,
            "der Abstand stieg oder blieb: {erster} auf {letzter}. Jeder Kern kann fuer \
             sich richtig sein und die Kette trotzdem falsch herum laufen"
        );
    }

    /// ⚑ **Und er sinkt deutlich, nicht nur um eins.**
    ///
    /// ⛑ Ohne diese Schaerfe bestuende der Test auch dann, wenn der
    /// Gradient fast ueberall null waere und nur ein einziges Gewicht
    /// zufaellig in die richtige Richtung ruckte.
    #[test]
    fn der_abstand_sinkt_deutlich() {
        let v0 = mlp_vorgaben(1, 0);
        let (hs, is) = (v0.hidden_size, v0.intermediate_size);
        let lut = silu_tabelle(-256, 256, v0.silu_in_frac, v0.silu_out_frac);
        let grad_lut = crate::backward::silu_grad_aus_lut(&lut);

        let mut gate: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 29) % 13 - 6) << 8).collect();
        let mut up: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 17) % 11 - 5) << 8).collect();
        let mut down: Vec<Master> = (0..hs * is).map(|i| ((i as i32 * 23) % 9 - 4) << 8).collect();
        let x: Vec<i16> = (0..hs).map(|i| ((i * 13) % 9) as i16 - 4).collect();
        let ziel: Vec<i16> = (0..hs).map(|i| (i as i16 + 1) * 30).collect();

        let mut erster = 0i64;
        let mut letzter = 0i64;
        for s in 0..400u64 {
            let a = schritt_auf_mlp(
                &mut gate, &mut up, &mut down, &x, &ziel, &lut, &grad_lut,
                mlp_vorgaben(400, s),
            );
            if s == 0 {
                erster = a;
            }
            letzter = a;
        }
        assert!(
            letzter * 2 < erster,
            "der Abstand fiel nur von {erster} auf {letzter}, also um weniger als die Haelfte"
        );
    }

    /// Wie oft ein Schub **gegen** den Gradienten den Abstand senkt.
    ///
    /// Gibt `(gesenkt, gewertet)` je Matrix zurueck. ⚑ **Unveraenderte
    /// Faelle zaehlen nicht mit**: Ein Schub, den die Quantisierung
    /// schluckt, sagt ueber die Richtung nichts, und ihn als Fehlschlag
    /// zu werten machte die Messung zu einer Messung der Rasterweite.
    fn richtungstreffer(schub: i32) -> [(usize, usize); 3] {
        let v = mlp_vorgaben(400, 0);
        let (hs, is) = (v.hidden_size, v.intermediate_size);
        let lut = silu_tabelle(-256, 256, v.silu_in_frac, v.silu_out_frac);
        let grad_lut = crate::backward::silu_grad_aus_lut(&lut);
        let gate: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 29) % 13 - 6) << 8).collect();
        let up: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 17) % 11 - 5) << 8).collect();
        let down: Vec<Master> = (0..hs * is).map(|i| ((i as i32 * 23) % 9 - 4) << 8).collect();
        let x: Vec<i16> = (0..hs).map(|i| ((i * 13) % 9) as i16 - 4).collect();
        let ziel: Vec<i16> = (0..hs).map(|i| (i as i16 + 1) * 30).collect();
        let (l0, gr) = gradienten_des_mlp(&gate, &up, &down, &x, &ziel, &lut, &grad_lut, v);

        let mut aus = [(0usize, 0usize); 3];
        for (n, (grad, laenge)) in
            [(&gr.gate, gate.len()), (&gr.up, up.len()), (&gr.down, down.len())]
                .iter()
                .enumerate()
        {
            for i in 0..*laenge {
                let g = grad[i];
                if g == 0 {
                    continue;
                }
                let (mut ga, mut u, mut d) = (gate.clone(), up.clone(), down.clone());
                match n {
                    0 => ga[i] -= schub * g.signum(),
                    1 => u[i] -= schub * g.signum(),
                    _ => d[i] -= schub * g.signum(),
                };
                let l = gradienten_des_mlp(&ga, &u, &d, &x, &ziel, &lut, &grad_lut, v).0;
                if l == l0 {
                    continue;
                }
                aus[n].1 += 1;
                if l < l0 {
                    aus[n].0 += 1;
                }
            }
        }
        aus
    }

    /// ⚑ **Der Gradient zeigt bergab, und zwar messbar.**
    ///
    /// „Der Abstand sinkt" belegt nur, dass die Kette bergab laeuft. ⛑
    /// **Zwei Gegenproben blieben damit gruen** (die Gradientenaeste
    /// vertauscht, der Vorrat auf der falschen Skala), denn
    /// Abstiegsverfahren sind gutmuetig: Viele falsche, aber
    /// korrelierte Richtungen senken einen quadratischen Abstand auch.
    ///
    /// **Hier wird der Gradient an seiner Definition gemessen**, und
    /// zwar ueber alle Gewichte statt an einem einzelnen. ⚑ **Ein
    /// einzelnes reicht nicht:** Der Schub muss die Quantisierung
    /// ueberwinden und liegt damit weit ausserhalb des linearen
    /// Bereichs; gemessen sagt der Gradient in 82 bis 85 Prozent der
    /// Faelle richtig voraus, nicht in hundert. **Eine Zusicherung auf
    /// eine einzelne Stelle waere deshalb ein Zufallsgenerator.**
    #[test]
    fn der_gradient_zeigt_ueberwiegend_bergab() {
        let treffer = richtungstreffer(1024);
        for (n, name) in ["gate", "up", "down"].iter().enumerate() {
            let (gesenkt, gewertet) = treffer[n];
            assert!(
                gewertet >= 30,
                "{name}: nur {gewertet} Schuebe aenderten ueberhaupt etwas; \
                 die Messung traegt dann nicht"
            );
            // 70 Prozent liegt deutlich ueber dem Zufall (50) und
            // deutlich unter dem Gemessenen (seit Fund 174: hundert).
            assert!(
                gesenkt * 10 >= gewertet * 7,
                "{name}: nur {gesenkt} von {gewertet} Schueben gegen den Gradienten \
                 senkten den Abstand. Der Gradient zeigt nicht verlaesslich bergab"
            );
        }
    }

    /// ⚑ **Gate und Up tragen vergleichbar grosse Gradienten.**
    ///
    /// **Ein Richtungstest kann einen Skalenfehler nicht sehen.** ⛑ Die
    /// Gegenprobe „der Gradientenvorrat auf der falschen Skala" macht
    /// den Gate-Gradienten zweiunddreissigmal kleiner, **ohne sein
    /// Vorzeichen zu aendern**; der Abstieg laeuft dann weiter bergab,
    /// nur fuer eine der drei Matrizen mit einer stillschweigend
    /// anderen Lernrate. Gemessen: mittlerer Betrag 159 gegen 122 im
    /// gesunden Fall, **5 gegen 122** mit dem Fehler.
    ///
    /// Die Zusicherung ist begruendet und nicht gegriffen: Gate und Up
    /// sind **strukturell symmetrisch**, dieselbe Eingabe, dieselbe
    /// Form, vergleichbare Gewichte. Ihre Gradienten duerfen sich
    /// unterscheiden, aber nicht um Groessenordnungen.
    #[test]
    fn gate_und_up_tragen_vergleichbar_grosse_gradienten() {
        let v = mlp_vorgaben(400, 0);
        let (hs, is) = (v.hidden_size, v.intermediate_size);
        let lut = silu_tabelle(-256, 256, v.silu_in_frac, v.silu_out_frac);
        let grad_lut = crate::backward::silu_grad_aus_lut(&lut);
        let gate: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 29) % 13 - 6) << 8).collect();
        let up: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 17) % 11 - 5) << 8).collect();
        let down: Vec<Master> = (0..hs * is).map(|i| ((i as i32 * 23) % 9 - 4) << 8).collect();
        let x: Vec<i16> = (0..hs).map(|i| ((i * 13) % 9) as i16 - 4).collect();
        let ziel: Vec<i16> = (0..hs).map(|i| (i as i16 + 1) * 30).collect();
        let (_, gr) = gradienten_des_mlp(&gate, &up, &down, &x, &ziel, &lut, &grad_lut, v);

        let mittel = |g: &[Grad]| -> i64 {
            let s: i64 = g.iter().map(|v| i64::from(*v).abs()).sum();
            s / g.len().max(1) as i64
        };
        let (mg, mu) = (mittel(&gr.gate), mittel(&gr.up));
        assert!(mg > 0 && mu > 0, "ein Ast traegt gar keinen Gradienten: gate {mg}, up {mu}");
        let (klein, gross) = if mg < mu { (mg, mu) } else { (mu, mg) };
        assert!(
            gross <= klein * 8,
            "gate {mg} und up {mu} unterscheiden sich um mehr als das Achtfache; \
             das ist kein Unterschied der Daten mehr, sondern eine Skala"
        );
    }

    // ---- Aufmerksamkeitsblock ---------------------------------------

    const A_HS: usize = 8;
    const A_KOEPFE: usize = 4;
    const A_KV: usize = 2;
    const A_HD: usize = 8;
    const A_LEN: usize = 6;
    const A_MAXPOS: usize = 8;

    /// Sinus- und Kosinustabelle, flach `[A_MAXPOS, head_dim/2]`.
    ///
    /// ⚑ **Nachgebaut und nicht geladen**, aus demselben Grund wie die
    /// Silu-Tabelle: Der Test prueft die Kette und nicht die
    /// Verfuegbarkeit einer Datei. Gleitkomma baut hier nur den Aufbau.
    fn rope_tabellen() -> (Vec<i16>, Vec<i16>) {
        let halb = A_HD / 2;
        let mut cos = Vec::with_capacity(A_MAXPOS * halb);
        let mut sin = Vec::with_capacity(A_MAXPOS * halb);
        for t in 0..A_MAXPOS {
            for j in 0..halb {
                // ⚑ **Grundzahl drei und nicht eine Million.** Bei vier
                // Paaren dreht die grosse Grundzahl nur das erste
                // merklich, die uebrigen um Bruchteile eines
                // Tausendstels; eine Tabelle, die fast die
                // Einheitsdrehung ist, prueft weder die Ruecknahme der
                // Drehung noch den Positionsversatz. ⛑ Mit sechzehn
                // blieben beide Gegenproben zum Versatz gruen, mit drei
                // beissen sie.
                let theta = 1.0f64 / 3f64.powf(j as f64 / halb as f64);
                let w = t as f64 * theta;
                cos.push((w.cos() * 256.0).round() as i16);
                sin.push((w.sin() * 256.0).round() as i16);
            }
        }
        (cos, sin)
    }

    /// `exp(-x)` auf `score_frac` Bruchstellen, Eingangsraster
    /// `2^-exp_input_frac`.
    fn exp_tabelle(score_frac: u8, exp_input_frac: u8) -> Vec<i16> {
        (0..4096)
            .map(|i| {
                let x = i as f64 / (1u32 << exp_input_frac) as f64;
                ((-x).exp() * (1u32 << score_frac) as f64).round() as i16
            })
            .collect()
    }

    /// ⚑ **Die Skalen sind absichtlich paarweise verschieden.** Waeren
    /// sie gleich, fielen die Schiebeweiten des Vorwaerts- und des
    /// Rueckwaertspasses zusammen und der Aufbau pruefte die
    /// Skalenbuchhaltung nicht, um die es hier geht.
    fn a_vorgaben(lr_nenner: i64, lr_zaehler: i64, schritt: u64) -> Aufmerksamkeitsvorgaben {
        Aufmerksamkeitsvorgaben {
            hidden_size: A_HS,
            num_heads: A_KOEPFE,
            num_kv_heads: A_KV,
            head_dim: A_HD,
            // ⚑ **Nicht gleich `aus_frac`.** Waeren beide 8, fielen die
            // Skalen der Eingangs- und der Ausgabeprojektion zusammen
            // und eine Verwechslung waere unsichtbar.
            act_frac: 9,
            q_frac: 8,
            k_frac: 7,
            v_frac: 10,
            // ⚑ **Nicht gleich `act_frac` und nicht gleich `v_frac`.**
            // Sonst waeren die Umskalierung vor der Ausgabeprojektion
            // und die Wahl des Gradientenbusses beide wirkungslos, und
            // eine Verwechslung fiele nicht auf.
            attn_out_frac: 11,
            aus_frac: 8,
            score_frac: 12,
            prob_frac: 14,
            exp_input_frac: 8,
            rope_frac: 8,
            // ⚑ **Nicht null.** Bei Position null ist die Drehung die
            // Einheit, und der Schluessel der ersten Position wird von
            // jeder Abfrage gelesen; ein Aufbau, der dort beginnt,
            // gewichtet ausgerechnet die Stelle am staerksten, an der
            // RoPE nichts tut.
            positionsversatz: 1,
            // ⚑ **Vierzehn, und die Zahl folgt aus dem Aufbau.** Die
            // Master liegen bei Betraegen um 11 000, also fallen sieben
            // Stellen weg und sieben bleiben als Zeilenverschiebung.
            // Der reale Wert eines Gewichts ist damit unter eins.
            master_frac: 14,
            lr_zaehler,
            lr_nenner,
            kennung: Schrittkennung { ebene: 0, schritt, index_versatz: 0 },
        }
    }

    /// Vier Mastermatrizen, eine Folge und ein Ziel.
    #[allow(clippy::type_complexity)]
    fn a_aufbau() -> (Vec<Master>, Vec<Master>, Vec<Master>, Vec<Master>, Vec<Vec<i16>>, Vec<Vec<i16>>)
    {
        let qb = A_KOEPFE * A_HD;
        let kvb = A_KV * A_HD;
        // ⚑ **Master in der Groessenordnung 2^13, nicht 2^3.** Bei
        // Werten unter 128 gibt `gewicht_aus_master` die Verschiebung
        // null zurueck, das Gewicht ist dann ganzzahlig, und ein Schub
        // von eins ist eine Aenderung um hundert Prozent. Erst hier
        // bekommt eine Zeile eine echte Verschiebung (sieben), das
        // Gewicht liegt unter eins, und ein Schub von 128 ist ein
        // einziges Bit der Uebertragungsform.
        let q: Vec<Master> = (0..qb * A_HS).map(|i| (i as i32 * 6337) % 21001 - 10500).collect();
        let k: Vec<Master> = (0..kvb * A_HS).map(|i| (i as i32 * 4177) % 20001 - 10000).collect();
        let v: Vec<Master> = (0..kvb * A_HS).map(|i| (i as i32 * 2749) % 19001 - 9500).collect();
        let o: Vec<Master> = (0..A_HS * qb).map(|i| (i as i32 * 3391) % 20501 - 10250).collect();
        // ⚑ **Je Position ein anderer Eingang.** Bei gleichen Eingaengen
        // waeren alle Schluessel gleich, der Softmax gleichverteilt und
        // sein Gradient nahe null: Der Aufbau pruefte dann nichts.
        let x: Vec<Vec<i16>> = (0..A_LEN)
            .map(|t| (0..A_HS).map(|i| (((i + 3 * t) * 13 % 17) as i16 - 8) * 30).collect())
            .collect();
        let ziel: Vec<Vec<i16>> = (0..A_LEN)
            .map(|t| (0..A_HS).map(|i| ((i as i16 + 1) * 20) - (t as i16 * 15)).collect())
            .collect();
        (q, k, v, o, x, ziel)
    }

    /// ⚑ **Der Kreis ist auch ueber den Aufmerksamkeitsblock
    /// geschlossen**, und der hat mehr Skalenuebergaenge als der MLP:
    /// drei Projektionen auf drei verschiedene Skalen, RoPE, das
    /// Skalarprodukt auf die Punktzahlskala, der Softmax auf die
    /// Wahrscheinlichkeitsskala, die Gewichtung zurueck auf die V-Skala,
    /// die Umskalierung auf die Eingangsskala der Ausgabeprojektion.
    #[test]
    fn der_abstand_des_aufmerksamkeitsblocks_sinkt_deutlich() {
        let (mut q, mut k, mut v, mut o, x, ziel) = a_aufbau();
        let (cos, sin) = rope_tabellen();
        let v0 = a_vorgaben(1, 1, 0);
        let exp = exp_tabelle(v0.score_frac, v0.exp_input_frac);

        let mut erster = 0i64;
        let mut letzter = 0i64;
        for s in 0..300u64 {
            let a = schritt_auf_aufmerksamkeit(
                &mut q, &mut k, &mut v, &mut o, &x, &ziel, None, &cos, &sin, &exp,
                a_vorgaben(1 << 11, 1, s),
            );
            if s == 0 {
                erster = a;
            }
            letzter = a;
        }
        assert!(
            letzter * 2 < erster,
            "der Abstand fiel nur von {erster} auf {letzter}, also um weniger als die \
             Haelfte. Jeder Kern kann fuer sich richtig sein und die Kette trotzdem \
             falsch herum laufen"
        );
    }

    /// ⚑ **Und bergauf, wenn man das Vorzeichen dreht.**
    ///
    /// ⛑ Ohne diese Gegenprobe bliebe offen, ob der Abstand faellt, weil
    /// der Gradient stimmt, oder weil irgendeine Bewegung ihn faellt.
    #[test]
    fn mit_umgekehrtem_schritt_steigt_der_abstand_des_aufmerksamkeitsblocks() {
        let (mut q, mut k, mut v, mut o, x, ziel) = a_aufbau();
        let (cos, sin) = rope_tabellen();
        let v0 = a_vorgaben(1, 1, 0);
        let exp = exp_tabelle(v0.score_frac, v0.exp_input_frac);

        // ⚑ **Acht Schritte und nicht dreissig.** Bergauf wachsen die
        // Gewichte, und ab dem Betrag `127 · 2^master_frac` kann die
        // Uebertragungsform sie nicht mehr ausdruecken: Der Lauf bricht
        // dann ab, statt eine Aussage zu liefern. Acht genuegen, die
        // Aussage ist ohnehin nach dem ersten Schritt sichtbar.
        let mut erster = 0i64;
        let mut letzter = 0i64;
        for s in 0..8u64 {
            let a = schritt_auf_aufmerksamkeit(
                &mut q, &mut k, &mut v, &mut o, &x, &ziel, None, &cos, &sin, &exp,
                a_vorgaben(1 << 11, -1, s),
            );
            if s == 0 {
                erster = a;
            }
            letzter = a;
        }
        assert!(
            letzter > erster,
            "mit umgekehrtem Schritt sank der Abstand von {erster} auf {letzter}; \
             dann faellt er nicht wegen des Gradienten"
        );
    }

    /// ⚑ **Auf einer einzigen Position bekaemen Q und K exakt null**,
    /// und deshalb rechnet dieser Schritt ueber eine Folge.
    ///
    /// Bei einer Position gibt es genau einen Schluessel, der Softmax
    /// liefert exakt eins, und `p · (g − ⟨g, p⟩)` ist damit null. ⛑ **Ein
    /// Aufbau mit einer Position haette also ausschliesslich V und die
    /// Ausgabeprojektion geprueft**, waehrend die Ueberschrift
    /// „Aufmerksamkeitsblock" lautet. Der Test haelt das fest, damit
    /// niemand den Aufbau spaeter aus Bequemlichkeit kuerzt.
    #[test]
    fn auf_einer_einzigen_position_bekommen_q_und_k_keinen_gradienten() {
        let (q, k, v, o, x, ziel) = a_aufbau();
        let (cos, sin) = rope_tabellen();
        let vg = a_vorgaben(1 << 11, 1, 0);
        let exp = exp_tabelle(vg.score_frac, vg.exp_input_frac);

        let (_, gr) = gradienten_der_aufmerksamkeit(
            &q, &k, &v, &o, &x[..1], &ziel[..1], None, &cos, &sin, &exp, vg,
        );
        assert!(gr.q.iter().all(|g| *g == 0), "Q bekam auf einer Position einen Gradienten");
        assert!(gr.k.iter().all(|g| *g == 0), "K bekam auf einer Position einen Gradienten");
        // ⛑ Und die andere Haelfte ist nicht null, sonst bewiese der
        // Test nur, dass gar nichts gerechnet wurde.
        assert!(gr.v.iter().any(|g| *g != 0), "V bekam gar keinen Gradienten");
        assert!(gr.o.iter().any(|g| *g != 0), "die Ausgabeprojektion bekam gar keinen Gradienten");

        // Ueber die ganze Folge bekommen sie welchen.
        let (_, ganz) = gradienten_der_aufmerksamkeit(
            &q, &k, &v, &o, &x, &ziel, None, &cos, &sin, &exp, vg,
        );
        assert!(ganz.q.iter().any(|g| *g != 0), "Q bekam auch ueber die Folge keinen Gradienten");
        assert!(ganz.k.iter().any(|g| *g != 0), "K bekam auch ueber die Folge keinen Gradienten");
    }

    /// Geht **entlang des Gradienten** einen Schritt und vergleicht die
    /// gemessene Aenderung des Abstands mit der vorhergesagten.
    ///
    /// Gibt `(gemessen, vorhergesagt)` je Matrix zurueck.
    ///
    /// # ⚑ Warum entlang des Gradienten und nicht Gewicht fuer Gewicht
    ///
    /// ⛑ Ein einzelnes Gewicht zu schieben misst hier nichts: Der
    /// Schub muss die Quantisierung ueberwinden, die gemessene
    /// Aenderung ist dann ein ganzzahliger Sprung, und die
    /// vorhergesagte liegt darunter. Gemessen lagen die Korrelationen
    /// von Q, K und V bei **null**, waehrend die Richtung in neun von
    /// zehn Faellen stimmte. **Ein Schritt entlang des ganzen
    /// Gradienten mittelt die Spruenge heraus** und prueft genau die
    /// Aussage, die ein Gradient macht.
    ///
    /// # ⚑ Die Umrechnung von `dL/dW` auf den zurueckgegebenen Abstand
    ///
    /// `dL/dW` entsteht als `Gradient · Eingang` und traegt
    /// `2^(Bus + Eingangsskala)`. **Und die beiden sind je Matrix
    /// verschieden:** Q, K und V lesen den Blockeingang, die
    /// Ausgabeprojektion liest die Aufmerksamkeitsausgabe, und ihr
    /// eingehender Gradient liegt nicht auf dem Bus, sondern auf der
    /// Ausgabeskala. Vom Uebertragungsgewicht zum Master kommen
    /// `2^(2 · Verschiebung)` dazu. Und der zurueckgegebene Abstand ist
    /// der der **Darstellung**, der Gradient der des **Wertes**;
    /// dazwischen liegt `2^(2 · aus_frac)`.
    fn a_schritt_entlang_des_gradienten() -> [(f64, f64); 4] {
        let (q, k, v, o, x, ziel) = a_aufbau();
        let (cos, sin) = rope_tabellen();
        let vg = a_vorgaben(1 << 11, 1, 0);
        let exp = exp_tabelle(vg.score_frac, vg.exp_input_frac);
        let rechne = |q: &[Master], k: &[Master], v: &[Master], o: &[Master]| {
            gradienten_der_aufmerksamkeit(q, k, v, o, &x, &ziel, None, &cos, &sin, &exp, vg)
        };
        let (l0, gr) = rechne(&q, &k, &v, &o);
        let qb = A_KOEPFE * A_HD;
        // ⚑ **Der Schritt darf hoechstens ein Fuenfzigstel des Abstands
        // versprechen.** Ein Gradient ist eine Aussage ueber die
        // **Umgebung**; wer weiter geht, misst die Kruemmung mit. Die
        // Schranke gilt fuer alle vier Matrizen gleich, und jede
        // bekommt danach ihre eigene Schrittweite: Ihre Gradienten
        // liegen zwei Groessenordnungen auseinander.
        let deckel = l0 as f64 / 50.0;

        let mut aus = [(0.0f64, 0.0f64); 4];
        for (n, eintrag) in aus.iter_mut().enumerate() {
            let (grad, breite, bus_und_eingang) = match n {
                0 => (&gr.q, A_HS, vg.attn_out_frac as i32 + vg.act_frac as i32),
                1 => (&gr.k, A_HS, vg.attn_out_frac as i32 + vg.act_frac as i32),
                2 => (&gr.v, A_HS, vg.attn_out_frac as i32 + vg.act_frac as i32),
                _ => (&gr.o, qb, vg.aus_frac as i32 + vg.attn_out_frac as i32),
            };
            let master: &[Master] = match n {
                0 => &q,
                1 => &k,
                2 => &v,
                _ => &o,
            };
            let (_, verschiebungen) = gewicht_aus_master(master, breite, vg.master_frac);
            let spitze = grad.iter().map(|g| i64::from(*g).abs()).max().unwrap_or(0).max(1);
            // Der Schritt als Vielfaches: `delta_i = -g_i · weite / Spitze`,
            // also hoechstens `weite` Rasterstufen am groessten Gradienten.
            let vorhersage = |weite: i64| -> (Vec<Master>, f64) {
                let mut neu = master.to_vec();
                let mut vorher = 0.0f64;
                for (i, g) in grad.iter().enumerate() {
                    let delta = -(i64::from(*g) * weite / spitze);
                    if delta == 0 {
                        continue;
                    }
                    neu[i] = neu[i].saturating_add(delta as i32);
                    let s = i32::from(verschiebungen[i / breite]);
                    let hoch = 2 * vg.aus_frac as i32 - (bus_und_eingang + 2 * s);
                    vorher += delta as f64 * f64::from(*g) * 2f64.powi(hoch);
                }
                (neu, vorher)
            };
            // Mindestweite zwei, siehe
            // `der_eingangsgradient_der_ebene_sagt_die_aenderung_voraus`.
            let mut gewaehlt = vorhersage(2);
            for weite in [4i64, 8, 16, 32, 64, 128, 256, 512, 1024] {
                let kandidat = vorhersage(weite);
                if kandidat.1.abs() > deckel {
                    break;
                }
                gewaehlt = kandidat;
            }
            let (neu, vorher) = gewaehlt;
            let l1 = match n {
                0 => rechne(&neu, &k, &v, &o).0,
                1 => rechne(&q, &neu, &v, &o).0,
                2 => rechne(&q, &k, &neu, &o).0,
                _ => rechne(&q, &k, &v, &neu).0,
            };
            *eintrag = ((l1 - l0) as f64, vorher);
        }
        aus
    }

    /// ⚑ **Der Gradient sagt die Aenderung des Abstands voraus, nach
    /// Richtung und nach Groesse.**
    ///
    /// ⛑ **Das ist die Zusicherung, die den Richtungstest ueberholt.**
    /// Vier Gegenproben blieben gruen, solange nur die Richtung geprueft
    /// wurde: der Rueckwaertspass durch RoPE entfernt (eine Drehung
    /// laesst den Betrag unveraendert und dreht die Richtung nur
    /// teilweise), der K-Gradient zugewiesen statt summiert, und Bus und
    /// Eingangsskala vertauscht (ein Faktor auf drei der vier Matrizen,
    /// **auf alle drei derselbe**, und deshalb weder beim Vergleich der
    /// Aeste noch bei der Richtung sichtbar).
    ///
    /// ⚑ **Die Schranke ist eng, und das darf sie sein:** Der ganze
    /// Lauf ist ganzzahlig und damit auf jeder Maschine bitgleich.
    /// Gemessen liegen die vier Verhaeltnisse bei 0,84 / 1,11 / 0,88 /
    /// 1,03; die Schranke laesst 0,65 bis 1,50. Sie faengt damit einen
    /// Faktor zwei, und gemessen faengt sie ausserdem: die weggenommene
    /// Ruecknahme der Drehung (0,25 fuer Q, 0,36 fuer K), einen
    /// Positionsversatz, der nur auf einer Seite gilt (0,48 und 0,64),
    /// und einen zugewiesenen statt summierten K-Gradienten.
    ///
    /// # ⚑ Warum hier kein Test einzelne Gewichte schiebt
    ///
    /// Der MLP-Block hat einen (`der_gradient_zeigt_ueberwiegend_bergab`),
    /// und dort traegt er: Ein Schub gegen den Gradienten senkt den
    /// Abstand in 82 bis 85 Prozent der Faelle. **Im
    /// Aufmerksamkeitsblock schwankt dieselbe Messung ueber jede
    /// Schranke hinweg**, die man setzen koennte. Gemessen ueber sieben
    /// Schubweiten von 64 bis 1024:
    ///
    /// | | Q | K | V | O |
    /// |---|---|---|---|---|
    /// | Anteil richtig | 63 bis 77 % | 61 bis 80 % | 84 bis 95 % | 79 bis 95 % |
    ///
    /// Der Gradient stimmt dabei nachweislich: Ein Schritt entlang des
    /// ganzen Gradienten senkt den Abstand um das 0,84- bis 1,11-fache
    /// des vorhergesagten Betrags.
    ///
    /// **Der Grund ist der Weg, den Q und K nehmen.** Sie wirken nur
    /// ueber den Softmax, ihr Beitrag zur Ausgabe ist klein, und ein
    /// Schub, der die Quantisierung ueberwindet, ist laengst ausserhalb
    /// des linearen Bereichs. Ein solcher Test mit einer Schranke von
    /// siebzig Prozent waere nicht scharf, sondern **nur manchmal
    /// gruen**, und ein Test, der ohne Fehler rot wird, erzeugt Druck,
    /// ihn wegzunehmen. **Bei siebzig Prozent waere er fuer Q bei drei
    /// von sieben Schubweiten rot und fuer K bei drei**, ohne dass am
    /// Gradienten etwas falsch ist.
    #[test]
    fn der_gradient_sagt_die_aenderung_des_abstands_voraus() {
        let treffer = a_schritt_entlang_des_gradienten();
        for (n, name) in ["q", "k", "v", "o"].iter().enumerate() {
            let (gemessen, vorher) = treffer[n];
            assert!(
                vorher < -1.0,
                "{name}: der Schritt sagt keine Senkung voraus ({vorher:.1}); \
                 dann prueft der Vergleich nichts"
            );
            assert!(
                gemessen < 0.0,
                "{name}: der Abstand stieg um {gemessen:.0}, wo der Gradient eine \
                 Senkung um {vorher:.0} vorhersagte"
            );
            let verhaeltnis = gemessen / vorher;
            assert!(
                (0.75..=1.5).contains(&verhaeltnis),
                "{name}: gemessen {gemessen:.0}, vorhergesagt {vorher:.0}, also das \
                 {verhaeltnis:.2}-fache. Das ist kein Rundungsfehler mehr, sondern eine Skala"
            );
        }
    }

    /// ⚑ **Der Eingangsgradient sagt die Aenderung des Abstands
    /// voraus, im MLP-Block und im Aufmerksamkeitsblock.**
    ///
    /// ⛑ **Ohne diesen Test koennte das Feld jede Bedeutung tragen.** Es
    /// wird vom Blockschritt nicht gebraucht und erst beim Zusammenbau
    /// zur Ebene gelesen; ein ungelesenes Feld ist eine Einladung, ihm
    /// spaeter etwas anderes zu unterstellen.
    ///
    /// ⚑ **Und es prueft die Summe.** Gate und Up lesen denselben
    /// Eingang, Q, K und V ebenso. Wer nur einen Beitrag nimmt,
    /// halbiert oder drittelt den Gradienten, **ohne seine Richtung zu
    /// aendern**; genau das sieht der Groessenvergleich und ein
    /// Richtungstest nicht.
    ///
    /// # ⚑ Die Umrechnung, und sie ist eine andere als bei den Gewichten
    ///
    /// `eingang` traegt `dL/dx` nach dem **realen** Wert, dargestellt auf
    /// `act_frac`. Ein Schub `d` am ganzzahligen Eingang ist real
    /// `d / 2^act_frac`, und der zurueckgegebene Abstand traegt
    /// `2^(2 · aus_frac)`. Zusammen `d · g · 2^(2·(aus_frac − act_frac))`.
    /// **Eine Zeilenverschiebung kommt nicht vor**, denn der Eingang geht
    /// nicht durch `gewicht_aus_master`.
    ///
    /// # ⚑ Der Eingang dieses Aufbaus ist zehnmal groesser als der der
    /// uebrigen MLP-Tests
    ///
    /// ⛑ Mit dem urspruenglichen `x` (Betraege bis vier) misst dieser
    /// Test das **Eingangsraster** und nicht den Gradienten: Ein Schub um
    /// eins ist dort eine Aenderung um fuenfundzwanzig Prozent, und
    /// gemessen stimmten nur **zwei von acht** Richtungen. Mit Betraegen
    /// bis vierzig sind es **acht von acht**, und das Verhaeltnis liegt
    /// bei 0,87 / 0,93 / 0,86 fuer Schrittweiten 1 / 2 / 4. Dieselbe
    /// Lehre wie im Aufmerksamkeitsaufbau: **Ein Testaufbau, dessen
    /// Raster den Effekt ueberdeckt, prueft das Raster.**
    ///
    /// # ⚑ Und eine Gegenprobe, die **nicht** beisst, samt Messung
    ///
    /// Im Aufmerksamkeitsblock bleibt `zugewiesen statt summiert` fuer
    /// den **Eingangs**gradienten gruen: Verhaeltnis 0,985 gesund gegen
    /// 0,990 mit dem Fehler. **Der Grund ist kein Testmangel, sondern
    /// eine Eigenschaft der Aufmerksamkeit:** `x` wirkt ueber V direkt
    /// und linear auf die Ausgabe, ueber Q und K dagegen nur durch den
    /// Softmax. **V traegt den Eingangsgradienten praktisch allein**,
    /// und was Q und K beitragen, liegt unter dem Rauschen dieser
    /// Messung.
    ///
    /// **Das ist fuer den Zusammenbau zur Ebene wichtig zu wissen** und
    /// nicht nur eine Testnotiz: Der Gradient, der durch den
    /// Aufmerksamkeitsblock nach unten laeuft, ist im Wesentlichen der
    /// Wertpfad. Was die Zeile schuetzt, ist derselbe
    /// Summationsschritt im **Gewichts**pfad, und dort beissen beide
    /// Gegenproben.
    #[test]
    fn der_eingangsgradient_sagt_die_aenderung_des_abstands_voraus() {
        // ---- MLP-Block ----
        let v = mlp_vorgaben(400, 0);
        let (hs, is) = (v.hidden_size, v.intermediate_size);
        let lut = silu_tabelle(-256, 256, v.silu_in_frac, v.silu_out_frac);
        let grad_lut = crate::backward::silu_grad_aus_lut(&lut);
        let gate: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 29) % 13 - 6) << 8).collect();
        let up: Vec<Master> = (0..is * hs).map(|i| ((i as i32 * 17) % 11 - 5) << 8).collect();
        let down: Vec<Master> = (0..hs * is).map(|i| ((i as i32 * 23) % 9 - 4) << 8).collect();
        let x: Vec<i16> = (0..hs).map(|i| (((i * 13) % 9) as i16 - 4) * 10).collect();
        let ziel: Vec<i16> = (0..hs).map(|i| (i as i16 + 1) * 30).collect();
        let (l0, gr) = gradienten_des_mlp(&gate, &up, &down, &x, &ziel, &lut, &grad_lut, v);
        assert!(gr.eingang.iter().any(|g| *g != 0), "der Eingangsgradient ist ueberall null");

        let hoch = 2 * (i32::from(v.aus_frac) - i32::from(v.act_frac));
        let spitze = gr.eingang.iter().map(|g| i64::from(*g).abs()).max().unwrap_or(1).max(1);
        let mut neu = x.clone();
        let mut vorher = 0.0f64;
        for (i, g) in gr.eingang.iter().enumerate() {
            // ⚑ Schrittweite zwei, gemessen und nicht geraten: siehe oben.
            let d = -(i64::from(*g) * 2 / spitze);
            if d == 0 {
                continue;
            }
            neu[i] = neu[i].saturating_add(d as i16);
            vorher += d as f64 * f64::from(*g) * 2f64.powi(hoch);
        }
        let gemessen =
            (gradienten_des_mlp(&gate, &up, &down, &neu, &ziel, &lut, &grad_lut, v).0 - l0) as f64;
        pruefe_vorhersage("MLP-Eingang", gemessen, vorher);

        // ---- Aufmerksamkeitsblock ----
        let (q, k, vv, o, ax, aziel) = a_aufbau();
        let (cos, sin) = rope_tabellen();
        let avg = a_vorgaben(1 << 11, 1, 0);
        let exp = exp_tabelle(avg.score_frac, avg.exp_input_frac);
        let rechne = |xx: &[Vec<i16>]| {
            gradienten_der_aufmerksamkeit(&q, &k, &vv, &o, xx, &aziel, None, &cos, &sin, &exp, avg)
        };
        let (al0, agr) = rechne(&ax);
        assert_eq!(agr.eingang.len(), ax.len(), "eine Zeile je Position");
        assert!(
            agr.eingang.iter().any(|z| z.iter().any(|g| *g != 0)),
            "der Eingangsgradient des Aufmerksamkeitsblocks ist ueberall null"
        );

        let ahoch = 2 * (i32::from(avg.aus_frac) - i32::from(avg.act_frac));
        let aspitze = agr
            .eingang
            .iter()
            .flat_map(|z| z.iter())
            .map(|g| i64::from(*g).abs())
            .max()
            .unwrap_or(1)
            .max(1);
        let mut aneu = ax.clone();
        let mut avorher = 0.0f64;
        for (t, zeile) in agr.eingang.iter().enumerate() {
            for (i, g) in zeile.iter().enumerate() {
                let d = -(i64::from(*g) * 8 / aspitze);
                if d == 0 {
                    continue;
                }
                aneu[t][i] = aneu[t][i].saturating_add(d as i16);
                avorher += d as f64 * f64::from(*g) * 2f64.powi(ahoch);
            }
        }
        let agemessen = (rechne(&aneu).0 - al0) as f64;
        pruefe_vorhersage("Aufmerksamkeits-Eingang", agemessen, avorher);
    }

    /// Gemeinsame Schranke fuer die Vorhersagevergleiche.
    fn pruefe_vorhersage(name: &str, gemessen: f64, vorher: f64) {
        assert!(vorher < -1.0, "{name}: der Schritt sagt keine Senkung voraus ({vorher:.1})");
        assert!(
            gemessen < 0.0,
            "{name}: der Abstand stieg um {gemessen:.0}, wo eine Senkung um {vorher:.0} \
             vorhergesagt war"
        );
        let verhaeltnis = gemessen / vorher;
        assert!(
            (0.7..=1.4).contains(&verhaeltnis),
            "{name}: gemessen {gemessen:.0}, vorhergesagt {vorher:.0}, also das \
             {verhaeltnis:.2}-fache"
        );
    }

    // ---- Die ganze Ebene ---------------------------------------------

    const E_IS: usize = 16;

    /// Die Tabelle des Kehrwerts der Wurzel, nachgebaut wie in
    /// `rmsnorm.rs`: `lut[x] = 4096 / sqrt(x)`.
    fn rsqrt_tabelle() -> Vec<i16> {
        (0..4096)
            .map(|x| if x == 0 { 256 } else { (4096.0 / (x as f64).sqrt()).round() as i16 })
            .collect()
    }

    /// Zwei Gammas mit Skalen je Element, beide nahe eins.
    fn e_gammas() -> (Vec<i8>, Vec<u8>, Vec<i8>, Vec<u8>) {
        let ein: Vec<i8> = (0..A_HS).map(|i| 60 + (i as i8 % 5) * 3).collect();
        let ein_s = vec![6u8; A_HS];
        let mitte: Vec<i8> = (0..A_HS).map(|i| 70 - (i as i8 % 4) * 5).collect();
        let mitte_s = vec![6u8; A_HS];
        (ein, ein_s, mitte, mitte_s)
    }

    /// ⚑ **Die drei Residualskalen sind paarweise verschieden**, sonst
    /// faellt eine Verwechslung der Akkumulationsskalen nicht auf.
    /// Die drei Residualskalen des Ebenentests, kanalweise gleich.
    ///
    /// ⚑ **Gleich, und im echten Modell sind sie es nicht.** Auf
    /// Ebene 12 von Qwen2.5-0,5B spannen sie von vier bis fuenfzehn;
    /// dass die Ebene damit umgeht, prueft der Vergleich mit dem
    /// Mitschnitt und nicht dieser Aufbau.
    fn e_skalen() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        (vec![11u8; A_HS], vec![8u8; A_HS], vec![10u8; A_HS])
    }

    fn e_vorgaben<'a>(
        sk: &'a (Vec<u8>, Vec<u8>, Vec<u8>),
        lr_nenner: i64,
        lr_zaehler: i64,
        schritt: u64,
    ) -> Ebenenvorgaben<'a> {
        let mut a = a_vorgaben(lr_nenner, lr_zaehler, schritt);
        a.act_frac = 9;
        let mut m = mlp_vorgaben(lr_nenner, schritt);
        m.hidden_size = A_HS;
        m.intermediate_size = E_IS;
        m.act_frac = 8;
        m.master_frac = 14;
        m.lr_zaehler = lr_zaehler;
        Ebenenvorgaben {
            // ⚑ Dieses Pruefmodell hat keine QK-Normierung.
            qk_norm: None,
            aufmerksamkeit: a,
            mlp: m,
            residual_in_frac: &sk.0,
            residual_mid_frac: &sk.1,
            aus_frac: &sk.2,
            rsqrt_input_shift: 0,
            rsqrt_output_frac: 12,
            inv_n_q20: ((1i64 << 20) as f64 / A_HS as f64).round() as i64,
        }
    }

    /// Sieben Mastermatrizen, eine Folge und ein Ziel.
    #[allow(clippy::type_complexity)]
    fn e_aufbau() -> (Vec<Vec<Master>>, Vec<Vec<i16>>, Vec<Vec<i16>>) {
        let (q, k, v, o, _, _) = a_aufbau();
        let gate: Vec<Master> = (0..E_IS * A_HS).map(|i| (i as i32 * 6337) % 21001 - 10500).collect();
        let up: Vec<Master> = (0..E_IS * A_HS).map(|i| (i as i32 * 4177) % 20001 - 10000).collect();
        let down: Vec<Master> = (0..A_HS * E_IS).map(|i| (i as i32 * 2749) % 19001 - 9500).collect();
        let hidden: Vec<Vec<i16>> = (0..A_LEN)
            .map(|t| (0..A_HS).map(|i| (((i + 3 * t) * 13 % 17) as i16 - 8) * 20).collect())
            .collect();
        let ziel: Vec<Vec<i16>> = (0..A_LEN)
            .map(|t| (0..A_HS).map(|i| ((i as i16 % 3) - 1) * 12 - (t as i16)).collect())
            .collect();
        (vec![q, k, v, o, gate, up, down], hidden, ziel)
    }

    /// Ein Schritt auf der ganzen Ebene, mit allen Tabellen.
    #[allow(clippy::too_many_arguments)]
    fn e_schritt(
        m: &mut [Vec<Master>],
        hidden: &[Vec<i16>],
        ziel: &[Vec<i16>],
        v: Ebenenvorgaben<'_>,
        cos: &[i16],
        sin: &[i16],
        exp: &[i16],
        rsqrt: &[i16],
        silu: &[i16],
        grad: &[i16],
        gam: &(Vec<i8>, Vec<u8>, Vec<i8>, Vec<u8>),
    ) -> i64 {
        let (q, rest) = m.split_at_mut(1);
        let (k, rest) = rest.split_at_mut(1);
        let (vv, rest) = rest.split_at_mut(1);
        let (o, rest) = rest.split_at_mut(1);
        let (gate, rest) = rest.split_at_mut(1);
        let (up, down) = rest.split_at_mut(1);
        schritt_auf_ebene(
            &mut q[0], &mut k[0], &mut vv[0], &mut o[0],
            &mut gate[0], &mut up[0], &mut down[0],
            Normierungsgewichte {
                ein: &gam.0, ein_skalen: &gam.1, mitte: &gam.2, mitte_skalen: &gam.3,
            },
            hidden, ziel, None,
            Ebenentabellen { cos, sin, exp, rsqrt, silu, silu_grad: grad },
            v,
        )
    }

    /// ⚑ **Der Kreis ist ueber eine ganze Transformer-Ebene
    /// geschlossen.**
    ///
    /// Zwischen den beiden Bloecken liegen die Stellen, die kein
    /// Blocktest sieht: zwei **Normierungen**, deren Rueckwaertspass von
    /// allen Kanaelen abhaengt, und zwei **Residualadditionen**, die
    /// rueckwaerts Verzweigungen sind.
    #[test]
    fn der_abstand_der_ebene_sinkt() {
        let (mut m, hidden, ziel) = e_aufbau();
        let (cos, sin) = rope_tabellen();
        let sk = e_skalen();
        let v0 = e_vorgaben(&sk, 1, 1, 0);
        let exp = exp_tabelle(v0.aufmerksamkeit.score_frac, v0.aufmerksamkeit.exp_input_frac);
        let rsqrt = rsqrt_tabelle();
        let silu = silu_tabelle(-256, 256, v0.mlp.silu_in_frac, v0.mlp.silu_out_frac);
        let grad = crate::backward::silu_grad_aus_lut(&silu);
        let gam = e_gammas();

        let mut erster = 0i64;
        let mut letzter = 0i64;
        for s in 0..200u64 {
            let a = e_schritt(
                &mut m, &hidden, &ziel, e_vorgaben(&sk, 1 << 13, 1, s),
                &cos, &sin, &exp, &rsqrt, &silu, &grad, &gam,
            );
            if s == 0 {
                erster = a;
            }
            letzter = a;
        }
        assert!(
            letzter * 2 < erster,
            "der Abstand fiel nur von {erster} auf {letzter}, also um weniger als die Haelfte"
        );
    }

    /// Geht auf **einer** Matrix der Ebene entlang ihres Gradienten und
    /// vergleicht gemessene mit vorhergesagter Senkung.
    ///
    /// Gibt `(gemessen, vorhergesagt)` je Matrix zurueck, in der
    /// Reihenfolge Q, K, V, O, Gate, Up, Down.
    ///
    /// # ⚑ Die sieben Exponenten sind **nicht** derselbe
    ///
    /// `dL/dW` entsteht als `Gradient · Eingang` und traegt
    /// `2^(Bus + Eingangsskala)`. Beide unterscheiden sich je Matrix:
    /// Q, K und V lesen den ersten normierten Strom, die
    /// Ausgabeprojektion die Aufmerksamkeitsausgabe, Gate und Up den
    /// zweiten normierten Strom, Down das Produkt. **Wer hier einen
    /// Exponenten fuer alle nimmt, prueft sechs Matrizen gegen die
    /// falsche Erwartung.**
    fn e_schritt_entlang_des_gradienten() -> [(f64, f64); 7] {
        let (m, hidden, ziel) = e_aufbau();
        let (cos, sin) = rope_tabellen();
        let sk = e_skalen();
        let v = e_vorgaben(&sk, 1 << 13, 1, 0);
        let exp = exp_tabelle(v.aufmerksamkeit.score_frac, v.aufmerksamkeit.exp_input_frac);
        let rsqrt = rsqrt_tabelle();
        let silu = silu_tabelle(-256, 256, v.mlp.silu_in_frac, v.mlp.silu_out_frac);
        let grad = crate::backward::silu_grad_aus_lut(&silu);
        let gam = e_gammas();
        let mf = v.aufmerksamkeit.master_frac;
        let qb = A_KOEPFE * A_HD;

        let rechne = |m: &[Vec<Master>]| -> (i64, Ebenengradienten) {
            let (wq, sq) = gewicht_aus_master(&m[0], A_HS, mf);
            let (wk, sk) = gewicht_aus_master(&m[1], A_HS, mf);
            let (wv, sv) = gewicht_aus_master(&m[2], A_HS, mf);
            let (wo, so) = gewicht_aus_master(&m[3], qb, mf);
            let (wg, sg) = gewicht_aus_master(&m[4], A_HS, v.mlp.master_frac);
            let (wu, su) = gewicht_aus_master(&m[5], A_HS, v.mlp.master_frac);
            let (wd, sd) = gewicht_aus_master(&m[6], E_IS, v.mlp.master_frac);
            let gew = Ebenengewichte {
                aufmerksamkeit: Aufmerksamkeitsgewichte {
                    q: &wq, q_skalen: &sq, k: &wk, k_skalen: &sk,
                    v: &wv, v_skalen: &sv, o: &wo, o_skalen: &so,
                },
                gate: &wg, gate_skalen: &sg, up: &wu, up_skalen: &su,
                down: &wd, down_skalen: &sd,
                gamma_ein: &gam.0, gamma_ein_skalen: &gam.1,
                gamma_mitte: &gam.2, gamma_mitte_skalen: &gam.3,
            };
            let t = Ebenentabellen {
                cos: &cos, sin: &sin, exp: &exp, rsqrt: &rsqrt, silu: &silu, silu_grad: &grad,
            };
            let spur = vorwaerts_der_ebene(gew, &hidden, None, t, v);
            gradienten_der_ebene(gew, &spur, &hidden, &ziel, t, v)
        };

        let (l0, gr) = rechne(&m);
        let acc_attn = i32::from(v.acc_attn()[0]);
        let acc_mlp = i32::from(v.acc_mlp()[0]);
        let a_akt = v.aufmerksamkeit.act_frac as i32;
        let m_akt = v.mlp.act_frac as i32;
        let deckel = l0 as f64 / 50.0;

        // ⚑ **Q, K und V tragen den inneren Bus des Blocks, nicht die
        // Additionsskala.** Ihr Gradient kommt aus `linear_backward` mit
        // `attn_out_frac` als eingehender Skala; nur die
        // Ausgabeprojektion sieht die Skala der Residualaddition. ⛑ Mit
        // `acc_attn` an allen vier Stellen lagen die Verhaeltnisse je
        // nach Skalenwahl bei 0,07 oder bei 2,1, und beide Male sah es
        // nach einem Fehler im Code aus.
        let a_bus = v.aufmerksamkeit.attn_out_frac as i32;
        let matrizen: [(&Vec<Grad>, usize, i32, u8); 7] = [
            (&gr.aufmerksamkeit.q, A_HS, a_bus + a_akt, mf),
            (&gr.aufmerksamkeit.k, A_HS, a_bus + a_akt, mf),
            (&gr.aufmerksamkeit.v, A_HS, a_bus + a_akt, mf),
            (&gr.aufmerksamkeit.o, qb, acc_attn + a_bus, mf),
            (&gr.mlp.gate, A_HS, v.mlp.down_in_frac as i32 + m_akt, v.mlp.master_frac),
            (&gr.mlp.up, A_HS, v.mlp.down_in_frac as i32 + m_akt, v.mlp.master_frac),
            (&gr.mlp.down, E_IS, acc_mlp + v.mlp.down_in_frac as i32, v.mlp.master_frac),
        ];

        let mut aus = [(0.0f64, 0.0f64); 7];
        for (n, (grad_m, breite, bus_und_eingang, master_frac)) in matrizen.iter().enumerate() {
            let (_, verschiebungen) = gewicht_aus_master(&m[n], *breite, *master_frac);
            let spitze =
                grad_m.iter().map(|g| i64::from(*g).abs()).max().unwrap_or(0).max(1);
            let vorhersage = |weite: i64| -> (Vec<Master>, f64) {
                let mut neu = m[n].clone();
                let mut vorher = 0.0f64;
                for (i, g) in grad_m.iter().enumerate() {
                    let delta = -(i64::from(*g) * weite / spitze);
                    if delta == 0 {
                        continue;
                    }
                    neu[i] = neu[i].saturating_add(delta as i32);
                    let sz = i32::from(verschiebungen[i / *breite]);
                    let hoch = 2 * i32::from(v.aus_frac[0]) - (bus_und_eingang + 2 * sz);
                    vorher += delta as f64 * f64::from(*g) * 2f64.powi(hoch);
                }
                (neu, vorher)
            };
            let mut gewaehlt = vorhersage(1);
            for weite in [2i64, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
                let kandidat = vorhersage(weite);
                if kandidat.1.abs() > deckel {
                    break;
                }
                gewaehlt = kandidat;
            }
            let mut kopie = m.clone();
            kopie[n] = gewaehlt.0;
            aus[n] = ((rechne(&kopie).0 - l0) as f64, gewaehlt.1);
        }
        aus
    }

    /// ⚑ **Der Gradient der ganzen Ebene sagt die Aenderung des
    /// Abstands voraus, fuer jede der sieben Matrizen.**
    ///
    /// ⛑ **Das ist die Pruefung, die den Zusammenbau traegt.** „Der
    /// Abstand sinkt" belegt hier noch weniger als bei einem einzelnen
    /// Block: Eine Ebene hat sieben Matrizen, und wenn nur fuenf
    /// richtige Gradienten bekommen, faellt der Abstand trotzdem.
    ///
    /// Gemessen liegen die sieben Verhaeltnisse bei 1,17 / 0,83 / 1,26 /
    /// 1,02 / 0,78 / 1,05 / 0,87.
    #[test]
    fn der_gradient_der_ebene_sagt_die_aenderung_voraus() {
        let namen = ["q", "k", "v", "o", "gate", "up", "down"];
        let treffer = e_schritt_entlang_des_gradienten();
        for (n, name) in namen.iter().enumerate() {
            let (gemessen, vorher) = treffer[n];
            assert!(vorher < -1.0, "{name}: der Schritt sagt keine Senkung voraus ({vorher:.1})");
            assert!(
                gemessen < 0.0,
                "{name}: der Abstand stieg um {gemessen:.0}, wo eine Senkung um \
                 {vorher:.0} vorhergesagt war"
            );
            let verhaeltnis = gemessen / vorher;
            assert!(
                (0.65..=1.5).contains(&verhaeltnis),
                "{name}: gemessen {gemessen:.0}, vorhergesagt {vorher:.0}, also das \
                 {verhaeltnis:.2}-fache"
            );
        }
    }

    /// ⚑ **Der Eingangsgradient der Ebene sagt die Aenderung voraus.**
    ///
    /// ⛑ **Ohne diesen Test bleibt eine Gegenprobe gruen**, und zwar
    /// die wichtigste: der **Residualzweig der ersten Addition**. Er
    /// wirkt ausschliesslich auf `dL/dx` der Ebene, nicht auf die
    /// sieben Gewichtsgradienten; wer nur die prueft, sieht seinen
    /// Verlust nicht. **Und genau dieser Zweig ist der Weg, auf dem der
    /// Gradient an der Ebene vorbei nach unten laeuft**, also der
    /// Grund, warum tiefe Netze ueberhaupt trainierbar sind.
    ///
    /// Der Exponent ist ein anderer als bei den Gewichten: `eingang`
    /// traegt `dL/dx` nach dem realen Wert auf `residual_in_frac`, und
    /// ein Schub am ganzzahligen Eingang ist real `d / 2^residual_in`.
    #[test]
    fn der_eingangsgradient_der_ebene_sagt_die_aenderung_voraus() {
        let (m, hidden, ziel) = e_aufbau();
        let (cos, sin) = rope_tabellen();
        let sk = e_skalen();
        let v = e_vorgaben(&sk, 1 << 13, 1, 0);
        let exp = exp_tabelle(v.aufmerksamkeit.score_frac, v.aufmerksamkeit.exp_input_frac);
        let rsqrt = rsqrt_tabelle();
        let silu = silu_tabelle(-256, 256, v.mlp.silu_in_frac, v.mlp.silu_out_frac);
        let grad = crate::backward::silu_grad_aus_lut(&silu);
        let gam = e_gammas();
        let mf = v.aufmerksamkeit.master_frac;
        let qb = A_KOEPFE * A_HD;

        let rechne = |h: &[Vec<i16>]| -> (i64, Ebenengradienten) {
            let (wq, sq) = gewicht_aus_master(&m[0], A_HS, mf);
            let (wk, sk) = gewicht_aus_master(&m[1], A_HS, mf);
            let (wv, sv) = gewicht_aus_master(&m[2], A_HS, mf);
            let (wo, so) = gewicht_aus_master(&m[3], qb, mf);
            let (wg, sg) = gewicht_aus_master(&m[4], A_HS, v.mlp.master_frac);
            let (wu, su) = gewicht_aus_master(&m[5], A_HS, v.mlp.master_frac);
            let (wd, sd) = gewicht_aus_master(&m[6], E_IS, v.mlp.master_frac);
            let gew = Ebenengewichte {
                aufmerksamkeit: Aufmerksamkeitsgewichte {
                    q: &wq, q_skalen: &sq, k: &wk, k_skalen: &sk,
                    v: &wv, v_skalen: &sv, o: &wo, o_skalen: &so,
                },
                gate: &wg, gate_skalen: &sg, up: &wu, up_skalen: &su,
                down: &wd, down_skalen: &sd,
                gamma_ein: &gam.0, gamma_ein_skalen: &gam.1,
                gamma_mitte: &gam.2, gamma_mitte_skalen: &gam.3,
            };
            let t = Ebenentabellen {
                cos: &cos, sin: &sin, exp: &exp, rsqrt: &rsqrt, silu: &silu, silu_grad: &grad,
            };
            let spur = vorwaerts_der_ebene(gew, h, None, t, v);
            gradienten_der_ebene(gew, &spur, h, &ziel, t, v)
        };

        let (l0, gr) = rechne(&hidden);
        assert_eq!(gr.eingang.len(), hidden.len(), "eine Zeile je Position");
        assert!(
            gr.eingang.iter().any(|z| z.iter().any(|g| *g != 0)),
            "der Eingangsgradient der Ebene ist ueberall null"
        );

        let hoch = 2 * (i32::from(v.aus_frac[0]) - i32::from(v.residual_in_frac[0]));
        let spitze = gr
            .eingang
            .iter()
            .flat_map(|z| z.iter())
            .map(|g| i64::from(*g).abs())
            .max()
            .unwrap_or(1)
            .max(1);
        let deckel = l0 as f64 / 50.0;
        let vorhersage = |weite: i64| -> (Vec<Vec<i16>>, f64) {
            let mut neu = hidden.clone();
            let mut vorher = 0.0f64;
            for (t, zeile) in gr.eingang.iter().enumerate() {
                for (i, g) in zeile.iter().enumerate() {
                    let d = -(i64::from(*g) * weite / spitze);
                    if d == 0 {
                        continue;
                    }
                    neu[t][i] = neu[t][i].saturating_add(d as i16);
                    vorher += d as f64 * f64::from(*g) * 2f64.powi(hoch);
                }
            }
            (neu, vorher)
        };
        // ⚑ **Zwei ist die kleinste Weite, die etwas misst.** Der
        // Schritt ist auf die Spitze normiert; bei Weite eins bewegt
        // sich **genau ein** Gewicht, und eine einzelne ganzzahlige
        // Aenderung ist Raster und keine Messung. ⛑ Gemessen: bei
        // Weite eins standen 3 326 vorhergesagt gegen 114 gemessen, bei
        // Weite zwei 22 124 gegen 21 574.
        let mut gewaehlt = vorhersage(2);
        for weite in [4i64, 8, 16, 32, 64, 128, 256] {
            let kandidat = vorhersage(weite);
            if kandidat.1.abs() > deckel {
                break;
            }
            gewaehlt = kandidat;
        }
        let gemessen = (rechne(&gewaehlt.0).0 - l0) as f64;
        pruefe_vorhersage("Ebenen-Eingang", gemessen, gewaehlt.1);
    }

    /// ⚑ **Mit zwei Nullmatrizen ist die Ebene ein Durchreicher, und
    /// dann steht der Residualweg exakt da.**
    ///
    /// Sind `o_proj` und `down_proj` null, liefern beide Bloecke die
    /// Ausgabe null. Vorwaerts bleibt `out = hidden`, nur umskaliert;
    /// rueckwaerts bleibt `dL/dx = dL/dy`, ebenfalls nur umskaliert.
    /// **Beide Seiten lassen sich hinschreiben**, und der Vergleich ist
    /// eine Gleichheit statt eines Verhaeltnisses.
    ///
    /// ⛑ **Das ist die Pruefung, die der statistische Vergleich nicht
    /// leistet.** Der Residualzweig der **ersten** Addition wirkt nur
    /// auf `dL/dx`, und dort geht er neben dem Beitrag der Normierung
    /// unter: Gemessen aenderte seine Wegnahme das Verhaeltnis von 0,872
    /// auf 0,886, also gar nichts. **Ein Zweig, der im Rauschen liegt,
    /// braucht eine Gleichung und keine Schranke.**
    ///
    /// ⚑ **Und der Weg ist der Grund, warum tiefe Netze trainierbar
    /// sind:** Er fuehrt den Gradienten an der Ebene **vorbei** nach
    /// unten, ungedaempft durch Normierung und Bloecke.
    #[test]
    fn mit_zwei_nullmatrizen_reicht_die_ebene_durch() {
        let (mut m, hidden, ziel) = e_aufbau();
        // o_proj und down_proj auf null: beide Bloecke geben null aus.
        m[3] = vec![0; m[3].len()];
        m[6] = vec![0; m[6].len()];
        let (cos, sin) = rope_tabellen();
        let sk = e_skalen();
        let v = e_vorgaben(&sk, 1 << 13, 1, 0);
        let exp = exp_tabelle(v.aufmerksamkeit.score_frac, v.aufmerksamkeit.exp_input_frac);
        let rsqrt = rsqrt_tabelle();
        let silu = silu_tabelle(-256, 256, v.mlp.silu_in_frac, v.mlp.silu_out_frac);
        let grad = crate::backward::silu_grad_aus_lut(&silu);
        let gam = e_gammas();
        let mf = v.aufmerksamkeit.master_frac;
        let qb = A_KOEPFE * A_HD;

        let (wq, sq) = gewicht_aus_master(&m[0], A_HS, mf);
        let (wk, sk) = gewicht_aus_master(&m[1], A_HS, mf);
        let (wv, sv) = gewicht_aus_master(&m[2], A_HS, mf);
        let (wo, so) = gewicht_aus_master(&m[3], qb, mf);
        let (wg, sg) = gewicht_aus_master(&m[4], A_HS, v.mlp.master_frac);
        let (wu, su) = gewicht_aus_master(&m[5], A_HS, v.mlp.master_frac);
        let (wd, sd) = gewicht_aus_master(&m[6], E_IS, v.mlp.master_frac);
        let gew = Ebenengewichte {
            aufmerksamkeit: Aufmerksamkeitsgewichte {
                q: &wq, q_skalen: &sq, k: &wk, k_skalen: &sk,
                v: &wv, v_skalen: &sv, o: &wo, o_skalen: &so,
            },
            gate: &wg, gate_skalen: &sg, up: &wu, up_skalen: &su,
            down: &wd, down_skalen: &sd,
            gamma_ein: &gam.0, gamma_ein_skalen: &gam.1,
            gamma_mitte: &gam.2, gamma_mitte_skalen: &gam.3,
        };
        let t = Ebenentabellen {
            cos: &cos, sin: &sin, exp: &exp, rsqrt: &rsqrt, silu: &silu, silu_grad: &grad,
        };
        let spur = vorwaerts_der_ebene(gew, &hidden, None, t, v);

        // Vorwaerts: die Ebene reicht durch.
        let acc_attn = v.acc_attn();
        let acc_mlp = v.acc_mlp();
        for (nr, h) in hidden.iter().enumerate() {
            for (i, hi) in h.iter().enumerate() {
                let r = rescale_i64(i64::from(*hi), v.residual_in_frac[i], acc_attn[i]);
                let mitte =
                    clamp_i16_von_i64(rescale_i64(r, acc_attn[i], v.residual_mid_frac[i]));
                let r2 = rescale_i64(i64::from(mitte), v.residual_mid_frac[i], acc_mlp[i]);
                let soll = clamp_i16_von_i64(rescale_i64(r2, acc_mlp[i], v.aus_frac[i]));
                assert_eq!(
                    spur.y[nr][i], soll,
                    "Position {nr}, Kanal {i}: die Ebene reicht nicht durch"
                );
            }
        }

        let (_, gr) = gradienten_der_ebene(gew, &spur, &hidden, &ziel, t, v);

        // Rueckwaerts: der Gradient reicht ebenso durch.
        for (nr, zt) in ziel.iter().enumerate() {
            for (i, zi) in zt.iter().enumerate() {
                let g_y = 2 * (i64::from(spur.y[nr][i]) - i64::from(*zi));
                let ueber_mitte = rescale_i64(g_y, v.aus_frac[i], v.residual_mid_frac[i]);
                let soll =
                    begrenze(rescale_i64(ueber_mitte, v.residual_mid_frac[i], v.residual_in_frac[i]));
                assert_eq!(
                    gr.eingang[nr][i], soll,
                    "Position {nr}, Kanal {i}: der Gradient reicht nicht durch. \
                     Fehlt ein Zweig der Residualaddition, steht hier null"
                );
            }
        }
        // ⛑ Und die Erwartung ist nicht selbst null.
        assert!(
            gr.eingang.iter().any(|z| z.iter().any(|g| *g != 0)),
            "der durchgereichte Gradient ist ueberall null, der Test misst nichts"
        );
    }

    /// ⚑ **Zweimal gerechnet ist zweimal dasselbe**, Gradient fuer    /// ⚑ **Zweimal gerechnet ist zweimal dasselbe**, Gradient fuer
    /// Gradient. Der Rueckwaertspass sammelt Beitraege ueber Koepfe und
    /// Positionen; jede Summe ist eine ganzzahlige Addition und damit
    /// ordnungsfrei. Ohne diese Zusicherung waere jede Aufteilung der
    /// Arbeit auf mehrere Rechner offen.
    #[test]
    fn zwei_laeufe_liefern_denselben_gradienten() {
        let (q, k, v, o, x, ziel) = a_aufbau();
        let (cos, sin) = rope_tabellen();
        let vg = a_vorgaben(1 << 11, 1, 0);
        let exp = exp_tabelle(vg.score_frac, vg.exp_input_frac);
        let lauf =
            || gradienten_der_aufmerksamkeit(&q, &k, &v, &o, &x, &ziel, None, &cos, &sin, &exp, vg);
        assert_eq!(lauf(), lauf());
    }

    /// Je Zeile eine eigene Skala, sonst faellt die kleine auf null.
    #[test]
    fn zwei_zeilen_bekommen_zwei_skalen() {
        let master = vec![
            1_000_000, 2_000_000, // grosse Zeile
            3, 4, // kleine Zeile
        ];
        let (w, s) = gewicht_aus_master(&master, 2, 20);
        assert_ne!(s[0], s[1], "beide Zeilen bekamen dieselbe Skala");
        // ⚑ Die kleine Zeile ueberlebt: Mit gemeinsamer Skala waere sie
        // null.
        assert!(w[2] != 0 || w[3] != 0, "die kleine Zeile fiel auf null");
    }

    /// ⚑ **Über die Oktavgrenze hinweg bleibt der reale Wert stetig
    /// (Fund 174, behoben).**
    ///
    /// ⛑ **Dieser Test hielt bis zum 2026-09-04 einen Mangel fest** und
    /// verlangte ausdrücklich, dass er beim Beheben rot wird. Genau das
    /// ist geschehen; jetzt hält er die Eigenschaft, die vorher fehlte.
    ///
    /// **Der Mangel war:** In `shifts` stand `s` statt `master_frac − s`,
    /// und damit war der reale Wert `master / 2^(2·s)`. Ein Master, der
    /// um eins wuchs, liess das Gewicht der ganzen Zeile auf ein Viertel
    /// fallen, sobald ihr Betragsmaximum eine Zweierpotenz überschritt:
    /// 127 gab 127,00, **128 gab 32,00**.
    ///
    /// **Geprüft wird beides:** dass der Wert innerhalb einer Oktave
    /// mitwächst, und dass er über die Grenze hinweg **nicht springt**.
    /// Die zweite Hälfte ist die eigentliche Aussage; die erste allein
    /// hatte die alte Fassung auch bestanden.
    #[test]
    fn ueber_der_oktavgrenze_bleibt_der_reale_wert_stetig() {
        const M: u8 = 20;
        let real = |m: i32| -> f64 {
            let (w, s) = gewicht_aus_master(&[m, m / 2, -m / 4, 3], 4, M);
            f64::from(w[0]) / f64::from(1u32 << s[0])
        };
        // Der reale Wert eines Masters ist `master / 2^M`, unabhaengig
        // davon, wie viele Stellen zum Hineinpassen in `i8` wegfallen.
        let soll = |m: i32| f64::from(m) / f64::from(1u32 << M);

        // Innerhalb der Oktave waechst er mit dem Master.
        assert!(real(200) > real(150), "innerhalb der Oktave ist es nicht monoton");

        // ⚑ **Und ueber jede Oktavgrenze hinweg auch.** 127 nach 128 ist
        // die Stelle, an der die alte Fassung auf ein Viertel fiel.
        for grenze in [128i32, 256, 512, 8192, 1 << 20] {
            let (unten, oben) = (real(grenze - 1), real(grenze));
            assert!(
                oben > unten,
                "an der Grenze {grenze} faellt der reale Wert: {unten} -> {oben}"
            );
            for m in [grenze - 1, grenze] {
                let abweichung = (real(m) - soll(m)).abs();
                assert!(
                    abweichung <= soll(m).abs() / 100.0 + f64::from(1u32 << M).recip(),
                    "Master {m}: realer Wert {} statt {}",
                    real(m),
                    soll(m)
                );
            }
        }
    }

    /// ⚑ **Ein Gewicht, das die Uebertragungsform sprengt, bricht ab.**
    ///
    /// Der reale Wert ist `master / 2^master_frac`; ueberschreitet er
    /// 127, laesst er sich als `i8` mit einer Zeilenverschiebung nicht
    /// mehr ausdruecken. ⛑ **Stille Saettigung waere hier das Falsche:**
    /// Sie fiele erst an der Verlustkurve auf, und dort sieht sie aus
    /// wie ein Trainingsproblem statt wie ein Darstellungsproblem.
    #[test]
    #[should_panic(expected = "Der reale Wert ueberschreitet")]
    fn ein_zu_grosses_gewicht_bricht_ab() {
        // Betrag 2^20 bei vier Bruchstellen: realer Wert 65 536.
        let _ = gewicht_aus_master(&[1 << 20, 0, 0, 0], 4, 4);
    }

    /// ⚑ **Eine Nullzeile bekommt keine erfundene Skala.**
    #[test]
    fn eine_nullzeile_bleibt_null() {
        let master = vec![0, 0, 0, 5];
        let (w, s) = gewicht_aus_master(&master, 2, 12);
        // ⚑ **Die Skala des Masters, nicht null.** Aus Nullen wird bei
        // jeder Verschiebung wieder null; die ehrliche Angabe ist die,
        // auf der der Master steht.
        assert_eq!(s[0], 12, "eine Nullzeile bekommt die Skala des Masters");
        assert_eq!(&w[..2], &[0, 0]);
    }

    /// Der groesste Betrag passt nach der Verschiebung in `i8`.
    #[test]
    fn nach_der_verschiebung_passt_alles_in_i8() {
        for gross in [127i32, 128, 1_000, i32::MAX / 2] {
            let master = vec![gross, -gross, 1, 0];
            let (w, s) = gewicht_aus_master(&master, 4, 31);
            assert!(
                w.iter().all(|v| (-127..=127).contains(&i32::from(*v))),
                "Verschiebung {} reichte fuer {gross} nicht",
                s[0]
            );
        }
    }
}

// ===========================================================================
// Das Expertengemisch (TRAINING V, Schritt 2f)
// ===========================================================================

/// Was ein Rückwärtspass durch ein Expertengemisch zurückgibt.
///
/// # ⚑ Warum die Expertengradienten dünn besetzt sind
///
/// Je Position rechnen **k von n** Experten, bei Qwen3-30B-A3B acht von
/// hundertachtundzwanzig. Ein Gradient über alle Experten wäre zu
/// 93,75 Prozent null und kostete bei 128 Experten je Ebene rund
/// 604 Millionen Werte. **Deshalb steht hier je gewähltem Experten ein
/// Eintrag**, und der Aufrufer schreibt genau diese fort.
///
/// ⚑ **Und das ist keine Optimierung, sondern der Kern der Sache.** Ein
/// Expertengemisch ist genau deshalb billig, weil ein Token nur k
/// Experten berührt; ein Trainingsschritt, der alle anfasst, hat den
/// Vorteil weggeworfen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Gemischgradienten {
    /// Je **gewähltem** Experten, in Auswahlreihenfolge: sein Index und
    /// die Gradienten seiner drei Matrizen.
    pub experten: Vec<(u16, Mlpgradienten)>,
    /// `dL/dW` der Routerprojektion, in Zeilen zu `hidden_size`.
    pub router: Vec<Grad>,
    /// `dL/dx` nach dem Eingang des Blocks, auf `act_frac`.
    ///
    /// ⚑ **Die Summe über alle gewählten Experten und den Router.**
    /// Jeder von ihnen liest denselben Eingang; wer nur einen Beitrag
    /// weiterreicht, verliert den Rest, und der Fehler wächst mit k.
    pub eingang: Vec<Grad>,
}

/// Was der Rückwärtspass über das Gemisch braucht, zusätzlich zu
/// [`Mlpvorgaben`].
#[derive(Debug, Clone, Copy)]
pub struct Gemischvorgaben {
    /// Wie viele Experten die Ebene hat.
    pub anzahl_experten: usize,
    /// Bruchstellen der Mischgewichte.
    pub gewicht_frac: u8,
    // ⛑ **Hier stand `router_frac`, und genau das war der Fehler.**
    // Die Skala der Routerlogits wird im Rückwärtspass nicht gebraucht:
    // Der Logitgradient trägt seine Skala aus `moe_backward`
    // (`aus_frac + logit_zusatz_bits`), und `dL/dx` kommt auf der Skala
    // des Eingangs heraus. Ein Feld, das dasteht und nicht gebraucht
    // wird, findet den Weg in den falschen Steckplatz; dieses hat ihn
    // gefunden. Weg damit.
    /// Zusätzliche Bruchstellen des Logitgradienten.
    ///
    /// # ⚑ Wozu, und warum die Zahl nicht null sein darf (Fund 79)
    ///
    /// Der Routergradient trägt den Faktor `p_i · (1 − p_i)`; je
    /// entschiedener die Wahl, desto kleiner. Auf der Skala des
    /// Aktivierungsgradienten rundet er auf null, **bevor er wirkt**.
    /// Diese Bits führen ihn feiner.
    ///
    /// ⚑ **Gegen die Sättigung hilft das nicht**, nur gegen die
    /// Rundung. Die Sättigung ist ein absorbierender Zustand und
    /// braucht eine andere Antwort; siehe
    /// `runtime/src/bin/router_saettigung.rs`.
    pub logit_zusatz_bits: u8,
}

/// Der Rückwärtspass durch ein **Expertengemisch**.
///
/// # ⚑ Der Weg, und er hat drei Zweige statt einem
///
/// Ein dichter Block hat einen Weg vom Ausgang zum Eingang. Hier sind es
/// drei, und sie treffen sich am Eingang wieder:
///
/// 1. **Durch die Mischung** in jeden gewählten Experten
///    ([`crate::backward::moe_backward`]), und von dort durch dessen
///    dichten Block wie bei jeder anderen Ebene.
/// 2. **Durch die Mischgewichte** in die Routerlogits, denselben Weg
///    zurück durch den Softmax.
/// 3. **Vom Logitgradienten** durch die Routerprojektion in den
///    Eingang.
///
/// ⚑ **Alle drei lesen denselben `x`**, und ihre Beiträge zu `dL/dx`
/// addieren sich. Wer einen davon vergisst, bekommt einen Gradienten,
/// der plausibel aussieht und um einen ganzen Zweig danebenliegt: genau
/// die Klasse von Fund 173 und 175.
#[allow(clippy::too_many_arguments)]
pub fn gradienten_des_gemisches(
    g: &[Grad],
    x: &[i16],
    experten: &[u16],
    gewichte: &[i32],
    ausgaben: &[Vec<i16>],
    spuren: &[Mlpspur],
    expertengewichte: &[Expertengewichte<'_>],
    router: &[i8],
    router_skalen: &[u8],
    silu_lut: &[i16],
    grad_lut: &[i16],
    v: Mlpvorgaben,
    gv: Gemischvorgaben,
) -> Gemischgradienten {
    assert_eq!(gewichte.len(), experten.len(), "gradienten_des_gemisches: Gewichte je Experte");
    assert_eq!(ausgaben.len(), experten.len(), "gradienten_des_gemisches: Ausgaben je Experte");
    assert_eq!(spuren.len(), experten.len(), "gradienten_des_gemisches: Spuren je Experte");
    assert_eq!(
        expertengewichte.len(),
        experten.len(),
        "gradienten_des_gemisches: Gewichte je Experte"
    );

    // 1. Durch die Mischung.
    let gemisch = crate::backward::moe_backward(
        g,
        experten,
        gewichte,
        ausgaben,
        gv.anzahl_experten,
        gv.gewicht_frac,
        v.aus_frac,
        gv.logit_zusatz_bits,
    );

    // 2. Je gewähltem Experten durch seinen dichten Block.
    let mut eingang = vec![0i32; v.hidden_size];
    let mut je_experte: Vec<(u16, Mlpgradienten)> = Vec::with_capacity(experten.len());
    for (n, e) in experten.iter().enumerate() {
        let w = &expertengewichte[n];
        let gr = gradienten_des_mlp_aus_gradient(
            &gemisch.je_ausgabe[n],
            x,
            &spuren[n],
            w.gate,
            w.gate_skalen,
            w.up,
            w.up_skalen,
            w.down,
            w.down_skalen,
            silu_lut,
            grad_lut,
            v,
        );
        for (ziel, teil) in eingang.iter_mut().zip(gr.eingang.iter()) {
            *ziel = ziel.saturating_add(*teil);
        }
        je_experte.push((*e, gr));
    }

    // 3. Vom Logitgradienten durch die Routerprojektion.
    //
    // ⚑ **Die Skala des Logitgradienten ist `aus_frac +
    // logit_zusatz_bits`**, siehe `moe_backward`. Wer hier `aus_frac`
    // einsetzt, rechnet um `2^logit_zusatz_bits` daneben, und das sieht
    // aus wie ein zu starker Router. Dieselbe Falle wie Fund 173.
    // ⚑ **`gx_frac` ist `act_frac`, die Skala des EINGANGS.** Hier stand
    // am 2026-09-05 `router_frac`, die Skala des Routerausgangs, und der
    // Eingangsgradient kam um den Faktor `2^(router_frac − act_frac)`
    // zu gross heraus: gemessen exakt achtfach. **Dieselbe Klasse wie
    // Fund 173 und 175**, und derselbe Test hat sie gefunden, nämlich
    // der gegen die geschlossene Form. Der Abstandstest blieb grün.
    let (g_x_router, gw_router) = crate::backward::linear_backward(
        &gemisch.logits,
        x,
        router,
        v.hidden_size,
        router_skalen,
        v.aus_frac + gv.logit_zusatz_bits,
        v.act_frac,
    );
    for (ziel, teil) in eingang.iter_mut().zip(g_x_router.iter()) {
        *ziel = ziel.saturating_add(*teil);
    }

    Gemischgradienten { experten: je_experte, router: nach_grad(&gw_router), eingang }
}

/// Die Gewichte **eines** Experten.
#[derive(Debug, Clone, Copy)]
pub struct Expertengewichte<'a> {
    /// Gate-Projektion.
    pub gate: &'a [i8],
    /// Zeilenskalen der Gate-Projektion.
    pub gate_skalen: &'a [u8],
    /// Up-Projektion.
    pub up: &'a [i8],
    /// Zeilenskalen der Up-Projektion.
    pub up_skalen: &'a [u8],
    /// Down-Projektion.
    pub down: &'a [i8],
    /// Zeilenskalen der Down-Projektion.
    pub down_skalen: &'a [u8],
}

#[cfg(test)]
mod gemisch_tests {
    use super::*;
    use crate::moe::{mische_experten, route_top_k};
    use crate::optimierer::MASTER_FRAC;

    /// # ⚑ Warum der Aufbau nicht winzig ist
    ///
    /// Der erste Entwurf nahm `HS = 6`, `IS = 8`, `N = 4`. Gegen die
    /// geschlossene Form lag **schon der dichte Block allein** um den
    /// Faktor 0,6 bis 2,4 daneben, bei durchweg richtigen Vorzeichen.
    /// Das ist kein Fehler, sondern die Auflösung: Der Rückwärtspass
    /// summiert acht Produkte und rundet dabei, und bei acht Summanden
    /// mittelt sich nichts heraus.
    ///
    /// ⚑ **Ein Test, der Quantisierungsrauschen nicht von einem Fehler
    /// trennen kann, prüft nichts.**
    const N: usize = 8;
    const K: usize = 2;
    const HS: usize = 48;
    const IS: usize = 64;
    const ACT: u8 = 8;
    const AUS: u8 = 8;
    const ROUTER_FRAC: u8 = 11;
    const EXP_IN: u8 = 8;
    const GEW: u8 = 14;

    fn exp_lut() -> Vec<i16> {
        (0..4096)
            .map(|i| ((-(i as f64) / f64::from(1u32 << EXP_IN)).exp() * 16384.0).round() as i16)
            .collect()
    }
    fn silu_lut() -> Vec<i16> {
        (0..16384)
            .map(|i| {
                let x = (i as f64 - 8192.0) / 256.0;
                ((x / (1.0 + (-x).exp())) * 256.0).round().clamp(-32768.0, 32767.0) as i16
            })
            .collect()
    }

    /// Eine erzeugte, aber feste Zahlenfolge.
    fn zahl(i: usize, spanne: i64) -> i64 {
        let mut z = (i as u64).wrapping_mul(0x9E3779B97F4A7C15);
        z ^= z >> 29;
        z = z.wrapping_mul(0xBF58476D1CE4E5B9);
        z ^= z >> 32;
        (z % (2 * spanne as u64 + 1)) as i64 - spanne
    }

    struct Aufbau {
        gate: Vec<Vec<Master>>,
        up: Vec<Vec<Master>>,
        down: Vec<Vec<Master>>,
        router: Vec<Master>,
    }

    fn aufbau() -> Aufbau {
        let mf = MASTER_FRAC;
        let m = |n: usize, versatz: usize| -> Vec<Master> {
            (0..n).map(|i| (zahl(i + versatz, 90) << (mf - 7)) as i32).collect()
        };
        Aufbau {
            gate: (0..N).map(|e| m(IS * HS, 1000 + e * 7919)).collect(),
            up: (0..N).map(|e| m(IS * HS, 20000 + e * 6113)).collect(),
            down: (0..N).map(|e| m(HS * IS, 40000 + e * 5051)).collect(),
            router: m(N * HS, 90000),
        }
    }

    fn vorgaben() -> Mlpvorgaben {
        Mlpvorgaben {
            hidden_size: HS,
            intermediate_size: IS,
            act_frac: ACT,
            gate_frac: 8,
            up_frac: 8,
            down_in_frac: 8,
            aus_frac: AUS,
            master_frac: MASTER_FRAC,
            silu_in_frac: EXP_IN,
            silu_lut_offset: 8192,
            silu_out_frac: 8,
            lr_zaehler: 1,
            // ⚑ **Gemessen, nicht geraten** (2026-09-05), 60 Schritte,
            // von 869.082.771 aus:
            //
            // | Nenner | Abstand danach |
            // |---|---|
            // | 2^12 | 4.325.869 |
            // | 2^10 | **1.378.445** |
            // | 2^8 | 832.650 |
            // | 2^6 | 151.254 |
            // | 4 | bricht ab: die Gewichte laufen aus dem Master |
            //
            // ⚑ **Die letzte Zeile ist die interessante.** Eine zu grosse
            // Schrittweite scheitert hier nicht leise, sondern laut:
            // `gewicht_aus_master` meldet, dass der reale Wert 127
            // überschreitet. Das ist die Zusicherung aus Fund 174 bei
            // der Arbeit.
            lr_nenner: 1 << 10,
            kennung: Schrittkennung { ebene: 1, schritt: 0, index_versatz: 0 },
        }
    }

    fn gv() -> Gemischvorgaben {
        Gemischvorgaben {
            anzahl_experten: N,
            gewicht_frac: GEW,
            logit_zusatz_bits: 6,
        }
    }

    /// Der Vorwärtspass des Gemisches, so wie die Laufzeit ihn rechnet.
    #[allow(clippy::type_complexity)]
    fn vorwaerts(
        a: &Aufbau,
        x: &[i16],
        lut: &[i16],
        silu: &[i16],
    ) -> (Vec<i16>, Vec<u16>, Vec<i32>, Vec<Vec<i16>>, Vec<Mlpspur>, Vec<i32>) {
        let v = vorgaben();
        let (rw, rs) = gewicht_aus_master(&a.router, HS, MASTER_FRAC);
        let logits: Vec<i32> = crate::linear::linear_w8a16(x, &rw, HS, &rs, ACT, ROUTER_FRAC)
            .iter()
            .map(|l| i32::from(*l))
            .collect();
        let routing = route_top_k(&logits, K, lut, ROUTER_FRAC - EXP_IN, GEW, true);

        let mut ausgaben = Vec::new();
        let mut spuren = Vec::new();
        for e in &routing.experten {
            let i = *e as usize;
            let (g, gs) = gewicht_aus_master(&a.gate[i], HS, MASTER_FRAC);
            let (u, us) = gewicht_aus_master(&a.up[i], HS, MASTER_FRAC);
            let (d, ds) = gewicht_aus_master(&a.down[i], IS, MASTER_FRAC);
            let mut sp = Mlpspur::default();
            let aus_frac = vec![v.aus_frac; HS];
            let aus = crate::mlp::mlp_int_mit_spur(
                x, &g, &u, &d, HS, IS, &gs, &us, &ds, silu, ACT, v.gate_frac, v.up_frac,
                v.down_in_frac, v.silu_in_frac, v.silu_lut_offset, v.silu_out_frac,
                &aus_frac, Some(&mut sp),
            );
            ausgaben.push(aus);
            spuren.push(sp);
        }
        let y = mische_experten(&ausgaben, &routing.gewichte, GEW);
        (y, routing.experten, routing.gewichte, ausgaben, spuren, logits)
    }

    /// Die sechs Felder eines Experten in Uebertragungsform.
    type Satz = (Vec<i8>, Vec<u8>, Vec<i8>, Vec<u8>, Vec<i8>, Vec<u8>);

    fn gewichte_von<'a>(
        a: &'a Aufbau,
        experten: &[u16],
        halde: &'a mut Vec<Satz>,
    ) -> Vec<Expertengewichte<'a>> {
        for e in experten {
            let i = *e as usize;
            let (g, gs) = gewicht_aus_master(&a.gate[i], HS, MASTER_FRAC);
            let (u, us) = gewicht_aus_master(&a.up[i], HS, MASTER_FRAC);
            let (d, ds) = gewicht_aus_master(&a.down[i], IS, MASTER_FRAC);
            halde.push((g, gs, u, us, d, ds));
        }
        halde
            .iter()
            .map(|(g, gs, u, us, d, ds)| Expertengewichte {
                gate: g,
                gate_skalen: gs,
                up: u,
                up_skalen: us,
                down: d,
                down_skalen: ds,
            })
            .collect()
    }

    /// ⚑ **Der Kreis schliesst sich über ein Expertengemisch: Der
    /// Abstand sinkt.**
    ///
    /// Dieselbe Aussage wie für den dichten Block, und sie ist die
    /// einzige, die kein einzelner Golden-Vektor treffen kann: Jeder
    /// Kern kann für sich richtig sein und die Kette trotzdem in die
    /// falsche Richtung laufen, wenn eine Skala nicht passt.
    #[test]
    fn der_abstand_sinkt_ueber_das_gemisch() {
        let lut = exp_lut();
        let silu = silu_lut();
        let mut a = aufbau();
        let x: Vec<i16> = (0..HS).map(|i| (zahl(i + 5, 400)) as i16).collect();
        let ziel: Vec<i16> = (0..HS).map(|i| (zahl(i + 77, 300)) as i16).collect();

        let abstand = |a: &Aufbau| -> i64 {
            let (y, ..) = vorwaerts(a, &x, &lut, &silu);
            y.iter()
                .zip(ziel.iter())
                .map(|(p, z)| {
                    let d = i64::from(*p) - i64::from(*z);
                    d * d
                })
                .sum()
        };
        let vorher = abstand(&a);

        for s in 0..60u64 {
            let mut v = vorgaben();
            v.kennung.schritt = s;
            let (y, experten, gewichte, ausgaben, spuren, _logits) =
                vorwaerts(&a, &x, &lut, &silu);
            // dL/dy des quadratischen Abstands: 2 (y − ziel).
            let g: Vec<Grad> = y
                .iter()
                .zip(ziel.iter())
                .map(|(p, z)| 2 * (i32::from(*p) - i32::from(*z)))
                .collect();
            let (rw, rs) = gewicht_aus_master(&a.router, HS, MASTER_FRAC);
            let mut halde = Vec::new();
            let ew = gewichte_von(&a, &experten, &mut halde);
            let gr = gradienten_des_gemisches(
                &g, &x, &experten, &gewichte, &ausgaben, &spuren, &ew, &rw, &rs, &silu,
                &crate::backward::silu_grad_aus_lut(&silu), v, gv(),
            );
            let mut versatz = 0u64;
            for (e, mg) in &gr.experten {
                let i = *e as usize;
                let k = Schrittkennung { index_versatz: versatz, ..v.kennung };
                schritt(&mut a.gate[i], &mg.gate, k, v.lr_zaehler, v.lr_nenner);
                versatz += a.gate[i].len() as u64;
                let k = Schrittkennung { index_versatz: versatz, ..v.kennung };
                schritt(&mut a.up[i], &mg.up, k, v.lr_zaehler, v.lr_nenner);
                versatz += a.up[i].len() as u64;
                let k = Schrittkennung { index_versatz: versatz, ..v.kennung };
                schritt(&mut a.down[i], &mg.down, k, v.lr_zaehler, v.lr_nenner);
                versatz += a.down[i].len() as u64;
            }
            let k = Schrittkennung { index_versatz: versatz, ..v.kennung };
            schritt(&mut a.router, &gr.router, k, v.lr_zaehler, v.lr_nenner);
        }
        let nachher = abstand(&a);
        eprintln!("  Gemisch: Abstand {vorher} -> {nachher}");
        // ⚑ **Zwei Grössenordnungen**, gegen gemessene 947 von 253.109.
        // Die Schranke prüft die Aussage, nicht die Nachkommastelle.
        assert!(
            nachher * 100 < vorher,
            "der Abstand fiel nicht deutlich: {vorher} auf {nachher}"
        );
    }

    /// ⚑ **Drei Zweige treffen sich am Eingang, und jeder trägt bei.**
    ///
    /// Die Gegenprobe zu Fund 173 und 175 an dieser Stelle: Ein
    /// Gradient, dem ein ganzer Zweig fehlt, sieht plausibel aus.
    #[test]
    fn der_routerzweig_traegt_zum_eingangsgradienten_bei() {
        let lut = exp_lut();
        let silu = silu_lut();
        let a = aufbau();
        let x: Vec<i16> = (0..HS).map(|i| (zahl(i + 5, 400)) as i16).collect();
        let (y, experten, gewichte, ausgaben, spuren, _l) = vorwaerts(&a, &x, &lut, &silu);
        let g: Vec<Grad> = y.iter().map(|p| 2 * i32::from(*p)).collect();
        let (rw, rs) = gewicht_aus_master(&a.router, HS, MASTER_FRAC);
        let mut halde = Vec::new();
        let ew = gewichte_von(&a, &experten, &mut halde);
        let grad_lut = crate::backward::silu_grad_aus_lut(&silu);
        let mit = gradienten_des_gemisches(
            &g, &x, &experten, &gewichte, &ausgaben, &spuren, &ew, &rw, &rs, &silu, &grad_lut,
            vorgaben(), gv(),
        );
        // Ohne Router: ein Nullgewicht an derselben Stelle.
        let rw_null = vec![0i8; rw.len()];
        let ohne = gradienten_des_gemisches(
            &g, &x, &experten, &gewichte, &ausgaben, &spuren, &ew, &rw_null, &rs, &silu,
            &grad_lut, vorgaben(), gv(),
        );
        assert_ne!(
            mit.eingang, ohne.eingang,
            "der Routerzweig traegt nichts zum Eingangsgradienten bei"
        );
        assert!(
            mit.router.iter().any(|v| *v != 0),
            "der Routergradient ist ueberall null"
        );
    }

    /// Der Vorwärtspass **in Gleitkomma**, mit denselben Gewichten und
    /// derselben Expertenwahl: die geschlossene Form, die der
    /// Ganzzahlpfad annähert.
    ///
    /// ⚑ **Die Auswahl kommt von aussen und wird nicht neu getroffen.**
    /// Sie ist eine Stufenfunktion; innerhalb eines Stücks ist die
    /// Abbildung glatt, und nur darin gibt es eine Ableitung.
    fn vorwaerts_f64(a: &Aufbau, x: &[f64], experten: &[u16]) -> Vec<f64> {
        let echt = |m: &[Master], breite: usize| -> Vec<Vec<f64>> {
            let (w, sh) = gewicht_aus_master(m, breite, MASTER_FRAC);
            w.chunks(breite)
                .enumerate()
                .map(|(z, zeile)| {
                    zeile.iter().map(|v| f64::from(*v) / 2f64.powi(i32::from(sh[z]))).collect()
                })
                .collect()
        };
        let mal = |w: &[Vec<f64>], v: &[f64]| -> Vec<f64> {
            w.iter().map(|z| z.iter().zip(v).map(|(a, b)| a * b).sum()).collect()
        };

        // Router und Mischgewichte: Softmax über die GEWÄHLTEN.
        let logits = mal(&echt(&a.router, HS), x);
        let gewaehlt: Vec<f64> = experten.iter().map(|e| logits[*e as usize]).collect();
        let m = gewaehlt.iter().cloned().fold(f64::MIN, f64::max);
        let exps: Vec<f64> = gewaehlt.iter().map(|z| (z - m).exp()).collect();
        let summe: f64 = exps.iter().sum();
        let w: Vec<f64> = exps.iter().map(|e| e / summe).collect();

        let mut y = vec![0.0f64; HS];
        for (n, e) in experten.iter().enumerate() {
            let i = *e as usize;
            let gate = mal(&echt(&a.gate[i], HS), x);
            let up = mal(&echt(&a.up[i], HS), x);
            let h: Vec<f64> =
                gate.iter().zip(up.iter()).map(|(g, u)| (g / (1.0 + (-g).exp())) * u).collect();
            let aus = mal(&echt(&a.down[i], IS), &h);
            for (ziel, v) in y.iter_mut().zip(aus.iter()) {
                *ziel += w[n] * v;
            }
        }
        y
    }

    /// Wie [`vorwaerts_f64`], aber die Mischgewichte sind festgehalten.
    fn vorwaerts_f64_feste_gewichte(
        a: &Aufbau,
        x: &[f64],
        experten: &[u16],
        w: &[f64],
    ) -> Vec<f64> {
        let echt = |m: &[Master], breite: usize| -> Vec<Vec<f64>> {
            let (ww, sh) = gewicht_aus_master(m, breite, MASTER_FRAC);
            ww.chunks(breite)
                .enumerate()
                .map(|(z, zeile)| {
                    zeile.iter().map(|v| f64::from(*v) / 2f64.powi(i32::from(sh[z]))).collect()
                })
                .collect()
        };
        let mal = |w: &[Vec<f64>], v: &[f64]| -> Vec<f64> {
            w.iter().map(|z| z.iter().zip(v).map(|(a, b)| a * b).sum()).collect()
        };
        let mut y = vec![0.0f64; HS];
        for (n, e) in experten.iter().enumerate() {
            let i = *e as usize;
            let gate = mal(&echt(&a.gate[i], HS), x);
            let up = mal(&echt(&a.up[i], HS), x);
            let h: Vec<f64> =
                gate.iter().zip(up.iter()).map(|(q, u)| (q / (1.0 + (-q).exp())) * u).collect();
            let aus = mal(&echt(&a.down[i], IS), &h);
            for (ziel, v) in y.iter_mut().zip(aus.iter()) {
                *ziel += w[n] * v;
            }
        }
        y
    }

    /// ⚑ **Der Eingangsgradient trifft die geschlossene Form.**
    ///
    /// # ⚑ Warum nicht gegen den Ganzzahlpfad selbst gemessen wird
    ///
    /// Der erste Entwurf nahm einen Differenzenquotienten des
    /// **Ganzzahl**-Vorwärtspasses, erst mit quadratischem, dann mit
    /// linearem Verlust. Beide streuten, und die Diagnose kostete drei
    /// Anläufe:
    ///
    /// | Aufbau | Verhältnis gemessen zu vorhergesagt |
    /// |---|---|
    /// | quadratisch, k = 2 | 0,38 bis 1,74 |
    /// | quadratisch, **k = 1** (Pfad schon belegt) | 0,69 bis 1,74 |
    /// | linear, k = 1 | 0,57 bis 2,33 |
    ///
    /// ⚑ **Die zweite Zeile war die Antwort.** Bei `k = 1` gibt es
    /// keinen Routerzweig, und der verbleibende MLP-Pfad ist an anderer
    /// Stelle unabhängig belegt. Wenn **der** genauso streut, streut
    /// nicht der Gradient, sondern die Messung.
    ///
    /// Der Grund: `y` ist `i16`. Eine Störung von `x` verschiebt es um
    /// wenige Einheiten, und davon ist ein Teil Rundung. Gerechnet liegt
    /// das Rauschen bei `Σ|g_i|`, also rund 9000, gegen Signale von 2600
    /// bis 40000. **Ein Differenzenquotient über eine Funktion mit
    /// dieser Körnung misst zur Hälfte die Rundung.**
    ///
    /// ⚑ **Deshalb gegen die geschlossene Form**, wie
    /// `rmsnorm_backward_trifft_die_geschlossene_form` nebenan: Dieselben
    /// Gewichte, dieselbe Auswahl, aber in Gleitkomma gerechnet. Das ist
    /// die Funktion, die der Ganzzahlpfad annähert, und ihre Ableitung
    /// ist die Zahl, die der Gradient treffen soll.
    #[test]
    fn der_eingangsgradient_trifft_die_geschlossene_form() {
        let lut = exp_lut();
        let silu = silu_lut();
        let a = aufbau();
        let x: Vec<i16> = (0..HS).map(|i| (zahl(i + 5, 400)) as i16).collect();
        let g: Vec<Grad> = (0..HS).map(|i| (zahl(i + 999, 3000)) as i32).collect();

        let (_y, experten, gewichte, ausgaben, spuren, _l) = vorwaerts(&a, &x, &lut, &silu);
        let (rw, rs) = gewicht_aus_master(&a.router, HS, MASTER_FRAC);
        let mut halde = Vec::new();
        let ew = gewichte_von(&a, &experten, &mut halde);
        let gr = gradienten_des_gemisches(
            &g, &x, &experten, &gewichte, &ausgaben, &spuren, &ew, &rw, &rs, &silu,
            &crate::backward::silu_grad_aus_lut(&silu), vorgaben(), gv(),
        );

        // Der Routerzweig für sich, wie ihn die Funktion rechnet.
        let gemisch = crate::backward::moe_backward(
            &g, &experten, &gewichte, &ausgaben, N, GEW, AUS, 6,
        );
        let (g_x_router, _) =
            crate::backward::linear_backward(&gemisch.logits, &x, &rw, HS, &rs, AUS + 6, ACT);

        // Die Messgroesse: `M = Σ g_i · y_i` in Ganzzahleinheiten, also
        // `Σ g_i · y_real · 2^AUS`.
        let skala = 2f64.powi(i32::from(AUS));
        let xr: Vec<f64> = x.iter().map(|v| f64::from(*v) / 2f64.powi(i32::from(ACT))).collect();
        let messen = |xr: &[f64]| -> f64 {
            let y = vorwaerts_f64(&a, xr, &experten);
            y.iter().zip(g.iter()).map(|(p, c)| p * f64::from(*c)).sum::<f64>() * skala
        };

        // ⚑ Zentraler Differenzenquotient in Gleitkomma.
        let eps = 1e-4f64;
        let geschlossen: Vec<f64> = (0..HS)
            .map(|j| {
                let mut hoch = xr.clone();
                let mut runter = xr.clone();
                hoch[j] += eps;
                runter[j] -= eps;
                (messen(&hoch) - messen(&runter)) / (2.0 * eps) / 2f64.powi(i32::from(ACT))
            })
            .collect();
        let unser: Vec<f64> = gr.eingang.iter().map(|v| f64::from(*v)).collect();

        // ⚑ **Der Winkel und nicht die einzelne Komponente.**
        //
        // Gemessen am 2026-09-05 streut die einzelne Komponente auch auf
        // dem **dichten** Pfad, der unabhängig belegt ist, um 0,74 bis
        // 1,14; über die Mischung um 0,5 bis 1,6. Das ist die Auflösung
        // des ganzzahligen Rückwärtspasses und kein Fehler: Er summiert
        // 64 Produkte und rundet dabei.
        //
        // **Ein Gradient hat die Aufgabe, bergab zu zeigen.** Der
        // Kosinus zwischen ihm und der geschlossenen Form misst genau
        // das, und er ist gegen das Rauschen der einzelnen Komponente
        // robust, ohne einen Skalenfehler durchzulassen: Der Fehler von
        // heute (`router_frac` statt `act_frac`, Faktor acht auf einem
        // von zwei Zweigen) drückt ihn auf 0,733, und das Längenverhältnis
        // auf 5,2.
        let punkt: f64 = unser.iter().zip(geschlossen.iter()).map(|(a, b)| a * b).sum();
        let n1: f64 = unser.iter().map(|v| v * v).sum::<f64>().sqrt();
        let n2: f64 = geschlossen.iter().map(|v| v * v).sum::<f64>().sqrt();
        let kosinus = punkt / (n1 * n2);
        let laengenverhaeltnis = n1 / n2;
        eprintln!(
            "    Kosinus zur geschlossenen Form: {kosinus:.4}, \
             Laengenverhaeltnis {laengenverhaeltnis:.3}"
        );
        assert!(
            kosinus > 0.93,
            "der Gradient zeigt nicht in dieselbe Richtung wie die geschlossene Form: \
             Kosinus {kosinus:.4}"
        );
        // ⚑ **Und die Länge zählt mit.** Ein Gradient, der richtig zeigt
        // und um eine Zweierpotenz zu lang ist, wäre ein Skalenfehler,
        // den der Kosinus allein durchliesse.
        assert!(
            (0.70..=1.40).contains(&laengenverhaeltnis),
            "der Gradient ist um den Faktor {laengenverhaeltnis:.3} zu lang oder zu kurz"
        );

        // ⚑ **Und der Routerzweig für sich, gegen seine eigene
        // geschlossene Form.** Er ist die neue Rechnung; der
        // Expertenzweig ist an anderer Stelle belegt. Hier trägt er vier
        // Stellen, weil er nur einen Nachschlag und ein Skalarprodukt
        // weit vom Eingang entfernt ist.
        let w_fest: Vec<f64> =
            gewichte.iter().map(|w| f64::from(*w) / 2f64.powi(i32::from(GEW))).collect();
        let messen_fest = |xr: &[f64]| -> f64 {
            let y = vorwaerts_f64_feste_gewichte(&a, xr, &experten, &w_fest);
            y.iter().zip(g.iter()).map(|(p, c)| p * f64::from(*c)).sum::<f64>() * skala
        };
        for j in 0..HS.min(8) {
            let mut hoch = xr.clone();
            let mut runter = xr.clone();
            hoch[j] += eps;
            runter[j] -= eps;
            let fest = (messen_fest(&hoch) - messen_fest(&runter)) / (2.0 * eps)
                / 2f64.powi(i32::from(ACT));
            let router_geschlossen = geschlossen[j] - fest;
            let verhaeltnis = router_geschlossen / f64::from(g_x_router[j]);
            eprintln!(
                "    Routerzweig j={j}: geschlossen {router_geschlossen:.0}, \
                 gerechnet {}, Verhaeltnis {verhaeltnis:.4}",
                g_x_router[j]
            );
            assert!(
                (0.97..=1.03).contains(&verhaeltnis),
                "j={j}: der Routerzweig liegt um den Faktor {verhaeltnis:.4} daneben"
            );
        }
    }

    /// ⚑ **Die Eichung: der dichte Block allein gegen die geschlossene
    /// Form.**
    ///
    /// # Wozu, wenn der dichte Pfad anderswo belegt ist
    ///
    /// **Damit die Schranke daneben eine Bedeutung hat.** Der Test über
    /// das Gemisch verlangt einen Kosinus über 0,93; ohne diesen hier
    /// wäre offen, ob das streng oder grosszügig ist. Hier steht, was
    /// derselbe Aufbau **ohne** Mischung erreicht, und das ist der
    /// Massstab.
    ///
    /// ⚑ **Und er hat am 2026-09-05 die Fehlersuche entschieden.** Als
    /// die einzelnen Komponenten über das Gemisch um den Faktor 0,3 bis
    /// −3,7 danebenlagen, war die Frage: liegt es am Gradienten oder an
    /// der Messung? Dieser Test sagte „auch der belegte Pfad streut um
    /// 0,74 bis 1,14", und damit war klar, dass die einzelne Komponente
    /// nichts entscheidet und der Winkel gefragt ist.
    #[test]
    fn der_dichte_block_allein_eicht_die_schranke() {
        let silu = silu_lut();
        let a = aufbau();
        let x: Vec<i16> = (0..HS).map(|i| (zahl(i + 5, 400)) as i16).collect();
        let g: Vec<Grad> = (0..HS).map(|i| (zahl(i + 999, 3000)) as i32).collect();
        let v = vorgaben();
        let i = 0usize;
        let (gw, gs) = gewicht_aus_master(&a.gate[i], HS, MASTER_FRAC);
        let (uw, us) = gewicht_aus_master(&a.up[i], HS, MASTER_FRAC);
        let (dw, ds) = gewicht_aus_master(&a.down[i], IS, MASTER_FRAC);
        let aus_frac = vec![v.aus_frac; HS];
        let mut spur = Mlpspur::default();
        let _ = crate::mlp::mlp_int_mit_spur(
            &x, &gw, &uw, &dw, HS, IS, &gs, &us, &ds, &silu, ACT, v.gate_frac, v.up_frac,
            v.down_in_frac, v.silu_in_frac, v.silu_lut_offset, v.silu_out_frac, &aus_frac,
            Some(&mut spur),
        );
        let gr = gradienten_des_mlp_aus_gradient(
            &g, &x, &spur, &gw, &gs, &uw, &us, &dw, &ds, &silu,
            &crate::backward::silu_grad_aus_lut(&silu), v,
        );

        // Ein Experte, Mischgewicht eins: genau der dichte Block.
        let nur = [0u16];
        let eins = [1.0f64];
        let messen = |xr: &[f64]| -> f64 {
            let y = vorwaerts_f64_feste_gewichte(&a, xr, &nur, &eins);
            y.iter().zip(g.iter()).map(|(p, c)| p * f64::from(*c)).sum::<f64>()
                * 2f64.powi(i32::from(AUS))
        };
        let xr: Vec<f64> = x.iter().map(|c| f64::from(*c) / 2f64.powi(i32::from(ACT))).collect();
        let eps = 1e-4;
        let geschlossen: Vec<f64> = (0..HS)
            .map(|j| {
                let mut h = xr.clone();
                let mut r = xr.clone();
                h[j] += eps;
                r[j] -= eps;
                (messen(&h) - messen(&r)) / (2.0 * eps) / 2f64.powi(i32::from(ACT))
            })
            .collect();
        let unser: Vec<f64> = gr.eingang.iter().map(|v| f64::from(*v)).collect();
        let punkt: f64 = unser.iter().zip(geschlossen.iter()).map(|(a, b)| a * b).sum();
        let n1: f64 = unser.iter().map(|v| v * v).sum::<f64>().sqrt();
        let n2: f64 = geschlossen.iter().map(|v| v * v).sum::<f64>().sqrt();
        let kosinus = punkt / (n1 * n2);
        let laenge = n1 / n2;
        eprintln!("    dichter Block: Kosinus {kosinus:.4}, Laengenverhaeltnis {laenge:.3}");
        assert!(kosinus > 0.95, "schon der dichte Block zeigt woandershin: {kosinus:.4}");
        assert!((0.80..=1.25).contains(&laenge), "Laengenverhaeltnis {laenge:.3}");
    }

    /// ⚑ **Je gewähltem Experten ein Eintrag, und die Indizes stimmen.**
    #[test]
    fn nur_die_gewaehlten_experten_bekommen_gradienten() {
        let lut = exp_lut();
        let silu = silu_lut();
        let a = aufbau();
        let x: Vec<i16> = (0..HS).map(|i| (zahl(i + 5, 400)) as i16).collect();
        let (y, experten, gewichte, ausgaben, spuren, _l) = vorwaerts(&a, &x, &lut, &silu);
        let g: Vec<Grad> = y.iter().map(|p| 2 * i32::from(*p)).collect();
        let (rw, rs) = gewicht_aus_master(&a.router, HS, MASTER_FRAC);
        let mut halde = Vec::new();
        let ew = gewichte_von(&a, &experten, &mut halde);
        let gr = gradienten_des_gemisches(
            &g, &x, &experten, &gewichte, &ausgaben, &spuren, &ew, &rw, &rs, &silu,
            &crate::backward::silu_grad_aus_lut(&silu), vorgaben(), gv(),
        );
        assert_eq!(gr.experten.len(), K, "es rechnen k Experten, also gibt es k Eintraege");
        let indizes: Vec<u16> = gr.experten.iter().map(|(e, _)| *e).collect();
        assert_eq!(indizes, experten, "die Reihenfolge weicht von der Auswahl ab");
        assert_eq!(gr.router.len(), N * HS, "der Routergradient hat die falsche Groesse");
        assert_eq!(gr.eingang.len(), HS);
    }
}
