//! Training über einen **Ebenenbereich**, also das, was ein Shard tut
//! (Whitepaper Kap. 7.2).
//!
//! # ⚑ Die Beobachtung, aus der alles folgt
//!
//! Ein Trainingspod ist **dieselbe Pipeline wie ein Inferenzpod,
//! zweimal**: einmal vorwärts, einmal rückwärts. Shard *j* hält die
//! Ebenen `[von, bis)`, rechnet vorwärts und behält seinen Mitschnitt;
//! danach bekommt er `dL/dZ` am Ausgang seines Bereichs und gibt
//! `dL/dX` an dessen Eingang zurück, also genau das, was Shard *j−1* am
//! eigenen Ausgang braucht.
//!
//! # ⚑ Drei Dinge, an denen das steht oder fällt
//!
//! **Erstens: die globale Ebenennummer.** Das stochastische Runden
//! entscheidet über jedes einzelne Gewicht, und der Würfel ist eine
//! reine Funktion aus `(ebene, schritt, index)`. Ein Shard, der seine
//! Ebenen bei null durchnummerierte, würfelte anders. **Niemand sähe es
//! dem Ergebnis an:** Es wäre ein plausibles Delta, nur ein anderes, und
//! die Redundanzprüfung zweier verschieden zugeschnittener Pods
//! schlüge fehl, ohne dass einer von beiden falsch gerechnet hätte.
//!
//! `optimierer::shardtransparenz` hält die Eigenschaft fest; hier wird
//! sie **benutzt**, indem [`Shardvorgaben::von`] die globale Nummer ist.
//!
//! **Zweitens: die Skala des Gradienten.** Ein Gradientenfeld trägt
//! `dL/dZ` nach dem **realen Wert** von Z, auf einer Skala, die der
//! Aufrufer wählt. Zwischen zwei Shards muss diese Skala mitlaufen,
//! sonst weiss der Empfänger nicht, was er bekommen hat. **Ein Gradient
//! ohne Skala ist eine Zahl ohne Einheit**, und das ist genau die
//! Fehlerklasse von Fund 179: Dort rechnete ein Zweig mit der falschen
//! Skala und war exakt achtfach daneben, während der Abstandstest grün
//! blieb.
//!
//! Hier ist sie [`Shardergebnis::eingang_frac`], und sie ist keine
//! Wahl: Es ist die Skala des Residualstroms am Eingang des Bereichs.
//!
//! **Drittens: der Mitschnitt überlebt die Lücke.** Bei der Inferenz ist
//! ein Shard je Token zustandslos. Beim Training hält er zwischen
//! Vorwärts- und Rückwärtslauf Zustand, und zwar **je Ebene eine
//! Ebenenspur**. Ein Shard, der mitten im Segment stirbt, nimmt ihn mit;
//! die Reserve kann nicht einspringen, weil sie ihn nicht hat, und das
//! Segment muss von vorn gerechnet werden.
//!
//! ⚑ **Daraus folgt eine Obergrenze für die Segmentlänge**, und sie ist
//! keine Bequemlichkeit, sondern der Erwartungswert verlorener Arbeit
//! geteilt durch die Ausfallrate.

use integer_llm_kernels::optimierer::{
    ausserhalb_der_form, delta, schritt, trainingsabdruck, Master, Schrittkennung, MASTER_FRAC,
};
use integer_llm_kernels::backward::silu_grad_aus_lut;
use integer_llm_kernels::trainingsschritt::{
    gewicht_aus_master, gradienten_der_ebene_aus_gradient, vorwaerts_der_ebene, Ebenenspur,
    Ebenentabellen,
};

use crate::model::{Feedforward, IntegerModel};
use crate::trainingsschleife::{
    breiten_der_ebene, gewichte_der_ebene, master_der_ebene, vorgaben_der_ebene,
    vorspannungen_der_ebene,
};

