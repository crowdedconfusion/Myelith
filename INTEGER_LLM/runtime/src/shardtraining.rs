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
//!
//! # ⚑ Gemischebenen, und was an ihnen anders ist
//!
//! Eine Ebene mit Expertengemisch läuft durch dieselben Stationen:
//! Normierung, Aufmerksamkeit, Residual, Normierung, Block, Residual.
//! **Nur der Block ist ein anderer**, und daraus folgen zwei Dinge, die
//! eine dichte Ebene nicht hat.
//!
//! **Erstens: die Experten kommen erst, wenn sie gewählt sind.** Das
//! qwen3-30b-a3b hat 128 Experten je Ebene zu je 4,7 Millionen
//! Gewichten. Alle als Master zu halten wären **2,4 GB je Ebene**, und
//! ein Shard hält ein Dutzend. Bei Top-8 über eine kurze Folge sind es
//! gemessen **25 von 128**.
//!
//! **Zweitens: der Versatz im Würfelraum hängt an der Expertennummer**,
//! nicht an der Auswahlreihenfolge. Sonst würfelte derselbe Experte
//! verschieden, je nachdem, an welcher Position er zuerst drankam, und
//! zwei Shards mit anderer Positionsverteilung liefen auseinander. Die
//! Aufteilung steht in [`Gemischversatz`].
//!
//! ⚑ **Gemessen am 2026-09-06 auf dem echten 30B:** Vier Gemischebenen
//! in zwei Shards zerlegt ergeben dasselbe wie dieselben vier am Stück,
//! Matrix für Matrix, 575 967 566 von 586 153 984 Gewichten bewegt.

use integer_llm_kernels::optimierer::{
    ausserhalb_der_form, delta, sammle, sammle_roh, schritt, schritt_aus_summe, schritt_normiert,
    trainingsabdruck, Master, Schrittkennung, MASTER_FRAC,
};
use integer_llm_kernels::backward::silu_grad_aus_lut;
use integer_llm_kernels::trainingsschritt::{
    gewicht_aus_master, gradienten_der_ebene_aus_gradient, vorwaerts_der_ebene, Ebenenspur,
    Ebenentabellen,
};

use crate::model::{Feedforward, IntegerModel};
use crate::trainingsschleife::{
    breiten_der_ebene, gewichte_der_ebene, master_aus_gewicht, master_der_ebene,
    vorgaben_der_ebene, vorspannungen_der_ebene,
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
    /// Je Ebene des Bereichs: ihr Stand.
    pub master: Vec<Ebenenstand>,
    /// Der Stand **vor** dem ersten Schritt, für das Δ-Commitment.
    anfang: Vec<Ebenenstand>,
    von: usize,
    bis: usize,
}

/// Der Gewichtsstand **einer** Ebene, dicht oder als Expertengemisch.
///
/// # ⚑ Warum ein Gemisch nicht einfach sieben Matrizen sind
///
/// Eine dichte Ebene hat vier Aufmerksamkeitsmatrizen und drei für den
/// MLP-Block: sieben, immer dieselben. Ein Gemisch hat vier, dazu den
/// Router und **je gewähltem Experten drei weitere**. Welche Experten
/// gewählt werden, entscheidet der Router, also die Gewichte, also der
/// laufende Schritt.
///
/// ⚑ **Die Experten kommen deshalb erst dazu, wenn sie gewählt sind.**
/// Das qwen3-30b-a3b hat 128 Experten je Ebene zu je 4,7 Millionen
/// Gewichten; sie alle als Master zu halten wären **2,4 GB je Ebene**,
/// und der Shard hält zwölf Ebenen. Bei Top-8 über eine kurze Folge sind
/// es höchstens ein paar Dutzend.
/// Ein Experte, aus seinen Mastern quantisiert: Gate, Up und Down, je
/// mit ihren Zeilenskalen.
///
/// ⚑ **Ein Name statt eines Sechsertupels.** Ausgeschrieben war der Typ
/// nicht zu lesen und nicht zu aendern, ohne an drei Stellen zu zaehlen,
/// an welcher Stelle welche Matrix steht.
type QuantisierterExperte = (Vec<i8>, Vec<u8>, Vec<i8>, Vec<u8>, Vec<i8>, Vec<u8>);

pub enum Ebenenstand {
    /// Q, K, V, O, Gate, Up, Down.
    Dicht(Box<[Vec<Master>; 7]>),
    /// Aufmerksamkeit, Router, und die bisher gewählten Experten.
    Gemisch {
        /// Q, K, V, O.
        aufmerksamkeit: Box<[Vec<Master>; 4]>,
        /// Die Routerprojektion.
        router: Vec<Master>,
        /// Je **Expertennummer** seine drei Matrizen.
        ///
        /// ⚑ **`BTreeMap` und nicht `HashMap`.** Die Karte wird geordnet
        /// durchlaufen, wenn der Abdruck entsteht; eine Hashtabelle gäbe
        /// dabei eine Reihenfolge, die an Adressen hängt.
        experten: std::collections::BTreeMap<u16, [Vec<Master>; 3]>,
    },
}

/// Welche Matrix einer Ebene gemeint ist.
///
/// # ⚑ Warum nicht der Index aus `matrizen()`
///
/// Die kanonische Reihenfolge ist eine **Momentaufnahme**: Bei einer
/// Gemischebene wächst die Expertenkarte, während gerechnet wird, und
/// Index 5 meint nach dem dritten Durchgang etwas anderes als nach dem
/// ersten. Eine Sammlung, die darüber Buch führt, addierte den
/// Gradienten eines Experten auf einen anderen.
///
/// **Diese Kennung hängt dagegen an der Sache**, nicht an der
/// Reihenfolge: Die Expertennummer steht drin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Matrixkennung {
    /// Q, K, V, O, Gate, Up, Down einer dichten Ebene.
    Dicht(u8),
    /// Q, K, V, O einer Gemischebene.
    Aufmerksamkeit(u8),
    /// Die Routerprojektion.
    Router,
    /// Expertennummer und eine seiner drei Matrizen.
    Experte(u16, u8),
}

