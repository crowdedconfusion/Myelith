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
//! myelith-30b-a3b hat 128 Experten je Ebene zu je 4,7 Millionen
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
    ausserhalb_der_form, delta, sammle, sammle_roh, schritt, schritt_aus_summe,
    schritt_normiert_je_zeile,
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

/// Eine Ebene mit ihren Matrizen vorher und jetzt.
///
/// `(globale Ebenennummer, Anfangsstand je Matrix, jetziger Stand je
/// Matrix)`, beide in kanonischer Reihenfolge und **gleich lang**.
/// Siehe [`Shardgewichte::paare_mit_anfang`].
pub type Matrixpaare<'a> = (usize, Vec<Vec<Master>>, Vec<&'a [Master]>);

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
/// Das myelith-30b-a3b hat 128 Experten je Ebene zu je 4,7 Millionen
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
    /// Je Matrixkennung ihre Zeilenbreite (`in_features`), falls
    /// bekannt.
    ///
    /// # ⚑ Warum sie nicht aus der Summe folgt
    ///
    /// Eine flache Matrix sagt ihre Laenge, nicht ihre Form. Wer die
    /// Zeilenbreite raet, raet bei quadratischen Matrizen richtig und
    /// bei `[4864, 896]` falsch, und der Fehler faellt nirgends auf:
    /// Die Rechnung geht durch und normiert auf eine erfundene Zeile.
    ///
    /// 📌 **Leer heisst: wie bisher, je Matrix.** Das ist die
    /// Vorgabe, damit ein Aufrufer, der nichts sagt, das bisherige
    /// Verhalten bekommt und keine stille Aenderung.
    zeilenbreiten: std::collections::BTreeMap<Matrixkennung, usize>,
    /// Welche Matrizen ueberhaupt bewegt werden (Parameterisolierung).
    auswahl: Auswahl,
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

    /// ⚑ **Momentum ueber die Summen, als Messwerkzeug (2026-09-08).**
    ///
    /// Faltet einen Puffer ein, der zwischen den Durchgaengen lebt:
    /// `m = m − m/2^schub + g`, danach ist `m` die neue Summe. Alles in
    /// Ganzzahlen, also **exakt und reihenfolgeunabhaengig**; in
    /// Gleitkomma driftete ein Momentum mit der Summierungsreihenfolge
    /// und koennte in einem geshardeten Netz gar nicht in den Konsens.
    ///
    /// ⚑ **Warum es beide Befunde vom 2026-09-08 zugleich adressiert:**
    /// Der Gradient einer **wiederholten** Tatsache zeigt jedes Mal in
    /// dieselbe Richtung und summiert sich auf; der Gradient breiten
    /// Textes zeigt wechselnd und mittelt sich weg. Fund 198 hat beides
    /// gemessen, den zu langsamen Aufbau und die Erosion ab Durchgang
    /// acht.
    ///
    /// 📌 **Kein Konsensweg.** Diese Funktion aendert `schritt_normiert`
    /// nicht und wird ausschliesslich vom Messwerkzeug gerufen. Ob
    /// Momentum in den Trainingsvertrag gehoert, ist eine Entscheidung
    /// und keine Messung.
    pub fn momentum_falten(
        &mut self,
        puffer: &mut std::collections::BTreeMap<(usize, Matrixkennung), Vec<i64>>,
        schub: u32,
    ) {
        for (schluessel, (_, summe)) in self.summen.iter_mut() {
            let m = puffer
                .entry(*schluessel)
                .or_insert_with(|| vec![0i64; summe.len()]);
            if m.len() != summe.len() {
                m.resize(summe.len(), 0);
            }
            let teiler = 1i64 << schub;
            for (mi, gi) in m.iter_mut().zip(summe.iter()) {
                // 📌 **Division und NICHT Rechtsschieben (2026-09-08).**
                // Die erste Fassung nahm `*mi >> schub`. Arithmetisches
                // Rechtsschieben rundet Richtung minus unendlich und ist
                // damit **unsymmetrisch**: `-1 >> 3` ist `-1`, also
                // zerfaellt eine kleine negative Zahl vollstaendig,
                // waehrend `1 >> 3` null ist und eine kleine positive
                // unveraendert bleibt. Ueber Millionen Gewichte war das
                // kein Momentum, sondern ein **Gleichrichter** mit
                // systematischer Drift ins Positive; der Lauf M2 wurde
                // dadurch in allen Zahlen schlechter als der ohne
                // Puffer. Rust trunkiert bei Division Richtung null,
                // also symmetrisch.
                *mi = mi.saturating_sub(*mi / teiler).saturating_add(*gi);
            }
            summe.copy_from_slice(m);
        }
    }

    /// ⚑ **Vorzeichenabstieg, als Messwerkzeug (Eskalationsstufe T2).**
    ///
    /// Ersetzt jede Gradientensumme durch ihr **Vorzeichen**, skaliert
    /// auf einen festen Betrag. Weil [`schritt_normiert`] danach durch
    /// das Betragsmaximum teilt und alle Betraege gleich sind, bewegt
    /// sich **jedes** Gewicht um denselben Schritt, nur in der Richtung
    /// seines Gradienten.
    ///
    /// # ⚑ Warum das gerade hier naheliegt
    ///
    /// Am 2026-09-08 wurde gemessen, dass **Momentum nichts bringt**
    /// (M1 gegen M2: jede Zahl schlechter). Der Grund ist eine
    /// Unvertraeglichkeit zweier Bausteine: Momentum macht die Summe
    /// groesser, und die Normierung teilt durch deren Maximum, rechnet
    /// die Verstaerkung also **von Bauart wegen wieder heraus**.
    ///
    /// ⚑ **Ein Vorzeichen hat diese Unvertraeglichkeit nicht**, weil es
    /// gar keine Skala traegt. Und in Ganzzahlen ist es die billigste
    /// denkbare Operation: ein Vergleich, keine Division.
    ///
    /// 📌 **Kein Konsensweg.** Ob Vorzeichenabstieg in den
    /// Trainingsvertrag gehoert, ist eine Entscheidung und keine
    /// Messung; `schritt_normiert` bleibt unangetastet.
    pub fn vorzeichen_falten(&mut self, betrag: i64) {
        for (_, summe) in self.summen.values_mut() {
            for g in summe.iter_mut() {
                *g = match (*g).cmp(&0) {
                    std::cmp::Ordering::Greater => betrag,
                    std::cmp::Ordering::Less => -betrag,
                    std::cmp::Ordering::Equal => 0,
                };
            }
        }
    }

    /// Eine Sammlung, die vor dem Anwenden **normiert**.
    pub fn normiert() -> Self {
        Self { normiert: true, ..Self::default() }
    }

    /// Ob diese Sammlung normiert.
    pub fn ist_normiert(&self) -> bool {
        self.normiert
    }

    /// Setzt die Zeilenbreiten, damit **je Zeile** normiert wird.
    ///
    /// ⚑ **Ohne diesen Aufruf bleibt alles wie bisher.** Das ist die
    /// Vorgabe: Ein Aufrufer, der nichts sagt, bekommt das bisherige
    /// Verhalten und keine stille Aenderung.
    pub fn zeilenbreiten_setzen(
        &mut self,
        breiten: std::collections::BTreeMap<Matrixkennung, usize>,
    ) {
        self.zeilenbreiten = breiten;
    }

    /// Die Zeilenbreite einer Matrix, `0` fuer „unbekannt".
    pub fn zeilenbreite(&self, k: Matrixkennung) -> usize {
        self.zeilenbreiten.get(&k).copied().unwrap_or(0)
    }

    /// Beschraenkt, welche Matrizen bewegt werden.
    pub fn auswahl_setzen(&mut self, a: Auswahl) {
        self.auswahl = a;
    }

    /// Was gerade gilt.
    pub fn auswahl(&self) -> Auswahl {
        self.auswahl
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

    /// Dieselben Matrizen, veraenderlich, in **derselben** kanonischen
    /// Reihenfolge wie [`Self::matrizen`].
    ///
    /// ⚑ Nur fuer Messwerkzeuge gedacht, die den Stand absichtlich
    /// stoeren. Der Trainingsweg fasst Matrizen ueber
    /// [`Self::matrix_mut`] an, weil er weiss, welche er meint.
    pub fn matrizen_veraenderlich(&mut self) -> Vec<&mut Vec<Master>> {
        match self {
            Self::Dicht(m) => m.iter_mut().collect(),
            Self::Gemisch { aufmerksamkeit, router, experten } => {
                let mut aus: Vec<&mut Vec<Master>> = aufmerksamkeit.iter_mut().collect();
                aus.push(router);
                for drei in experten.values_mut() {
                    aus.extend(drei.iter_mut());
                }
                aus
            }
        }
    }

    /// Wie viele Experten dieser Ebene **beruehrt** wurden.
    ///
    /// ⚑ **Beim Gemisch ist das die Zahl, die zaehlt.** Ein Experte,
    /// den der Router nie waehlt, bekommt nie einen Gradienten und
    /// bleibt untrainiert. Eine Meldung ueber bewegte Gewichte ohne
    /// diese Zahl laedt zu genau dem falschen Schluss ein: Bei 128
    /// Experten je Ebene koennen Hunderte Millionen Gewichte bewegt
    /// aussehen und trotzdem nur ein Bruchteil der Ebene erreicht sein.
    ///
    /// Die Karte traegt genau die gewaehlten Experten, weil sie beim
    /// Routing angelegt wird (`experten.entry(*i).or_insert_with`).
    ///
    /// ⚠️ **Die Gesamtzahl steht hier nicht**, und das ist Absicht: Sie
    /// ist eine Eigenschaft des **Modells**, nicht des Gewichtsstands.
    /// Wer sie braucht, holt sie dort; ein zweiter Ort fuer dieselbe
    /// Zahl liefe irgendwann auseinander.
    ///
    /// `None` bei einer dichten Ebene: Dort gibt es nichts zu zaehlen,
    /// und eine Null saehe aus wie „kein Experte beruehrt".
    pub fn beruehrte_experten(&self) -> Option<usize> {
        match self {
            Self::Dicht { .. } => None,
            Self::Gemisch { experten, .. } => Some(experten.len()),
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
        // 📌 **Hier stand am 2026-09-08 eine Schranke gegen Modelle mit
        // QK-Normierung**, und sie war richtig: Der Trainingspfad
        // rechnete die Aufmerksamkeit ohne sie, waehrend die Inferenz
        // sie anwandte, und **nichts pruefte das**. Ein Lauf auf einem
        // Qwen3-Artefakt waere durchgelaufen und haette gegen ein
        // Modell trainiert, das es nicht gibt.
        //
        // ⚑ **Sie ist am selben Tag aufgehoben worden**, weil der
        // Vorwaerts- und der Rueckwaertspfad sie jetzt tragen:
        // `qk_norm_heads_mit_spur` und `qk_norm_heads_backward`, der
        // zweite gegen die numerische Ableitung des echten Kernels
        // geprueft. Die Schranke bleibt als Fehlerart bestehen, denn
        // sie ist der Ort, an dem die naechste solche Luecke gemeldet
        // wird.
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

    /// Anfang und Jetzt, Matrix für Matrix, **vollständig auch für die
    /// Experten**, die erst im Lauf hinzukamen.
    ///
    /// # 📌 Fund 346 (2026-09-12): `zip` bricht an der kürzeren Seite ab
    ///
    /// Der Anfangsstand einer Gemischebene trägt **keine** Experten: Ein
    /// Experte bekommt seinen Master erst, wenn der Router ihn wählt,
    /// und das geschieht nach der Aufnahme des Anfangs. Wer
    /// `anfang.matrizen().zip(jetzt.matrizen())` bildet, bekommt
    /// deshalb genau so viele Paare, wie der Anfang Matrizen hat, also
    /// **Aufmerksamkeit und Router**, und keinen einzigen Experten.
    /// `zip` sagt dazu nichts.
    ///
    /// ⛔️ **Drei Meldungen von `trainingsguete` waren dadurch blind für
    /// den Teil des Modells, den ein Gemisch überhaupt trainiert:** die
    /// Ausreisserzeile, die Zahl der geänderten int8-Gewichte und die
    /// der verschobenen Zeilenskalen. Ein Lauf auf `myelith-30b-a3b`
    /// meldete „0,32 Prozent der Gewichte geändert"; der Nenner
    /// 19 136 512 ist auf das Gewicht genau Aufmerksamkeit plus Router
    /// einer Ebene (18 874 368 + 262 144), während im selben Lauf 261
    /// Millionen Gewichte bewegt wurden, also rund fünfundfünfzig
    /// Expertenäquivalente.
    ///
    /// **Am schwersten wiegt die Ausreisserzeile.** Sie ist die
    /// Schranke, die einen entgleisten Lauf sichtbar macht (Faktor
    /// 359,34 statt 1,00 bei einer falschen Schrittweite). Auf einem
    /// Gemisch sah sie den Experten gar nicht zu.
    ///
    /// ⚑ **Das Δ-Commitment ist davon nicht betroffen.** [`Self::deltas`]
    /// läuft über die **Master**-Seite und setzt für einen fehlenden
    /// Anfang eine Nullmatrix ein; der Abdruck deckt die Experten also
    /// ab. Betroffen war die Anzeige, nicht der Konsens.
    ///
    /// # Was diese Methode tut
    ///
    /// Sie läuft über die Master-Seite, also in kanonischer Reihenfolge
    /// über alles, was der Lauf hält, und stellt jeder Matrix ihren
    /// Anfangsstand gegenüber. Für einen Experten, der im Anfang fehlt,
    /// wird dieser **aus dem Modell neu gebildet**, mit demselben
    /// Konstruktor, den auch das Routing benutzt. Damit ist der
    /// Vergleich der, den man meint: gegen den Stand vor dem ersten
    /// Schritt.
    pub fn paare_mit_anfang<'a>(&'a self, m: &IntegerModel) -> Vec<Matrixpaare<'a>> {
        let mut aus = Vec::with_capacity(self.master.len());
        for (i, jetzt) in self.master.iter().enumerate() {
            let ebene = self.von + i;
            let vorher: Vec<Vec<Master>> = match (self.anfang.get(i), jetzt) {
                // Dichte Ebene: die Listen sind gleich lang.
                (Some(a), Ebenenstand::Dicht(_)) => {
                    a.matrizen().iter().map(|s| s.to_vec()).collect()
                }
                (Some(Ebenenstand::Gemisch { aufmerksamkeit, router, .. }),
                 Ebenenstand::Gemisch { experten, .. }) => {
                    let mut v: Vec<Vec<Master>> = aufmerksamkeit.to_vec();
                    v.push(router.clone());
                    // ⚑ **Dieselbe kanonische Reihenfolge wie
                    // `matrizen()`**: Experten nach ihrer Nummer. Eine
                    // andere Ordnung verglände Matrizen über Kreuz, und
                    // das faellt an den Laengen nicht auf.
                    let moe = match m.layers.get(ebene).map(|l| &l.ffn) {
                        Some(Feedforward::Moe(g)) => Some(g),
                        _ => None,
                    };
                    for nr in experten.keys() {
                        match moe.and_then(|g| g.experts.get(*nr as usize)) {
                            Some(ex) => {
                                v.push(master_aus_gewicht(&ex.gate_proj));
                                v.push(master_aus_gewicht(&ex.up_proj));
                                v.push(master_aus_gewicht(&ex.down_proj));
                            }
                            // Kann nicht vorkommen, solange Stand und
                            // Modell zusammengehoeren; dann lieber eine
                            // leere Matrix als ein falsches Paar.
                            None => {
                                v.push(Vec::new());
                                v.push(Vec::new());
                                v.push(Vec::new());
                            }
                        }
                    }
                    v
                }
                _ => Vec::new(),
            };
            aus.push((ebene, vorher, jetzt.matrizen()));
        }
        aus
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
    /// 📌 **Das Modell hat QK-Normierung, der Trainingspfad nicht.**
    ///
    /// Qwen3 normiert Q und K je Kopf, bevor die Aufmerksamkeit
    /// rechnet; `model.rs` tut das im Vorwaertspfad, `vorwaerts_der_ebene`
    /// im Trainingspfad **nicht**.
    ///
    /// ⚑ **Bis zum 2026-09-08 fiel das nicht auf, weil nichts es
    /// pruefte.** Ein Lauf auf einem Qwen3-Artefakt waere
    /// durchgelaufen und haette eine andere Aufmerksamkeit gerechnet
    /// als die Inferenz, also einen Gradienten zu einem Modell, das es
    /// nicht gibt. **Ein stiller Unterschied im Vorwaertspfad ist die
    /// schlimmste Sorte**, weil das Ergebnis plausibel aussieht.
    QkNormNichtGetragen { ebene: usize },
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
            Self::QkNormNichtGetragen { ebene } => write!(
                f,
                "Ebene {ebene} hat QK-Normierung (Qwen3), und der Trainingspfad rechnet sie \
                 NICHT. Ein Lauf darauf traeniert gegen eine andere Aufmerksamkeit als die \
                 Inferenz. Bis das gebaut ist, traegt der Trainingspfad nur Modelle ohne \
                 QK-Normierung"
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
        crate::trainingsschleife::qk_vorgaben_der_ebene(m, e),
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
/// Welche Matrizen ueberhaupt bewegt werden duerfen.
///
/// # ⚑ Parameterisolierung, die dritte Saeule gegen das Vergessen
///
/// Die Literatur kennt drei Mittel gegen katastrophales Vergessen:
/// **Wiederholung** (mehr alter Text im Korpus), **Regularisierung**
/// (siehe [`zum_anfang_ziehen`]) und **Parameterisolierung**. Die
/// bekannteste Form der dritten ist ein Niedrigrangzusatz, der die
/// urspruenglichen Gewichte gar nicht anfasst; die einfachste ist,
/// **weniger anzufassen**.
///
/// ⚑ **Und die Literatur sagt auch, wo Tatsachen liegen:** ROME und
/// MEMIT bearbeiten die MLP-Bloecke, nicht die Aufmerksamkeit. Die
/// Aufmerksamkeit entscheidet, **worauf** eine Position blickt; der
/// MLP-Block ist der Schluessel-Wert-Speicher.
///
/// 📌 **Ungemessen in diesem Projekt.** Fund 205 hat schon einmal eine
/// Erklaerung aus der Literatur genommen und daraus einen Lauf gemacht,
/// der zwischen ihr und der Alternative nicht trennte. Diese Auswahl
/// ist deshalb **gebaut und standardmaessig aus**; wer sie einschaltet,
/// misst sie gegen einen Lauf ohne sie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Auswahl {
    /// Alles, was `matrizen_veraenderlich` hergibt. Die Vorgabe.
    #[default]
    Alles,
    /// Nur die drei MLP-Matrizen (`gate`, `up`, `down`).
    NurMlp,
    /// ⚑ Nur `down_proj`, die Stelle, die MEMIT bearbeitet.
    NurAbwaerts,
}

impl Auswahl {
    /// Ob eine Matrix bewegt werden darf.
    ///
    /// 📌 **Bei einem Expertengemisch greift nur `Alles`.** Die
    /// Zuordnung von Kennung zu Rolle ist dort eine andere, und sie zu
    /// raten hiesse, die falschen Matrizen einzufrieren. Wer ein
    /// Gemisch damit trainieren will, baut die Zuordnung zuerst.
    pub fn erlaubt(&self, k: Matrixkennung) -> bool {
        match (self, k) {
            (Self::Alles, _) => true,
            // Dicht: 0 bis 3 Aufmerksamkeit (q, k, v, o), 4 gate,
            // 5 up, 6 down. Dieselbe Reihenfolge wie in
            // `breiten_der_ebene`.
            (Self::NurMlp, Matrixkennung::Dicht(n)) => n >= 4,
            (Self::NurAbwaerts, Matrixkennung::Dicht(n)) => n == 6,
            // ⚑ **Und dasselbe fuer ein Expertengemisch** (2026-09-08
            // nachgetragen). Beim Gemisch **sind** die Experten der
            // MLP-Teil; die Aufmerksamkeit ist die Aufmerksamkeit, und
            // der Router gehoert zu ihr und nicht zum Speicher: Er
            // entscheidet, **wer** rechnet, nicht **was** herauskommt.
            //
            // Die drei Matrizen je Experte stehen in derselben
            // Reihenfolge wie im dichten Block: 0 gate, 1 up, 2 down.
            (Self::NurMlp, Matrixkennung::Experte(_, _)) => true,
            (Self::NurAbwaerts, Matrixkennung::Experte(_, n)) => n == 2,
            (Self::NurMlp | Self::NurAbwaerts, _) => false,
        }
    }

    /// Der Name fuer die Ausgabe.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Alles => "alles",
            Self::NurMlp => "nur MLP",
            Self::NurAbwaerts => "nur down_proj",
        }
    }
}