/// Was ein Shard über seinen Auftrag wissen muss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shardvorgaben {
    /// Die **globale** Nummer der ersten Ebene dieses Shards.
    ///
    /// ⚑ Siehe den Modulkopf: Daran hängt die Bitgleichheit zum
    /// Einzelknoten.
    pub von: usize,
    /// Hinter der letzten Ebene dieses Shards, global.
    pub bis: usize,
    /// Der wievielte Trainingsschritt des Segments.
    pub schritt: u64,
    /// Zähler der Lernrate. **Vom Protokoll gesetzt**, nicht vom Shard.
    pub lr_zaehler: i64,
    /// Nenner der Lernrate.
    pub lr_nenner: i64,
}

/// Die Gewichte eines Shards, über die Schritte hinweg.
///
/// # ⚑ Warum sie nicht im Mitschnitt stehen
///
/// Der Mitschnitt gehört zu **einem** Vorwärtslauf und wird danach
/// weggeworfen. Die Gewichte gehören zum **Segment** und müssen jeden
/// Schritt überleben: Ein Shard, der sie im Mitschnitt hielte, finge
/// bei jedem Schritt wieder beim Artefakt an, und dreissig Schritte
/// wären dreissig Mal derselbe erste Schritt.
///
/// ⚑ **Der erste Entwurf hatte genau diesen Fehler** (2026-09-05):
/// `rueckwaerts` änderte eine Kopie und warf sie weg. Ein Test über
/// einen einzigen Schritt hätte ihn nicht gefunden.
pub struct Shardgewichte {
    /// Je Ebene des Bereichs: ihre sieben Mastermatrizen.
    pub master: Vec<[Vec<Master>; 7]>,
    /// Der Stand **vor** dem ersten Schritt, für das Δ-Commitment.
    anfang: Vec<[Vec<Master>; 7]>,
    von: usize,
    bis: usize,
}

impl Shardgewichte {
    /// Liest die Gewichte des Bereichs aus dem Modell.
    pub fn aus_modell(m: &IntegerModel, von: usize, bis: usize) -> Result<Self, Shardfehler> {
        if von >= bis || bis > m.num_layers {
            return Err(Shardfehler::BereichUngueltig { von, bis, ebenen: m.num_layers });
        }
        let mut master = Vec::with_capacity(bis - von);
        for (e, ebene) in m.layers.iter().enumerate().take(bis).skip(von) {
            let Some(mm) = master_der_ebene(ebene) else {
                return Err(Shardfehler::GemischNochNicht { ebene: e });
            };
            master.push(mm);
        }
        Ok(Self { anfang: master.clone(), master, von, bis })
    }

    /// Wie viele Ebenen der Bereich hält.
    pub fn ebenen(&self) -> usize {
        self.bis - self.von
    }

    /// Die erste Ebene des Bereichs, global.
    pub fn von(&self) -> usize {
        self.von
    }

    /// Der Stand **vor** dem ersten Schritt.
    ///
    /// ⚑ **Lesbar und nicht schreibbar.** Wer ihn setzen könnte, könnte
    /// sein eigenes Δ-Commitment bestimmen, ohne etwas gerechnet zu
    /// haben.
    pub fn anfangsstand(&self) -> &[[Vec<Master>; 7]] {
        &self.anfang
    }
}

/// Der Mitschnitt eines Shards zwischen Vorwärts- und Rückwärtslauf.
///
/// ⚑ **Das ist der Zustand, den die Inferenz nicht hat.** Er gehört
/// nicht in eine Nachricht und nicht in den Konsens: Er ist zu gross und
/// vollständig aus Eingabe und Gewichten wiederherstellbar. Wer ihn
/// verliert, rechnet das Segment neu.
pub struct Shardmitschnitt {
    /// Je Ebene des Bereichs: der Residualstrom **vor** ihr.
    pub eingaenge: Vec<Vec<Vec<i16>>>,
    /// Je Ebene des Bereichs: ihre Spur.
    pub spuren: Vec<Ebenenspur>,
    /// Der Ausgang des Bereichs, also der Eingang des nächsten Shards.
    pub ausgang: Vec<Vec<i16>>,
}