/// Die aufgelaufene Bewegung eines Segments, in feinen Einheiten.
///
/// # ⚑ Wozu, und was ohne sie geschieht
///
/// Ein Trainingssegment schrieb bis zum 2026-09-06 nach **jeder** Folge
/// fort. Bei kleiner Lernrate liegt die Bewegung einer Folge unter einer
/// Master-Stufe; das stochastische Runden entscheidet dann je Gewicht
/// mit einem Münzwurf. Über wenige Ebenen ist das folgenlos, über
/// vierundzwanzig entscheidet es das Ergebnis: **gemessen ein Faktor
/// 1 644 bei sonst gleichem Lauf**, allein aus einem anderen
/// Würfelversatz (Fund 189).
///
/// ⚑ **Und die Redundanz kann es nicht sehen**, weil beide Pods eines
/// Paars denselben Würfel werfen. Ein schlechter Wurf ist ein Ergebnis,
/// über das sich zwei ehrliche Pods einig sind.
///
/// Die Sammlung summiert die feinen Einheiten aller Folgen und rundet
/// **einmal**. Erwartungswert gleich, Streuung durch die Zahl der
/// Folgen geteilt.
#[derive(Debug, Clone, Default)]
pub struct Sammlung {
    /// Je Ebene im Shard und Matrix: Würfelversatz und Summe.
    summen: std::collections::BTreeMap<(usize, Matrixkennung), (u64, Vec<i64>)>,
    /// Wie viele Folgen eingegangen sind.
    laeufe: u32,
    /// Ob die Bewegung je Matrix normiert wird, bevor die Rate greift.
    ///
    /// ⚑ **Der Unterschied ist, was eine Lernrate bedeutet.** Ohne
    /// Normierung hängt sie an der Gradientenskala des Modells und
    /// bedeutet auf jedem etwas anderes (Fund 194); mit ihr sagt
    /// `lr_nenner`, in wie vielen Aktualisierungen das grösste Gewicht
    /// einer Matrix eine Rasterstufe wandert.
    normiert: bool,
}

impl Sammlung {
    pub fn neu() -> Self {
        Self::default()
    }

    /// Eine Sammlung, die vor dem Anwenden **normiert**.
    pub fn normiert() -> Self {
        Self { normiert: true, ..Self::default() }
    }

    /// Ob diese Sammlung normiert.
    pub fn ist_normiert(&self) -> bool {
        self.normiert
    }

    /// Die Zahl der eingegangenen Folgen.
    pub fn laeufe(&self) -> u32 {
        self.laeufe
    }

    /// Wie viele Matrizen bisher berührt wurden.
    ///
    /// ⚑ Bei einer Gemischebene wächst diese Zahl mit den **gewählten**
    /// Experten und nicht mit allen vorhandenen.
    pub fn matrizen(&self) -> usize {
        self.summen.len()
    }

    /// Sagt an, dass eine weitere Folge eingegangen ist.
    pub fn folge_fertig(&mut self) {
        self.laeufe += 1;
    }

    fn addiere(&mut self, i: usize, k: Matrixkennung, versatz: u64, grad: &[i32], nenner: i64) {
        let normiert = self.normiert;
        let (_, ziel) = self
            .summen
            .entry((i, k))
            .or_insert_with(|| (versatz, vec![0i64; grad.len()]));
        // ⚑ **Beim Normieren wird der rohe Gradient gesammelt.** Wer
        // die Rate vorher anwendete, teilte durch eine Zahl und
        // normierte danach wieder weg, was er geteilt hat.
        if normiert {
            sammle_roh(ziel, grad);
        } else {
            sammle(ziel, grad, 1, nenner);
        }
    }
}

/// Wie ein Rückwärtslauf die Gewichte fortschreibt.
pub enum Fortschreibung<'a> {
    /// Nach jeder Folge, wie bisher.
    Sofort,
    /// Erst sammeln; angewandt wird später mit [`sammlung_anwenden`].
    Sammeln(&'a mut Sammlung),
}

impl Fortschreibung<'_> {
    fn tue(
        &mut self,
        i: usize,
        k: Matrixkennung,
        mm: &mut [Master],
        gg: &[i32],
        kn: Schrittkennung,
        nenner: i64,
    ) {
        match self {
            Self::Sofort => schritt(mm, gg, kn, 1, nenner),
            Self::Sammeln(s) => s.addiere(i, k, kn.index_versatz, gg, nenner),
        }
    }
}

impl Clone for Ebenenstand {
    fn clone(&self) -> Self {
        match self {
            Self::Dicht(m) => Self::Dicht(m.clone()),
            Self::Gemisch { aufmerksamkeit, router, experten } => Self::Gemisch {
                aufmerksamkeit: aufmerksamkeit.clone(),
                router: router.clone(),
                experten: experten.clone(),
            },
        }
    }
}

impl Ebenenstand {
    /// Die Matrix zu einer Kennung, zum Schreiben.
    ///
    /// ⚑ **`None` heisst „gibt es hier nicht" und nicht „Fehler".** Ein
    /// Experte, der in einem Durchgang gewählt wurde und in einem
    /// anderen nicht, fehlt zu Recht; wer hier abbräche, machte aus
    /// einer gewöhnlichen Lage einen Ausfall.
    pub fn matrix_mut(&mut self, k: Matrixkennung) -> Option<&mut Vec<Master>> {
        match (self, k) {
            (Self::Dicht(m), Matrixkennung::Dicht(n)) => m.get_mut(n as usize),
            (Self::Gemisch { aufmerksamkeit, .. }, Matrixkennung::Aufmerksamkeit(n)) => {
                aufmerksamkeit.get_mut(n as usize)
            }
            (Self::Gemisch { router, .. }, Matrixkennung::Router) => Some(router),
            (Self::Gemisch { experten, .. }, Matrixkennung::Experte(nr, n)) => {
                experten.get_mut(&nr).and_then(|drei| drei.get_mut(n as usize))
            }
            _ => None,
        }
    }

    /// Alle Matrizen dieser Ebene in **kanonischer** Reihenfolge.
    ///
    /// ⚑ **Die Reihenfolge ist Teil des Abdrucks**, also eine
    /// Konsensgrösse: erst die Aufmerksamkeit, dann der dichte Block
    /// oder Router und Experten nach ihrer **Nummer**. Wer sie nach
    /// Auswahlreihenfolge nähme, bekäme für dieselbe Arbeit
    /// verschiedene Abdrücke, je nachdem, welcher Experte zuerst
    /// drankam.
    pub fn matrizen(&self) -> Vec<&[Master]> {
        match self {
            Self::Dicht(m) => m.iter().map(|v| v.as_slice()).collect(),
            Self::Gemisch { aufmerksamkeit, router, experten } => {
                let mut aus: Vec<&[Master]> =
                    aufmerksamkeit.iter().map(|v| v.as_slice()).collect();
                aus.push(router.as_slice());
                for drei in experten.values() {
                    aus.extend(drei.iter().map(|v| v.as_slice()));
                }
                aus
            }
        }
    }

    /// Ist diese Ebene ein Expertengemisch?
    pub fn ist_gemisch(&self) -> bool {
        matches!(self, Self::Gemisch { .. })
    }
}