/// Zieht die Gewichte um einen Bruchteil zum Ausgangsstand zurueck.
///
/// # ⚑ Warum das hier hingehoert, und warum es fast nichts kostet
///
/// **Der bindende Engpass des Trainings ist nicht der Optimierer,
/// sondern das Vergessen.** Sieben Messlaeufe am 2026-09-08 haben
/// sechs Verfahren verglichen; die Zieltatsache stieg in allen, und in
/// allen erodierte das allgemeine Wissen mit. Die Literatur kennt drei
/// Mittel dagegen (Wiederholung, Regularisierung, Parameterisolierung),
/// und keines davon war eingebaut.
///
/// ⚑ **Der Ausgangsstand liegt bereits im Speicher.** `Shardgewichte`
/// haelt ihn fuer das Δ-Commitment; ein Zug zurueck ist damit
/// **eine Subtraktion und eine Schiebeoperation**, kein neuer Zustand.
///
/// ```text
/// master += (anfang − master) >> staerke
/// ```
///
/// **Was das tut, und warum es das Richtige tut:** Ein Gewicht, das der
/// Gradient stetig in dieselbe Richtung schiebt, waechst schneller, als
/// der Zug es zurueckholt; eines, das nur durch Rauschen oder
/// Nebenwirkung verschoben wurde, wandert zurueck. **Der Zug
/// unterscheidet nicht nach Bedeutung, sondern nach Beharrlichkeit**,
/// und genau das ist der Unterschied zwischen einer gelernten Tatsache
/// und einem Kollateralschaden.
///
/// 📌 **Es ist L2-SP und nicht EWC.** EWC gewichtet den Zug mit der
/// Fisher-Information, also damit, wie **wichtig** ein Gewicht fuer das
/// alte Wissen war; das braeuchte einen zweiten Durchgang ueber alte
/// Daten und eine Groesse, die es hier nicht gibt. Der einfache Zug ist
/// die schwaechere Fassung, und er ist die, die ohne neue Messung
/// auskommt.
///
/// `staerke` ist eine Zweierpotenz: 6 zieht ein Vierundsechzigstel des
/// Abstandes zurueck, 10 ein Tausendstel. ⚑ **Null heisst: gar nicht**,
/// damit ein Aufrufer, der nichts sagt, nichts bekommt.
///
/// **Gibt zurueck**, wie viele Gewichte sich dabei bewegt haben.
pub fn zum_anfang_ziehen(g: &mut Shardgewichte, staerke: u32) -> u64 {
    if staerke == 0 {
        return 0;
    }
    let mut bewegt = 0u64;
    for (stand, anfang) in g.master.iter_mut().zip(g.anfang.iter()) {
        let alt: Vec<Vec<Master>> = anfang.matrizen().iter().map(|m| m.to_vec()).collect();
        for (mat, a) in stand.matrizen_veraenderlich().iter_mut().zip(alt.iter()) {
            if mat.len() != a.len() {
                continue;
            }
            for (w, u) in mat.iter_mut().zip(a.iter()) {
                // ⚑ **Die Verschiebung geht auf den Abstand, nicht auf
                // das Gewicht.** Wer `master >> staerke` abzoege, zoege
                // alles gegen null statt zum Ausgangsstand, und das
                // waere kein Anker, sondern ein Schrumpfen.
                let d = (*u as i64) - (*w as i64);
                let zug = d >> staerke;
                if zug != 0 {
                    *w = (*w as i64 + zug).clamp(Master::MIN as i64, Master::MAX as i64) as Master;
                    bewegt += 1;
                }
            }
        }
    }
    bewegt
}