/// Was ein Shard nach dem Rückwärtslauf abliefert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shardergebnis {
    /// `dL/dX` am **Eingang** des Bereichs, je Position.
    ///
    /// Das ist, was Shard *j−1* als `dL/dZ` an seinem Ausgang liest.
    pub eingang: Vec<Vec<i32>>,
    /// Die Skalen, auf denen [`Shardergebnis::eingang`] steht, **je
    /// Kanal**.
    ///
    /// ⚑ **Muss mit über den Draht**, siehe den Modulkopf: Ein Gradient
    /// ohne Skala ist eine Zahl ohne Einheit.
    ///
    /// ⚑ **Und es ist ein Feld, keine Zahl.** Der Residualstrom dieses
    /// Projekts trägt **eine Verschiebung je Kanal**, nicht eine für
    /// alle. Wer hier ein `u8` erwartete, hätte den Gradienten eines
    /// jeden Kanals ausser einem falsch skaliert, und der Fehler wäre
    /// klein genug, um wie Rauschen auszusehen.
    pub eingang_frac: Vec<u8>,
    /// Der Abdruck über die **Änderung** an den Gewichten dieses Shards.
    pub delta_commitment: String,
    /// Der Abdruck über den **Endstand** der Gewichte dieses Shards.
    pub abdruck: String,
    /// Wie viele Gewichte sich bewegt haben.
    pub bewegte_gewichte: usize,
    /// Wie viele Gewichte der Bereich hat.
    pub gewichte_gesamt: usize,
    /// Welche **globale** Ebene die Übertragungsform verlassen hat.
    ///
    /// ⚑ **Eine Panik wäre hier keine Antwort.** Ein Pod, dessen Segment
    /// aus der Form läuft, muss das melden können, damit die Kette es
    /// als gescheitert verbucht statt auf ein Ergebnis zu warten, das
    /// nie kommt: **Ein Absturz ist von einem ausgefallenen Miner nicht
    /// zu unterscheiden.**
    pub aus_der_form: Option<usize>,
}

/// Fehler eines Shardlaufs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shardfehler {
    /// Der Bereich liegt nicht im Modell oder ist leer.
    BereichUngueltig { von: usize, bis: usize, ebenen: usize },
    /// Eine Ebene des Bereichs ist ein Expertengemisch.
    ///
    /// ⚑ **Kein Abbruch aus Bequemlichkeit.** Der Rückwärtsweg über ein
    /// Gemisch steht (`gradienten_des_gemisches`), aber er über
    /// Shardgrenzen zu führen verlangt, dass der Router seine Auswahl
    /// mitschneidet; das ist ein eigener Punkt und keine Zeile hier.
    GemischNochNicht { ebene: usize },
    /// Der eingehende Gradient passt nicht zur Folge.
    GradientPasstNicht { erwartet: usize, bekommen: usize },
}

impl std::fmt::Display for Shardfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BereichUngueltig { von, bis, ebenen } => write!(
                f,
                "Ebenenbereich {von}..{bis} liegt nicht in einem Modell mit {ebenen} Ebenen"
            ),
            Self::GemischNochNicht { ebene } => {
                write!(f, "Ebene {ebene} ist ein Expertengemisch; das traegt der Shardweg noch nicht")
            }
            Self::GradientPasstNicht { erwartet, bekommen } => write!(
                f,
                "der eingehende Gradient hat {bekommen} Positionen, die Folge {erwartet}"
            ),
        }
    }
}

impl std::error::Error for Shardfehler {}

