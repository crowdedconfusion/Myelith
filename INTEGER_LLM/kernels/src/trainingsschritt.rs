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

use crate::backward::{linear_backward, silu_backward, silu_grad_frac, Grad};
use crate::fixed_point::{clamp_i16, rescale, rescale_i64};
use crate::integer_math::lut_lookup;
use crate::linear::linear_w8a16;
use crate::mlp::{mlp_int_mit_spur, Mlpspur};
use crate::optimierer::{schritt, Master, Schrittkennung};

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
pub fn gewicht_aus_master(
    master: &[Master],
    in_features: usize,
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
            continue;
        }
        // Wie weit muss nach rechts geschoben werden, damit der groesste
        // Betrag in i8 passt? `127` ist der groesste darstellbare Betrag.
        let mut s = 0u32;
        while (groesster >> s) > 127 {
            s += 1;
        }
        shifts[z] = u8::try_from(s).unwrap_or(u8::MAX);
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
        lr_zaehler,
        lr_nenner,
        kennung,
    } = v;
    let (w, shifts) = gewicht_aus_master(master, in_features);
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

/// Die Gewichtsgradienten eines MLP-Blocks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mlpgradienten {
    /// `dL/dW` der Gate-Projektion, in Zeilen zu `hidden_size`.
    pub gate: Vec<Grad>,
    /// `dL/dW` der Up-Projektion.
    pub up: Vec<Grad>,
    /// `dL/dW` der Down-Projektion, in Zeilen zu `intermediate_size`.
    pub down: Vec<Grad>,
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
    let (wg, sg) = gewicht_aus_master(gate_master, v.hidden_size);
    let (wu, su) = gewicht_aus_master(up_master, v.hidden_size);
    let (wd, sd) = gewicht_aus_master(down_master, v.intermediate_size);
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

    // 1. Durch `down_proj`: Gradient nach dem Produkt.
    let (g_h, gw_down) = linear_backward(
        &g, &spur.h, &wd, v.intermediate_size, &sd, v.aus_frac, v.down_in_frac,
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
    let (_gx_g, gw_gate) =
        linear_backward(&g_gate, x, &wg, v.hidden_size, &sg, v.down_in_frac, v.act_frac);
    let (_gx_u, gw_up) =
        linear_backward(&g_up, x, &wu, v.hidden_size, &su, v.down_in_frac, v.act_frac);

    (
        abstand,
        Mlpgradienten {
            gate: nach_grad(&gw_gate),
            up: nach_grad(&gw_up),
            down: nach_grad(&gw_down),
        },
    )
}

/// Saettigt einen `i64`-Gradienten auf [`Grad`].
///
/// ⚑ **Gesaettigt statt umlaufend**, denn ein umgelaufener Gradient
/// zeigt in die **Gegenrichtung**, und der Schritt liefe dann bergauf.
fn begrenze(v: i64) -> Grad {
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
            .map(|i| (i as i32 * 37) % 11 - 5)
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
                    lr_zaehler: 1,
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

        let mut gate: Vec<Master> = (0..is * hs).map(|i| (i as i32 * 29) % 13 - 6).collect();
        let mut up: Vec<Master> = (0..is * hs).map(|i| (i as i32 * 17) % 11 - 5).collect();
        let mut down: Vec<Master> = (0..hs * is).map(|i| (i as i32 * 23) % 9 - 4).collect();
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

        let mut gate: Vec<Master> = (0..is * hs).map(|i| (i as i32 * 29) % 13 - 6).collect();
        let mut up: Vec<Master> = (0..is * hs).map(|i| (i as i32 * 17) % 11 - 5).collect();
        let mut down: Vec<Master> = (0..hs * is).map(|i| (i as i32 * 23) % 9 - 4).collect();
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
        let gate: Vec<Master> = (0..is * hs).map(|i| (i as i32 * 29) % 13 - 6).collect();
        let up: Vec<Master> = (0..is * hs).map(|i| (i as i32 * 17) % 11 - 5).collect();
        let down: Vec<Master> = (0..hs * is).map(|i| (i as i32 * 23) % 9 - 4).collect();
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
        let treffer = richtungstreffer(64);
        for (n, name) in ["gate", "up", "down"].iter().enumerate() {
            let (gesenkt, gewertet) = treffer[n];
            assert!(
                gewertet >= 40,
                "{name}: nur {gewertet} Schuebe aenderten ueberhaupt etwas; \
                 die Messung traegt dann nicht"
            );
            // 70 Prozent liegt deutlich ueber dem Zufall (50) und
            // deutlich unter dem Gemessenen (82 bis 85).
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
        let gate: Vec<Master> = (0..is * hs).map(|i| (i as i32 * 29) % 13 - 6).collect();
        let up: Vec<Master> = (0..is * hs).map(|i| (i as i32 * 17) % 11 - 5).collect();
        let down: Vec<Master> = (0..hs * is).map(|i| (i as i32 * 23) % 9 - 4).collect();
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

    /// Je Zeile eine eigene Skala, sonst faellt die kleine auf null.
    #[test]
    fn zwei_zeilen_bekommen_zwei_skalen() {
        let master = vec![
            1_000_000, 2_000_000, // grosse Zeile
            3, 4, // kleine Zeile
        ];
        let (w, s) = gewicht_aus_master(&master, 2);
        assert_ne!(s[0], s[1], "beide Zeilen bekamen dieselbe Skala");
        // ⚑ Die kleine Zeile ueberlebt: Mit gemeinsamer Skala waere sie
        // null.
        assert!(w[2] != 0 || w[3] != 0, "die kleine Zeile fiel auf null");
    }

    /// ⚑ **Eine Nullzeile bekommt keine erfundene Skala.**
    #[test]
    fn eine_nullzeile_bleibt_null() {
        let master = vec![0, 0, 0, 5];
        let (w, s) = gewicht_aus_master(&master, 2);
        assert_eq!(s[0], 0, "eine Nullzeile braucht keine Verschiebung");
        assert_eq!(&w[..2], &[0, 0]);
    }

    /// Der groesste Betrag passt nach der Verschiebung in `i8`.
    #[test]
    fn nach_der_verschiebung_passt_alles_in_i8() {
        for gross in [127i32, 128, 1_000, i32::MAX / 2] {
            let master = vec![gross, -gross, 1, 0];
            let (w, s) = gewicht_aus_master(&master, 4);
            assert!(
                w.iter().all(|v| (-127..=127).contains(&i32::from(*v))),
                "Verschiebung {} reichte fuer {gross} nicht",
                s[0]
            );
        }
    }
}