impl Shardgewichte {
    /// Liest die Gewichte des Bereichs aus dem Modell.
    pub fn aus_modell(m: &IntegerModel, von: usize, bis: usize) -> Result<Self, Shardfehler> {
        if von >= bis || bis > m.num_layers {
            return Err(Shardfehler::BereichUngueltig { von, bis, ebenen: m.num_layers });
        }
        let mut master = Vec::with_capacity(bis - von);
        for ebene in m.layers.iter().take(bis).skip(von) {
            master.push(match &ebene.ffn {
                Feedforward::Dense(_) => Ebenenstand::Dicht(Box::new(
                    master_der_ebene(ebene).expect("dichte Ebene hat sieben Matrizen"),
                )),
                // ⚑ **Die Experten bleiben leer**, siehe [`Ebenenstand`]:
                // Sie kommen dazu, wenn der Router sie wählt.
                Feedforward::Moe(moe) => Ebenenstand::Gemisch {
                    aufmerksamkeit: Box::new([
                        master_aus_gewicht(&ebene.q_proj),
                        master_aus_gewicht(&ebene.k_proj),
                        master_aus_gewicht(&ebene.v_proj),
                        master_aus_gewicht(&ebene.o_proj),
                    ]),
                    router: master_aus_gewicht(&moe.router),
                    experten: std::collections::BTreeMap::new(),
                },
            });
        }
        Ok(Self { anfang: master.clone(), master, von, bis })
    }