/// Schreibt einen Gewichtsstand als Datei.
///
/// # 📌 Warum es das bis zum 2026-09-08 nicht gab
///
/// Das Messwerkzeug mass vorher, trainierte, mass nachher und **warf
/// alles weg**. „Noch ein paar Durchgaenge" hiess damit: von vorn
/// anfangen. Bei einem Lauf von drei Stunden ist das kein Detail.
///
/// ⚑ **Und es ist kein Ersatz fuer ein Artefakt.** Was hier
/// hinausgeht, sind die **Meister** (i32 mit `MASTER_FRAC`
/// Nachkommabits), nicht die Uebertragungsform. Ein Artefakt daraus zu
/// bauen ist ein eigener Schritt; hier geht es allein darum, einen
/// Lauf fortsetzen zu koennen.
///
/// **Das Format ist absichtlich stumpf:** eine Kopfzeile mit `von`,
/// `bis` und der Zahl der Matrizen je Ebene, dann die Werte als
/// `i32` in nativer Bytefolge. 📌 **Damit ist es NICHT zwischen
/// Maschinen uebertragbar**, und das ist hier richtig: Es dient dem
/// Fortsetzen auf derselben Maschine, und ein Format, das mehr
/// verspricht, waere ein Format, das jemand fuer den Konsens haelt.
pub fn stand_schreiben(g: &Shardgewichte, pfad: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::io::BufWriter::new(std::fs::File::create(pfad)?);
    f.write_all(b"MYLSTAND1")?;
    f.write_all(&(g.von as u32).to_ne_bytes())?;
    f.write_all(&(g.bis as u32).to_ne_bytes())?;
    for stand in &g.master {
        let matrizen = stand.matrizen();
        f.write_all(&(matrizen.len() as u32).to_ne_bytes())?;
        for m in matrizen {
            f.write_all(&(m.len() as u64).to_ne_bytes())?;
            for w in m {
                f.write_all(&w.to_ne_bytes())?;
            }
        }
    }
    f.flush()
}