/// Der Vorwärtslauf eines Shards, **mit Mitschnitt**.
///
/// `hidden` ist der Residualstrom am Eingang des Bereichs, je Position.
/// Bei Shard 0 ist das die Einbettung.
///
/// ⚑ **Die Gewichte kommen aus den Mastern und nicht aus dem Artefakt.**
/// Ein Trainingsschritt schreibt Master fort; würde vorwärts mit den
/// Artefaktgewichten gerechnet und rückwärts gegen die Master, liefen
/// beide nach dem ersten Schritt auseinander.
pub fn vorwaerts(
    m: &IntegerModel,
    g: &Shardgewichte,
    v: &Shardvorgaben,
    hidden: &[Vec<i16>],
) -> Result<Shardmitschnitt, Shardfehler> {
    if v.von >= v.bis || v.bis > m.num_layers || g.von != v.von || g.bis != v.bis {
        return Err(Shardfehler::BereichUngueltig {
            von: v.von,
            bis: v.bis,
            ebenen: m.num_layers,
        });
    }
    let grad_lut = silu_grad_aus_lut(&m.silu_lut);
    let mut mitschnitt = Shardmitschnitt {
        eingaenge: Vec::with_capacity(v.bis - v.von),
        spuren: Vec::with_capacity(v.bis - v.von),
        ausgang: Vec::new(),
    };
    let mut strom: Vec<Vec<i16>> = hidden.to_vec();

    for (i, e) in (v.von..v.bis).enumerate() {
        let ebene = &m.layers[e];
        let Feedforward::Dense(mlp) = &ebene.ffn else {
            return Err(Shardfehler::GemischNochNicht { ebene: e });
        };
        let is = mlp.gate_proj.shape[0];
        let breiten = breiten_der_ebene(m, is);
        let umgerechnet: Vec<(Vec<i8>, Vec<u8>)> = (0..7)
            .map(|n| gewicht_aus_master(&g.master[i][n], breiten[n], MASTER_FRAC))
            .collect();
        let gew = gewichte_der_ebene(&umgerechnet, ebene);
        let vg = vorgaben_der_ebene(
            m, &ebene.scales, e, is, v.schritt, v.lr_zaehler, v.lr_nenner,
        );
        let tab = Ebenentabellen {
            cos: &m.cos_lut,
            sin: &m.sin_lut,
            exp: &m.exp_lut,
            rsqrt: &m.rsqrt_lut,
            silu: &m.silu_lut,
            silu_grad: &grad_lut,
        };
        let spur = vorwaerts_der_ebene(gew, &strom, vorspannungen_der_ebene(ebene), tab, vg);
        mitschnitt.eingaenge.push(std::mem::replace(&mut strom, spur.y.clone()));
        mitschnitt.spuren.push(spur);
    }
    mitschnitt.ausgang = strom;
    Ok(mitschnitt)
}