    /// Eine Kopie des Standes, für Messungen, die nichts verändern
    /// dürfen.
    ///
    /// ⚑ **Ein Abdruck darf den Stand nicht bewegen.** Wer ihn auf dem
    /// laufenden Stand bildet und dabei einen Schritt rechnet, misst
    /// etwas anderes, als er meldet.
    pub fn clone_stand(&self) -> Self {
        Self {
            master: self.master.clone(),
            anfang: self.anfang.clone(),
            von: self.von,
            bis: self.bis,
        }
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
    pub fn anfangsstand(&self) -> &[Ebenenstand] {
        &self.anfang
    }

    /// Das Δ dieses Shards, Matrix für Matrix, in kanonischer
    /// Reihenfolge.
    ///
    /// # ⚑ Warum das hier steht und nicht beim Aufrufer
    ///
    /// Pod und Prüfer bilden denselben Abdruck; täte es jeder für sich,
    /// wäre eine Abweichung nicht von einem Rechenfehler zu
    /// unterscheiden. Bei einem Gemisch kommt hinzu, dass **beide
    /// Seiten dieselben Experten in derselben Ordnung** durchlaufen
    /// müssen, und die Ordnung ist die der Expertennummern.
    ///
    /// ⚑ **Ein Experte, den nur eine Seite gewählt hat, fällt auf.** Er
    /// steht dann in der einen Liste und in der anderen nicht, und die
    /// Abdrücke gehen auseinander. Das ist richtig so: Verschiedene
    /// Experten heisst verschiedene Arbeit.
    pub fn deltas(&self) -> Vec<Vec<Master>> {
        self.anfang
            .iter()
            .zip(self.master.iter())
            .flat_map(|(a, b)| {
                let (av, bv) = (a.matrizen(), b.matrizen());
                // ⚑ Ein neu hinzugekommener Experte hat im Anfangsstand
                // kein Gegenstück. Sein Δ ist dann sein voller Stand
                // gegen null, und genau das drückt `delta` gegen eine
                // Nullmatrix aus.
                let leer: Vec<Master> = Vec::new();
                bv.into_iter()
                    .enumerate()
                    .map(|(i, y)| {
                        let x = av.get(i).copied().unwrap_or(&leer);
                        if x.len() == y.len() {
                            delta(x, y)
                        } else {
                            y.to_vec()
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Wie viele Gewichte der Shard insgesamt fortschreibt.
    pub fn gewichte_gesamt(&self) -> usize {
        self.master.iter().flat_map(|e| e.matrizen()).map(|m| m.len()).sum()
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
    pub spuren: Vec<Ebenenmitschnitt>,
    /// Der Ausgang des Bereichs, also der Eingang des nächsten Shards.
    pub ausgang: Vec<Vec<i16>>,
}

/// Die Spur **einer** Ebene, dicht oder als Expertengemisch.
pub enum Ebenenmitschnitt {
    /// Die Spur einer dichten Ebene, wie der Kernel sie liefert.
    Dicht(Box<Ebenenspur>),
    /// Die Spur einer Gemischebene.
    Gemisch(Box<Gemischebenenspur>),
}

/// Was eine Gemischebene vorwärts hinterlässt.
///
/// ⚑ **Dieselben Felder wie [`Ebenenspur`], nur ist der Block ein
/// anderer.** Aufmerksamkeit, beide Normierungen und der Residualstrom
/// verhalten sich in einer Gemischebene genau wie in einer dichten; wer
/// sie hier anders rechnete, bekäme zwei Ebenenarten, die sich nicht
/// nur im Feedforward unterscheiden.
#[derive(Default)]
pub struct Gemischebenenspur {
    /// Die erste Normierung je Position.
    pub norm_ein: Vec<Vec<i16>>,
    /// Ihre Zwischenwerte.
    pub norm_ein_spur: Vec<integer_llm_kernels::rmsnorm::Rmsnormspur>,
    /// Die Spur des Aufmerksamkeitsblocks.
    pub aufmerksamkeit: integer_llm_kernels::trainingsschritt::Aufmerksamkeitsspur,
    /// Der Residualstrom nach der ersten Addition.
    pub residual: Vec<Vec<i16>>,
    /// Die zweite Normierung je Position.
    pub norm_mitte: Vec<Vec<i16>>,
    /// Ihre Zwischenwerte.
    pub norm_mitte_spur: Vec<integer_llm_kernels::rmsnorm::Rmsnormspur>,
    /// Was das Gemisch je Position getan hat.
    pub gemisch: Vec<Gemischteil>,
    /// Der Ausgang der Ebene je Position.
    pub y: Vec<Vec<i16>>,
}

/// Was das Expertengemisch **an einer Position** getan hat.
///
/// ⚑ **Je Position eine eigene Auswahl**, und darin liegt der ganze
/// Unterschied zum dichten Block: Bei 128 Experten und Top-8 wählt jede
/// Position ihre eigenen acht, und der Rückwärtsweg muss wissen,
/// **welche** es waren und **mit welchem Gewicht**. Nur sie haben einen
/// Gradienten.
pub struct Gemischteil {
    /// Die gewählten Experten, in Auswahlreihenfolge.
    pub experten: Vec<u16>,
    /// Die Mischgewichte, in derselben Folge.
    pub gewichte: Vec<i32>,
    /// Die Ausgabe jedes gewählten Experten.
    pub ausgaben: Vec<Vec<i16>>,
    /// Die Zwischenwerte jedes gewählten Experten.
    pub teile: Vec<integer_llm_kernels::mlp::Mlpspur>,
    /// Die Routerlogits über alle Experten.
    pub logits: Vec<i32>,
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
    /// Gewichtsstand und Modell beschreiben verschiedene Ebenenarten.
    ///
    /// ⚑ **Das ist ein Fehler des Aufrufers, kein fehlendes Merkmal.**
    /// Gemischebenen trägt der Shardweg seit dem 2026-09-06. Diese
    /// Meldung kommt, wenn ein Gewichtsstand aus einem anderen Modell
    /// stammt als die Spur, und dann wäre jede Rechnung darauf sinnlos.
    ArtPasstNicht { ebene: usize },
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
            Self::ArtPasstNicht { ebene } => write!(
                f,
                "der Gewichtsstand der Ebene {ebene} passt nicht zu ihrer Art im Modell"
            ),
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
    g: &mut Shardgewichte,
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
        let tab = Ebenentabellen {
            cos: &m.cos_lut,
            sin: &m.sin_lut,
            exp: &m.exp_lut,
            rsqrt: &m.rsqrt_lut,
            silu: &m.silu_lut,
            silu_grad: &grad_lut,
        };
        let spur = match &ebene.ffn {
            Feedforward::Dense(mlp) => {
                let is = mlp.gate_proj.shape[0];
                let breiten = breiten_der_ebene(m, is);
                let Ebenenstand::Dicht(master) = &g.master[i] else {
                    return Err(Shardfehler::ArtPasstNicht { ebene: e });
                };
                let umgerechnet: Vec<(Vec<i8>, Vec<u8>)> = (0..7)
                    .map(|n| gewicht_aus_master(&master[n], breiten[n], MASTER_FRAC))
                    .collect();
                let gew = gewichte_der_ebene(&umgerechnet, ebene);
                let vg = vorgaben_der_ebene(
                    m, &ebene.scales, e, is, v.schritt, v.lr_zaehler, v.lr_nenner,
                );
                Ebenenmitschnitt::Dicht(Box::new(vorwaerts_der_ebene(
                    gew,
                    &strom,
                    vorspannungen_der_ebene(ebene),
                    tab,
                    vg,
                )))
            }
            Feedforward::Moe(moe) => Ebenenmitschnitt::Gemisch(Box::new(
                gemisch_vorwaerts(m, ebene, moe, &mut g.master[i], e, v, &strom, tab)?,
            )),
        };
        let y = match &spur {
            Ebenenmitschnitt::Dicht(s) => s.y.clone(),
            Ebenenmitschnitt::Gemisch(s) => s.y.clone(),
        };
        mitschnitt.eingaenge.push(std::mem::replace(&mut strom, y));
        mitschnitt.spuren.push(spur);
    }
    mitschnitt.ausgang = strom;
    Ok(mitschnitt)
}

/// Der Vorwärtslauf **einer** Gemischebene über die ganze Folge.
///
/// # ⚑ Aufbau, und warum er dem der dichten Ebene folgen muss
///
/// Normierung, Aufmerksamkeit, Residualaddition, zweite Normierung,
/// Block, zweite Residualaddition. Nur der Block ist ein anderer. Wer
/// hier abwiche, bekäme zwei Ebenenarten, die sich nicht nur im
/// Feedforward unterscheiden, und ein Modell mit beiden liefe
/// auseinander.
///
/// ⚑ **Die Experten werden hier materialisiert, nicht vorher.** Erst
/// der Router sagt, wer gebraucht wird; wer alle 128 vorhielte, hielte
/// 2,4 GB je Ebene.
#[allow(clippy::too_many_arguments)]
fn gemisch_vorwaerts(
    m: &IntegerModel,
    ebene: &crate::model::TransformerLayer,
    moe: &crate::model::MoeLayer,
    stand: &mut Ebenenstand,
    e: usize,
    v: &Shardvorgaben,
    hidden: &[Vec<i16>],
    tab: Ebenentabellen<'_>,
) -> Result<Gemischebenenspur, Shardfehler> {
    use integer_llm_kernels::fixed_point::rescale_i64;
    use integer_llm_kernels::mlp::{mlp_int_mit_spur, Mlpspur};
    use integer_llm_kernels::moe::{mische_experten, route_top_k};
    use integer_llm_kernels::trainingsschritt::vorwaerts_der_aufmerksamkeit;

    let Ebenenstand::Gemisch { aufmerksamkeit, router, experten } = stand else {
        return Err(Shardfehler::ArtPasstNicht { ebene: e });
    };
    let sc = &ebene.scales;
    let cfg = &m.config;
    let hs = m.hidden_size;
    let is = moe.experts[0].gate_proj.shape[0];
    let vg = vorgaben_der_ebene(m, sc, e, is, v.schritt, v.lr_zaehler, v.lr_nenner);
    let (a_vorgaben, m_vorgaben) = vg.bloecke();
    let acc_attn = vg.acc_attn();
    let acc_mlp = vg.acc_mlp();
    let aus_frac = vg.aus_frac;
    let mut spur = Gemischebenenspur::default();

    // 1. Erste Normierung je Position.
    for h in hidden.iter() {
        let mut ns = integer_llm_kernels::rmsnorm::Rmsnormspur::Leer;
        let n = integer_llm_kernels::rmsnorm::rmsnorm_i16_mit_spur(
            h,
            &sc.residual_in_frac,
            &ebene.input_layernorm_gamma.data,
            &ebene.input_layernorm_gamma.shifts,
            tab.rsqrt,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            m.inv_n_q20,
            a_vorgaben.act_frac,
            Some(&mut ns),
        );
        spur.norm_ein.push(n);
        spur.norm_ein_spur.push(ns);
    }

    // 2. Aufmerksamkeit, aus den Mastern.
    let a_breiten = [hs, hs, hs, m.num_heads * m.head_dim];
    let a_umgerechnet: Vec<(Vec<i8>, Vec<u8>)> = (0..4)
        .map(|n| gewicht_aus_master(&aufmerksamkeit[n], a_breiten[n], MASTER_FRAC))
        .collect();
    let a_gew = integer_llm_kernels::trainingsschritt::Aufmerksamkeitsgewichte {
        q: &a_umgerechnet[0].0,
        q_skalen: &a_umgerechnet[0].1,
        k: &a_umgerechnet[1].0,
        k_skalen: &a_umgerechnet[1].1,
        v: &a_umgerechnet[2].0,
        v_skalen: &a_umgerechnet[2].1,
        o: &a_umgerechnet[3].0,
        o_skalen: &a_umgerechnet[3].1,
    };
    spur.aufmerksamkeit = vorwaerts_der_aufmerksamkeit(
        a_gew,
        &spur.norm_ein,
        vorspannungen_der_ebene(ebene),
        tab.cos,
        tab.sin,
        tab.exp,
        Some(&acc_attn),
        a_vorgaben,
    );

    // 3. Residual, zweite Normierung, Gemisch, zweite Residualaddition.
    let exp_shift = moe.router_frac.saturating_sub(cfg.exp_input_frac);
    let (rw, rs) = gewicht_aus_master(router, hs, MASTER_FRAC);
    for (nr, h) in hidden.iter().enumerate() {
        let o_aus = &spur.aufmerksamkeit.y[nr];
        let mut residual = vec![0i16; hs];
        for i in 0..hs {
            let r = rescale_i64(i64::from(h[i]), sc.residual_in_frac[i], acc_attn[i]);
            let summe = r + i64::from(o_aus[i]);
            residual[i] = klemme_i16(rescale_i64(summe, acc_attn[i], sc.residual_mid_frac[i]));
        }
        let mut ns = integer_llm_kernels::rmsnorm::Rmsnormspur::Leer;
        let norm = integer_llm_kernels::rmsnorm::rmsnorm_i16_mit_spur(
            &residual,
            &sc.residual_mid_frac,
            &ebene.post_attention_layernorm_gamma.data,
            &ebene.post_attention_layernorm_gamma.shifts,
            tab.rsqrt,
            cfg.rsqrt_input_shift,
            cfg.rsqrt_output_frac,
            m.inv_n_q20,
            m_vorgaben.act_frac,
            Some(&mut ns),
        );

        // Der Router, aus dem Master.
        let logits: Vec<i32> = integer_llm_kernels::linear::linear_w8a16(
            &norm, &rw, hs, &rs, m_vorgaben.act_frac, moe.router_frac,
        )
        .iter()
        .map(|l| i32::from(*l))
        .collect();
        let routing = route_top_k(
            &logits, moe.top_k, tab.exp, exp_shift, cfg.prob_frac_bits, moe.norm_topk_prob,
        );

        // ⚑ **Erst hier kommt ein Experte zu seinem Master.**
        for i in &routing.experten {
            experten.entry(*i).or_insert_with(|| {
                let ex = &moe.experts[*i as usize];
                [
                    master_aus_gewicht(&ex.gate_proj),
                    master_aus_gewicht(&ex.up_proj),
                    master_aus_gewicht(&ex.down_proj),
                ]
            });
        }

        let mut teile: Vec<Mlpspur> = Vec::with_capacity(routing.experten.len());
        let mut ausgaben: Vec<Vec<i16>> = Vec::with_capacity(routing.experten.len());
        for i in &routing.experten {
            let mm = &experten[i];
            let (gw, gs) = gewicht_aus_master(&mm[0], hs, MASTER_FRAC);
            let (uw, us) = gewicht_aus_master(&mm[1], hs, MASTER_FRAC);
            let (dw, ds) = gewicht_aus_master(&mm[2], is, MASTER_FRAC);
            let mut sp = Mlpspur::default();
            let aus = mlp_int_mit_spur(
                &norm, &gw, &uw, &dw, hs, is, &gs, &us, &ds, tab.silu,
                m_vorgaben.act_frac, m_vorgaben.gate_frac, m_vorgaben.up_frac,
                m_vorgaben.down_in_frac, m_vorgaben.silu_in_frac, m_vorgaben.silu_lut_offset,
                m_vorgaben.silu_out_frac, &acc_mlp, Some(&mut sp),
            );
            ausgaben.push(aus);
            teile.push(sp);
        }
        let block = mische_experten(&ausgaben, &routing.gewichte, cfg.prob_frac_bits);

        let mut y = vec![0i16; hs];
        for i in 0..hs {
            let r = rescale_i64(i64::from(residual[i]), sc.residual_mid_frac[i], acc_mlp[i]);
            let summe = r + i64::from(block[i]);
            // ⚑ **Dieselbe Regel wie bei der dichten Ebene** (Fund
            // 188): der Ausgang liegt auf der Eingangsskala der
            // nächsten Ebene.
            y[i] = klemme_i16(rescale_i64(summe, acc_mlp[i], aus_frac[i]));
        }

        spur.residual.push(residual);
        spur.norm_mitte.push(norm);
        spur.norm_mitte_spur.push(ns);
        spur.gemisch.push(Gemischteil {
            experten: routing.experten,
            gewichte: routing.gewichte,
            ausgaben,
            teile,
            logits,
        });
        spur.y.push(y);
    }
    Ok(spur)
}

/// Klemmt einen `i64` auf `i16`.
///
/// ⚑ **Gesättigt statt umlaufend**, wie überall in diesem Projekt: Ein
/// umgelaufener Residualstrom wechselt das Vorzeichen, und die Ebene
/// rechnet ab da etwas anderes.
fn klemme_i16(v: i64) -> i16 {
    v.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
}

/// Wo die Matrizen einer Gemischebene im Würfelraum liegen.
///
/// # ⚑ Der Versatz ist Protokoll, nicht Buchhaltung
///
/// Der Würfel des stochastischen Rundens ist eine reine Funktion aus
/// `(ebene, schritt, index)`, und `index` zählt **innerhalb der Ebene**.
/// Die Aufteilung dieses Raums entscheidet also mit, welches Gewicht
/// aufgerundet wird. Zwei Rechner, die sie verschieden vornehmen,
/// bekommen verschiedene Δm bei gleicher Rechnung.
///
/// ```text
/// [0, a)                          Q, K, V, O
/// [a, a + hs·n)                   Router
/// [a + hs·n + i·3·hs·is, ...)     Experte i: Gate, Up, Down
/// ```
///
/// ⚑ **Der Versatz eines Experten hängt an seiner NUMMER**, nicht an der
/// Reihenfolge, in der er gewählt wurde. Sonst würfelte derselbe Experte
/// verschieden, je nachdem, an welcher Position er zuerst drankam, und
/// zwei Shards mit anderer Positionsverteilung liefen auseinander.
///
/// ⚑ **Und der Router steht VOR den Experten, nicht dahinter.** Die
/// Zahl der Experten ist fest, die der gewählten nicht; ein Router
/// hinter den gewählten Experten läge bei jedem Schritt woanders.
struct Gemischversatz {
    aufmerksamkeit: [u64; 4],
    router: u64,
    experten_basis: u64,
    je_experte: u64,
    je_matrix: u64,
}

impl Gemischversatz {
    fn neu(hs: usize, is: usize, kopfbreite: usize, anzahl_experten: usize) -> Self {
        let a = [
            0u64,
            (hs * hs) as u64,
            (hs * hs + hs * kopfbreite) as u64,
            (hs * hs + 2 * hs * kopfbreite) as u64,
        ];
        // ⚑ Q ist `hs × hs`, K und V sind `hs × kv_breite`, O ist
        // `kopfbreite × hs`. Die genauen Längen kommen aus den Mastern;
        // hier zählt nur, dass die Bereiche sich nicht überlappen.
        let nach_attn = a[3] + (kopfbreite * hs) as u64;
        Self {
            aufmerksamkeit: a,
            router: nach_attn,
            experten_basis: nach_attn + (hs * anzahl_experten) as u64,
            je_experte: (hs * is * 3) as u64,
            je_matrix: (hs * is) as u64,
        }
    }

    fn experte(&self, nummer: u16, matrix: usize) -> u64 {
        self.experten_basis
            + u64::from(nummer) * self.je_experte
            + matrix as u64 * self.je_matrix
    }
}

/// Der Rückwärtslauf eines Shards: Gradienten, Schritt, Weitergabe.
///
/// `g_aus` ist `dL/dZ` am **Ausgang** des Bereichs, je Position; bei
/// Shard *j* ist das, was Shard *j+1* als `eingang` zurückgab.
///
/// ⚑ **Die Ebenen laufen rückwärts**, von `bis−1` bis `von`, und der
/// Gradient wandert dabei durch: Was eine Ebene als `eingang` liefert,
/// ist der Ausgangsgradient der Ebene davor.
pub fn rueckwaerts(
    m: &IntegerModel,
    gew_stand: &mut Shardgewichte,
    v: &Shardvorgaben,
    mitschnitt: &Shardmitschnitt,
    g_aus: &[Vec<i32>],
) -> Result<Shardergebnis, Shardfehler> {
    rueckwaerts_mit(m, gew_stand, v, mitschnitt, g_aus, &mut Fortschreibung::Sofort)
}

/// Was ein angewandter Sammelschritt bewegt hat.
#[derive(Debug, Clone)]
pub struct Sammelergebnis {
    /// Abdruck über die Deltas, in kanonischer Reihenfolge.
    pub delta_commitment: String,
    /// Abdruck über den neuen Gewichtsstand.
    pub abdruck: String,
    /// Wie viele Master sich bewegt haben.
    pub bewegte_gewichte: usize,
    /// Wie viele Master insgesamt gehalten werden.
    pub gewichte_gesamt: usize,
    /// Welche Ebene die Übertragungsform verlassen hat, falls eine.
    pub aus_der_form: Option<usize>,
    /// Über wie viele Folgen gesammelt wurde.
    pub laeufe: u32,
}

/// Wendet eine [`Sammlung`] an: **ein** Wurf je Gewicht.
///
/// ⚑ **Der Schrittzähler ist der des Segments, nicht der der Folge.**
/// Ein Sammelschritt **ist** ein Schritt; nähme er die Nummer der
/// letzten Folge, hinge der Würfel daran, wie viele Folgen eingingen,
/// und zwei Pods mit gleicher Arbeit, aber anderer Aufteilung liefen
/// auseinander.
pub fn sammlung_anwenden(
    sammlung: &Sammlung,
    gew_stand: &mut Shardgewichte,
    v: &Shardvorgaben,
) -> Sammelergebnis {
    let mut aus_der_form: Option<usize> = None;
    let mut summen = sammlung.summen.clone();
    for ((i, k), (versatz, summe)) in summen.iter_mut() {
        let ebene_global = v.von + i;
        let Some(mm) = gew_stand.master[*i].matrix_mut(*k) else {
            // Eine Matrix, die es nicht mehr gibt, bekommt nichts.
            // Vorkommen kann das nur bei Experten, und dann ist die
            // Summe für einen Experten gedacht, der aus dem Stand
            // gefallen ist.
            continue;
        };
        if mm.len() != summe.len() {
            continue;
        }
        let kn = Schrittkennung {
            ebene: ebene_global as u32,
            schritt: v.schritt,
            index_versatz: *versatz,
        };
        if sammlung.normiert {
            schritt_normiert(mm, summe, kn, v.lr_nenner);
        } else {
            schritt_aus_summe(mm, summe, kn);
        }
        if aus_der_form.is_none() && ausserhalb_der_form(mm).is_some() {
            aus_der_form = Some(ebene_global);
        }
    }

    let deltas = gew_stand.deltas();
    let delta_scheiben: Vec<&[Master]> = deltas.iter().map(|v| v.as_slice()).collect();
    let flach: Vec<&[Master]> = gew_stand.master.iter().flat_map(|e| e.matrizen()).collect();
    let bewegte = deltas.iter().flat_map(|d| d.iter()).filter(|v| **v != 0).count();
    Sammelergebnis {
        delta_commitment: trainingsabdruck(&delta_scheiben),
        abdruck: trainingsabdruck(&flach),
        bewegte_gewichte: bewegte,
        gewichte_gesamt: gew_stand.gewichte_gesamt(),
        aus_der_form,
        laeufe: sammlung.laeufe,
    }
}

/// Wie [`rueckwaerts`], aber mit wählbarer Fortschreibung.
///
/// ⚑ **Bei `Sammeln` bleiben die Gewichte stehen.** Das Ergebnis trägt
/// dann einen Abdruck über **unveränderte** Gewichte und null bewegte
/// Gewichte; erst [`sammlung_anwenden`] bewegt etwas. Wer den Abdruck
/// eines Sammellaufs einreichte, reichte den Ausgangsstand ein.
pub fn rueckwaerts_mit(
    m: &IntegerModel,
    gew_stand: &mut Shardgewichte,
    v: &Shardvorgaben,
    mitschnitt: &Shardmitschnitt,
    g_aus: &[Vec<i32>],
    fort: &mut Fortschreibung<'_>,
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
        let tab = Ebenentabellen {
            cos: &m.cos_lut,
            sin: &m.sin_lut,
            exp: &m.exp_lut,
            rsqrt: &m.rsqrt_lut,
            silu: &m.silu_lut,
            silu_grad: &grad_lut,
        };
        g = match (&ebene.ffn, &mitschnitt.spuren[i]) {
            (Feedforward::Dense(mlp), Ebenenmitschnitt::Dicht(spur)) => {
                let is = mlp.gate_proj.shape[0];
                let breiten = breiten_der_ebene(m, is);
                let Ebenenstand::Dicht(master) = &gew_stand.master[i] else {
                    return Err(Shardfehler::ArtPasstNicht { ebene: e });
                };
                let umgerechnet: Vec<(Vec<i8>, Vec<u8>)> = (0..7)
                    .map(|n| gewicht_aus_master(&master[n], breiten[n], MASTER_FRAC))
                    .collect();
                let gew = gewichte_der_ebene(&umgerechnet, ebene);
                let vg = vorgaben_der_ebene(
                    m, &ebene.scales, e, is, v.schritt, v.lr_zaehler, v.lr_nenner,
                );
                let gr = gradienten_der_ebene_aus_gradient(
                    gew,
                    spur,
                    &mitschnitt.eingaenge[i],
                    &g,
                    tab,
                    vg,
                );
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
                let Ebenenstand::Dicht(master) = &mut gew_stand.master[i] else {
                    unreachable!("gerade als dicht erkannt")
                };
                let mut versatz = 0u64;
                for (n, (mm, gg)) in master.iter_mut().zip(gradienten.iter()).enumerate() {
                    let laenge = mm.len() as u64;
                    fort.tue(
                        i,
                        Matrixkennung::Dicht(n as u8),
                        mm,
                        gg,
                        Schrittkennung { index_versatz: versatz, ..kn },
                        v.lr_nenner,
                    );
                    versatz += laenge;
                }
                gr.eingang
            }
            (Feedforward::Moe(moe), Ebenenmitschnitt::Gemisch(spur)) => gemisch_rueckwaerts(
                m,
                ebene,
                moe,
                &mut gew_stand.master[i],
                e,
                i,
                v,
                spur,
                &mitschnitt.eingaenge[i],
                &g,
                tab,
                fort,
            )?,
            _ => return Err(Shardfehler::ArtPasstNicht { ebene: e }),
        };
        if aus_der_form.is_none()
            && gew_stand.master[i].matrizen().iter().any(|mm| ausserhalb_der_form(mm).is_some())
        {
            aus_der_form = Some(e);
        }
    }

    let deltas = gew_stand.deltas();
    let delta_scheiben: Vec<&[Master]> = deltas.iter().map(|v| v.as_slice()).collect();
    let flach: Vec<&[Master]> =
        gew_stand.master.iter().flat_map(|e| e.matrizen()).collect();
    let bewegte_gewichte = deltas.iter().flat_map(|d| d.iter()).filter(|v| **v != 0).count();

    Ok(Shardergebnis {
        eingang: g,
        eingang_frac: m.layers[v.von].scales.residual_in_frac.clone(),
        delta_commitment: trainingsabdruck(&delta_scheiben),
        abdruck: trainingsabdruck(&flach),
        bewegte_gewichte,
        gewichte_gesamt: gew_stand.gewichte_gesamt(),
        aus_der_form,
    })
}

/// Der Rückwärtslauf **einer** Gemischebene.
///
/// # ⚑ Drei Zweige treffen sich am Eingang des Blocks
///
/// Ein dichter Block hat einen Weg vom Ausgang zum Eingang. Ein Gemisch
/// hat drei: über die **gewählten Experten**, über die **Mischgewichte**
/// zurück in die Routerlogits, und über die **Routerprojektion**.
/// `gradienten_des_gemisches` führt sie zusammen; hier steht der Rahmen
/// darum, also die Normierungen, die Aufmerksamkeit und der
/// Residualstrom.
#[allow(clippy::too_many_arguments)]
fn gemisch_rueckwaerts(
    m: &IntegerModel,
    ebene: &crate::model::TransformerLayer,
    moe: &crate::model::MoeLayer,
    stand: &mut Ebenenstand,
    e: usize,
    i: usize,
    v: &Shardvorgaben,
    spur: &Gemischebenenspur,
    hidden: &[Vec<i16>],
    g_aus: &[Vec<i32>],
    tab: Ebenentabellen<'_>,
    fort: &mut Fortschreibung<'_>,
) -> Result<Vec<Vec<i32>>, Shardfehler> {
    use integer_llm_kernels::fixed_point::rescale_i64;
    use integer_llm_kernels::trainingsschritt::{
        begrenze, gradienten_der_aufmerksamkeit_aus_gradient, gradienten_des_gemisches,
        normrueckwaerts, Expertengewichte, Gemischvorgaben,
    };
    use std::collections::BTreeMap;

    let Ebenenstand::Gemisch { aufmerksamkeit, router, experten } = stand else {
        return Err(Shardfehler::ArtPasstNicht { ebene: e });
    };
    let sc = &ebene.scales;
    let cfg = &m.config;
    let hs = m.hidden_size;
    let is = moe.experts[0].gate_proj.shape[0];
    let n = moe.experts.len();
    let vg = vorgaben_der_ebene(m, sc, e, is, v.schritt, v.lr_zaehler, v.lr_nenner);
    let (a_vorgaben, m_vorgaben) = vg.bloecke();
    let aus_frac = vg.aus_frac;
    let a_bus = a_vorgaben.aus_frac;
    let m_bus = m_vorgaben.aus_frac;
    let mid_bus = sc.residual_mid_frac.iter().copied().max().unwrap_or(0);
    let in_bus = sc.residual_in_frac.iter().copied().max().unwrap_or(0);
    let gv = Gemischvorgaben {
        anzahl_experten: n,
        gewicht_frac: cfg.prob_frac_bits,
        // ⚑ **Dieselben sechs Bits wie in der Gemischschleife.** Der
        // Routergradient trägt `p·(1−p)`; auf der Skala des
        // Aktivierungsgradienten rundet er auf null, bevor er wirkt.
        logit_zusatz_bits: 6,
    };
    let (rw, rs) = gewicht_aus_master(router, hs, MASTER_FRAC);

    let mut g_router: Vec<i64> = vec![0; router.len()];
    let mut g_experten: BTreeMap<u16, [Vec<i64>; 3]> = BTreeMap::new();
    let mut g_residual: Vec<Vec<i32>> = Vec::with_capacity(hidden.len());

    for (nr, g_y) in g_aus.iter().enumerate() {
        let teil = &spur.gemisch[nr];
        // Die Experten dieser Position, quantisiert aus ihren Mastern.
        let halde: Vec<QuantisierterExperte> = teil
            .experten
            .iter()
            .map(|i| {
                let mm = &experten[i];
                let (gw, gs) = gewicht_aus_master(&mm[0], hs, MASTER_FRAC);
                let (uw, us) = gewicht_aus_master(&mm[1], hs, MASTER_FRAC);
                let (dw, ds) = gewicht_aus_master(&mm[2], is, MASTER_FRAC);
                (gw, gs, uw, us, dw, ds)
            })
            .collect();
        let ew: Vec<Expertengewichte<'_>> = halde
            .iter()
            .map(|(g, gs, u, us, d, ds)| Expertengewichte {
                gate: g,
                gate_skalen: gs,
                up: u,
                up_skalen: us,
                down: d,
                down_skalen: ds,
            })
            .collect();

        // Zweite Residualaddition: beide Zweige bekommen `g_y`, der
        // Block auf seinem Bus.
        let g_block: Vec<i32> = g_y
            .iter()
            .enumerate()
            .map(|(i, x)| {
                begrenze(rescale_i64(i64::from(*x), aus_frac[i], m_bus))
            })
            .collect();
        let gr = gradienten_des_gemisches(
            &g_block,
            &spur.norm_mitte[nr],
            &teil.experten,
            &teil.gewichte,
            &teil.ausgaben,
            &teil.teile,
            &ew,
            &rw,
            &rs,
            tab.silu,
            tab.silu_grad,
            m_vorgaben,
            gv,
        );
        for (z, p) in g_router.iter_mut().zip(gr.router.iter()) {
            *z = z.saturating_add(i64::from(*p));
        }
        for (i, mg) in &gr.experten {
            let ziel = g_experten
                .entry(*i)
                .or_insert_with(|| [vec![0i64; hs * is], vec![0i64; hs * is], vec![0i64; hs * is]]);
            for (z, p) in ziel[0].iter_mut().zip(mg.gate.iter()) {
                *z = z.saturating_add(i64::from(*p));
            }
            for (z, p) in ziel[1].iter_mut().zip(mg.up.iter()) {
                *z = z.saturating_add(i64::from(*p));
            }
            for (z, p) in ziel[2].iter_mut().zip(mg.down.iter()) {
                *z = z.saturating_add(i64::from(*p));
            }
        }

        // Durch die zweite Normierung, dann beide Zweige der ersten
        // Addition auf die Kanalskala des Residualstroms.
        let g_norm = normrueckwaerts(
            &gr.eingang,
            &spur.residual[nr],
            &sc.residual_mid_frac,
            &ebene.post_attention_layernorm_gamma.data,
            &ebene.post_attention_layernorm_gamma.shifts,
            &spur.norm_mitte_spur[nr],
            m.inv_n_q20,
            m_vorgaben.act_frac,
            mid_bus,
        );
        let zeile: Vec<i32> = g_y
            .iter()
            .zip(g_norm.iter())
            .enumerate()
            .map(|(i, (a, b))| {
                begrenze(
                    rescale_i64(i64::from(*a), aus_frac[i], sc.residual_mid_frac[i])
                        + rescale_i64(i64::from(*b), mid_bus, sc.residual_mid_frac[i]),
                )
            })
            .collect();
        g_residual.push(zeile);
    }

    // Durch den Aufmerksamkeitsblock.
    let a_breiten = [hs, hs, hs, m.num_heads * m.head_dim];
    let a_umgerechnet: Vec<(Vec<i8>, Vec<u8>)> = (0..4)
        .map(|n| gewicht_aus_master(&aufmerksamkeit[n], a_breiten[n], MASTER_FRAC))
        .collect();
    let a_gew = integer_llm_kernels::trainingsschritt::Aufmerksamkeitsgewichte {
        q: &a_umgerechnet[0].0,
        q_skalen: &a_umgerechnet[0].1,
        k: &a_umgerechnet[1].0,
        k_skalen: &a_umgerechnet[1].1,
        v: &a_umgerechnet[2].0,
        v_skalen: &a_umgerechnet[2].1,
        o: &a_umgerechnet[3].0,
        o_skalen: &a_umgerechnet[3].1,
    };
    let g_attn: Vec<Vec<i32>> = g_residual
        .iter()
        .map(|z| {
            z.iter()
                .enumerate()
                .map(|(i, x)| begrenze(rescale_i64(i64::from(*x), sc.residual_mid_frac[i], a_bus)))
                .collect()
        })
        .collect();
    let a_grad = gradienten_der_aufmerksamkeit_aus_gradient(
        &g_attn,
        &spur.norm_ein,
        &spur.aufmerksamkeit,
        a_gew,
        tab.cos,
        tab.sin,
        a_vorgaben,
    );

    // Durch die erste Normierung und die erste Residualaddition.
    let mut eingang: Vec<Vec<i32>> = Vec::with_capacity(hidden.len());
    for nr in 0..hidden.len() {
        let g_norm = normrueckwaerts(
            &a_grad.eingang[nr],
            &hidden[nr],
            &sc.residual_in_frac,
            &ebene.input_layernorm_gamma.data,
            &ebene.input_layernorm_gamma.shifts,
            &spur.norm_ein_spur[nr],
            m.inv_n_q20,
            a_vorgaben.act_frac,
            in_bus,
        );
        let zeile: Vec<i32> = g_residual[nr]
            .iter()
            .zip(g_norm.iter())
            .enumerate()
            .map(|(i, (a, b))| {
                begrenze(
                    rescale_i64(i64::from(*a), sc.residual_mid_frac[i], sc.residual_in_frac[i])
                        + rescale_i64(i64::from(*b), in_bus, sc.residual_in_frac[i]),
                )
            })
            .collect();
        eingang.push(zeile);
    }

    // ⚑ **Der Schritt, mit dem festgelegten Versatz.**
    let versatz = Gemischversatz::neu(hs, is, m.num_heads * m.head_dim, n);
    let kn = vg.aufmerksamkeit.kennung;
    let a_gradienten = [&a_grad.q, &a_grad.k, &a_grad.v, &a_grad.o];
    for (nr, (mm, gg)) in aufmerksamkeit.iter_mut().zip(a_gradienten.iter()).enumerate() {
        fort.tue(
            i,
            Matrixkennung::Aufmerksamkeit(nr as u8),
            mm,
            gg,
            Schrittkennung { index_versatz: versatz.aufmerksamkeit[nr], ..kn },
            v.lr_nenner,
        );
    }
    let g_router_i32: Vec<i32> = g_router.iter().copied().map(begrenze).collect();
    fort.tue(
        i,
        Matrixkennung::Router,
        router,
        &g_router_i32,
        Schrittkennung { index_versatz: versatz.router, ..kn },
        v.lr_nenner,
    );
    for (nummer, drei) in &g_experten {
        let mm = experten.get_mut(nummer).expect("gewaehlt, also vorhanden");
        for (nr, teil) in drei.iter().enumerate() {
            let g32: Vec<i32> = teil.iter().copied().map(begrenze).collect();
            fort.tue(
                i,
                Matrixkennung::Experte(*nummer, nr as u8),
                &mut mm[nr],
                &g32,
                Schrittkennung { index_versatz: versatz.experte(*nummer, nr), ..kn },
                v.lr_nenner,
            );
        }
    }

    Ok(eingang)
}