/// Liest einen Gewichtsstand in einen bestehenden ein.
///
/// ⚑ **Die Form muss passen**, sonst wird abgelehnt statt zurechtgebogen:
/// Ein Stand, der zu einem anderen Ebenenbereich oder einem anderen
/// Modell gehoert, ergaebe stillschweigend Unsinn.
pub fn stand_lesen(g: &mut Shardgewichte, pfad: &std::path::Path) -> Result<(), String> {
    let d = std::fs::read(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
    let mut i = 0usize;
    let mut nimm = |n: usize| -> Result<&[u8], String> {
        if i + n > d.len() {
            return Err("die Datei endet zu frueh".to_string());
        }
        let s = &d[i..i + n];
        i += n;
        Ok(s)
    };
    if nimm(9)? != b"MYLSTAND1" {
        return Err("das ist kein Gewichtsstand (Kennung fehlt)".into());
    }
    let von = u32::from_ne_bytes(nimm(4)?.try_into().unwrap()) as usize;
    let bis = u32::from_ne_bytes(nimm(4)?.try_into().unwrap()) as usize;
    if von != g.von || bis != g.bis {
        return Err(format!(
            "der Stand gehoert zu den Ebenen {von} bis {bis}, hier laufen {} bis {}",
            g.von, g.bis
        ));
    }
    for (e, stand) in g.master.iter_mut().enumerate() {
        let anzahl = u32::from_ne_bytes(nimm(4)?.try_into().unwrap()) as usize;
        let mut matrizen = stand.matrizen_veraenderlich();
        if anzahl != matrizen.len() {
            return Err(format!(
                "Ebene {e}: der Stand hat {anzahl} Matrizen, das Modell {}",
                matrizen.len()
            ));
        }
        for m in matrizen.iter_mut() {
            let laenge = u64::from_ne_bytes(nimm(8)?.try_into().unwrap()) as usize;
            if laenge != m.len() {
                return Err(format!(
                    "Ebene {e}: eine Matrix hat {laenge} Werte, erwartet {}",
                    m.len()
                ));
            }
            for w in m.iter_mut() {
                *w = Master::from_ne_bytes(nimm(4)?.try_into().unwrap());
            }
        }
    }
    Ok(())
}

/// Die Zeilenbreiten eines Expertengemisches.
///
/// ⚑ **Damit traegt die Zeilennormierung (Fund 206) auch dort.** Sie
/// war beim Einbau nur fuer dichte Ebenen gebaut, und ein Gemisch fiel
/// still auf die Matrixnormierung zurueck: Es rechnete, es rechnete nur
/// das Alte.
///
/// | Kennung | Breite |
/// |---|---|
/// | `Aufmerksamkeit(0..3)` | `hidden` fuer q, k, v; `num_heads · head_dim` fuer o |
/// | `Router` | `hidden` |
/// | `Experte(_, 0)`, `Experte(_, 1)` | `hidden` fuer gate und up |
/// | `Experte(_, 2)` | ⚑ `moe_intermediate_size` fuer down |
///
/// `experten` ist die Zahl der Experten; die Karte deckt sie alle ab,
/// denn welche in einem Durchgang gewaehlt werden, steht erst zur
/// Laufzeit fest.
pub fn zeilenbreiten_gemisch(
    m: &crate::model::IntegerModel,
    moe_is: usize,
    experten: usize,
) -> std::collections::BTreeMap<Matrixkennung, usize> {
    let hs = m.hidden_size;
    let mut aus = std::collections::BTreeMap::new();
    let a = [hs, hs, hs, m.num_heads * m.head_dim];
    for (n, b) in a.iter().enumerate() {
        aus.insert(Matrixkennung::Aufmerksamkeit(n as u8), *b);
    }
    aus.insert(Matrixkennung::Router, hs);
    for e in 0..experten as u16 {
        aus.insert(Matrixkennung::Experte(e, 0), hs);
        aus.insert(Matrixkennung::Experte(e, 1), hs);
        aus.insert(Matrixkennung::Experte(e, 2), moe_is);
    }
    aus
}

/// Die Zeilenbreiten der sieben dichten Matrizen einer Ebene.
///
/// ⚑ **Fuer [`Sammlung::zeilenbreiten_setzen`]**, damit je Zeile statt
/// je Matrix normiert wird (Fund 206). Die Breiten kommen aus
/// derselben Quelle, aus der auch die Rueckumrechnung in die
/// Uebertragungsform sie nimmt; eine zweite Tabelle waere die zweite
/// Fassung, die irgendwann abweicht.
///
/// 📌 **Nur fuer dichte Ebenen.** Bei einem Expertengemisch haengt die
/// Breite an der Kennung des Experten, und die Zuordnung gehoert dann
/// dorthin, wo die Experten leben. Solange das nicht gebaut ist, gibt
/// diese Funktion fuer ein Gemisch **nichts** heraus statt etwas
/// Geratenes.
pub fn zeilenbreiten_dicht(
    m: &crate::model::IntegerModel,
    is: usize,
) -> std::collections::BTreeMap<Matrixkennung, usize> {
    let b = breiten_der_ebene(m, is);
    (0..7u8).map(|n| (Matrixkennung::Dicht(n), b[n as usize])).collect()
}

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
        // ⚑ **Parameterisolierung, vor dem Schritt.** Eine Matrix,
        // die nicht ausgewaehlt ist, bekommt gar nichts; sie zu
        // sammeln und dann zu verwerfen waere dasselbe Ergebnis mit
        // mehr Arbeit, aber der Sammler weiss die Auswahl nicht.
        if !sammlung.auswahl.erlaubt(*k) {
            continue;
        }
        if sammlung.normiert {
            // ⚑ **Je Zeile, wenn die Breite bekannt ist** (Fund 206).
            // `schritt_normiert_je_zeile` faellt bei Breite 0 selbst auf
            // die Matrixnormierung zurueck, also steht hier kein zweiter
            // Zweig, der auseinanderlaufen koennte.
            schritt_normiert_je_zeile(mm, summe, sammlung.zeilenbreite(*k), kn, v.lr_nenner);
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
        crate::trainingsschleife::qk_vorgaben_der_ebene(m, e),
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