/// Der Rückwärtslauf eines Shards: Gradienten, Schritt, Weitergabe.
///
/// `g_aus` ist `dL/dZ` am **Ausgang** des Bereichs, je Position; bei
/// Shard *j* ist das, was Shard *j+1* als `eingang` zurückgab.
///
/// ⚑ **Die Ebenen laufen rückwärts**, von `bis−1` bis `von`, und der
/// Gradient wandert dabei durch: Was `gradienten_der_ebene_aus_gradient`
/// als `eingang` liefert, ist der Ausgangsgradient der Ebene davor.
pub fn rueckwaerts(
    m: &IntegerModel,
    gew_stand: &mut Shardgewichte,
    v: &Shardvorgaben,
    mitschnitt: &Shardmitschnitt,
    g_aus: &[Vec<i32>],
) -> Result<Shardergebnis, Shardfehler> {
    if mitschnitt.eingaenge.is_empty() {
        return Err(Shardfehler::BereichUngueltig {
            von: v.von,
            bis: v.bis,
            ebenen: m.num_layers,
        });
    }
    let positionen = mitschnitt.eingaenge[0].len();
    if g_aus.len() != positionen {
        return Err(Shardfehler::GradientPasstNicht {
            erwartet: positionen,
            bekommen: g_aus.len(),
        });
    }
    let grad_lut = silu_grad_aus_lut(&m.silu_lut);
    let mut g = g_aus.to_vec();
    let mut aus_der_form: Option<usize> = None;

    for (i, e) in (v.von..v.bis).enumerate().rev() {
        let ebene = &m.layers[e];
        let Feedforward::Dense(mlp) = &ebene.ffn else {
            return Err(Shardfehler::GemischNochNicht { ebene: e });
        };
        let is = mlp.gate_proj.shape[0];
        let breiten = breiten_der_ebene(m, is);
        let umgerechnet: Vec<(Vec<i8>, Vec<u8>)> = (0..7)
            .map(|n| gewicht_aus_master(&gew_stand.master[i][n], breiten[n], MASTER_FRAC))
            .collect();
        let gew = gewichte_der_ebene(&umgerechnet, ebene);
        let vg = vorgaben_der_ebene(
            m, &ebene.scales, e, is, v.schritt, v.lr_zaehler, v.lr_nenner,
        );
        let tab = Ebenentabellen {
            cos: &m.cos_lut,
            sin: &m.sin_lut,
            exp: &m.exp_lut,
            rsqrt: &m.rsqrt_lut,
            silu: &m.silu_lut,
            silu_grad: &grad_lut,
        };
        let gr = gradienten_der_ebene_aus_gradient(
            gew,
            &mitschnitt.spuren[i],
            &mitschnitt.eingaenge[i],
            &g,
            tab,
            vg,
        );

        // ⚑ **Der Versatz läuft über die sieben Matrizen einer Ebene**,
        // nicht über den ganzen Shard: `Schrittkennung::index_versatz`
        // zählt innerhalb der Ebene, und die Ebene ist global
        // nummeriert. Genau diese Aufteilung macht den Zuschnitt
        // gleichgültig.
        let kn = vg.aufmerksamkeit.kennung;
        let gradienten = [
            &gr.aufmerksamkeit.q,
            &gr.aufmerksamkeit.k,
            &gr.aufmerksamkeit.v,
            &gr.aufmerksamkeit.o,
            &gr.mlp.gate,
            &gr.mlp.up,
            &gr.mlp.down,
        ];
        let mut versatz = 0u64;
        for (mm, gg) in gew_stand.master[i].iter_mut().zip(gradienten.iter()) {
            let laenge = mm.len() as u64;
            schritt(mm, gg, Schrittkennung { index_versatz: versatz, ..kn }, 1, v.lr_nenner);
            versatz += laenge;
        }
        if aus_der_form.is_none()
            && gew_stand.master[i].iter().any(|mm| ausserhalb_der_form(mm).is_some())
        {
            aus_der_form = Some(e);
        }
        g = gr.eingang;
    }

    // Die Abdrücke gehen über alle Matrizen aller Ebenen, in
    // Ebenenreihenfolge. ⚑ **Dieselbe Reihenfolge für beide**, sonst
    // beschriebe das Commitment eine andere Aufteilung derselben Zahlen.
    let flach: Vec<&[Master]> =
        gew_stand.master.iter().flat_map(|e| e.iter().map(|v| v.as_slice())).collect();
    let deltas: Vec<Vec<Master>> = gew_stand
        .anfang
        .iter()
        .zip(gew_stand.master.iter())
        .flat_map(|(a, b)| a.iter().zip(b.iter()).map(|(x, y)| delta(x, y)))
        .collect();
    let delta_scheiben: Vec<&[Master]> = deltas.iter().map(|v| v.as_slice()).collect();
    let bewegte_gewichte = gew_stand
        .anfang
        .iter()
        .zip(gew_stand.master.iter())
        .map(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .map(|(x, y)| x.iter().zip(y.iter()).filter(|(p, q)| p != q).count())
                .sum::<usize>()
        })
        .sum();

    Ok(Shardergebnis {
        eingang: g,
        // Die Skala des Residualstroms am Eingang der ersten Ebene des
        // Bereichs. **Nicht gewählt, sondern abgelesen.**
        eingang_frac: m.layers[v.von].scales.residual_in_frac.clone(),
        delta_commitment: trainingsabdruck(&delta_scheiben),
        abdruck: trainingsabdruck(&flach),
        bewegte_gewichte,
        gewichte_gesamt: gew_stand.master.iter().flat_map(|e| e.iter()).map(|m| m.len()).sum(),
        aus_der_form,
    })
}
