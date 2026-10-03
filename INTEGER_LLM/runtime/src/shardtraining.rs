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
    gewicht_aus_master, gewicht_aus_master_als, gradienten_der_ebene_aus_gradient, vorwaerts_der_ebene,
    Ebenenspur, Gewichtsform,
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
    /// Wie aus dem Master das Gewicht der Rechnung wird.
    ///
    /// ⚑ **Aus dem Modell abgeleitet**: Traegt der Bereich ternaere Ebenen,
    /// trainiert er ternaer (Straight-Through auf den Master), sonst int8.
    /// Das Artefakt legt es fest, und alle Knoten sehen dasselbe Artefakt;
    /// deshalb steht die Form nicht in [`Shardvorgaben`] und nicht auf dem
    /// Draht. [`Shardgewichte::form_setzen`] ist fuer die lokale Umwandlung
    /// eines int8-Modells in ein ternaeres.
    form: Gewichtsform,
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
    /// Mischer, Router, und die bisher gewählten Experten.
    Gemisch {
        /// Die Achtsamkeit (Q, K, V, O) oder die rekurrente Zustandsschicht.
        mischer: Mischerstand,
        /// Die Routerprojektion.
        router: Vec<Master>,
        /// **Der geteilte Experte** (Qwen3.6), falls die Ebene einen hat:
        /// Gate, Up, Down und die eine Zeile seines Tors. Er feuert bei
        /// jedem Token und ist deshalb von Anfang an da (Fund 516).
        geteilt: Option<Box<[Vec<Master>; 4]>>,
        /// Je **Expertennummer** seine drei Matrizen.
        ///
        /// ⚑ **`BTreeMap` und nicht `HashMap`.** Die Karte wird geordnet
        /// durchlaufen, wenn der Abdruck entsteht; eine Hashtabelle gäbe
        /// dabei eine Reihenfolge, die an Adressen hängt.
        experten: std::collections::BTreeMap<u16, [Vec<Master>; 3]>,
    },
}

/// **Die Matrizen des Mischers einer Gemischebene.**
///
/// ⚑ **Der Mischer und der Block sind zwei Achsen**: Das 35B hat zehn
/// Achtsamkeitsebenen und dreissig Zustandsebenen, alle mit Gemisch. Der
/// Block bleibt derselbe, nur der Mischer davor ist ein anderer.
#[derive(Clone)]
pub enum Mischerstand {
    /// Q, K, V, O.
    Achtsamkeit(Box<[Vec<Master>; 4]>),
    /// `in_proj_qkv`, `in_proj_z`, `in_proj_b`, `in_proj_a`, `out_proj` und
    /// die Faltung, in dieser Reihenfolge (sie ist Teil des Abdrucks).
    ///
    /// ⚑ **Nicht trainiert werden** das Gamma der torgesteuerten Norm,
    /// `dt_bias` und `exp_A`: je Wertkopf eine Zahl oder je Kanal ein
    /// Gamma, wie das Gamma der QK-Normierung, das aus demselben Grund
    /// stehen bleibt (`qk_norm_heads_backward`). Ihre Gradienten rechnet
    /// der Rueckweg; sie mitzunehmen hiesse neue Master und neue
    /// Wuerfelbereiche, also eine Aenderung des Trainingsvertrags.
    Zustand(Box<[Vec<Master>; 6]>),
}

impl Mischerstand {
    /// Die Matrizen in kanonischer Reihenfolge.
    pub fn matrizen(&self) -> &[Vec<Master>] {
        match self {
            Self::Achtsamkeit(a) => &a[..],
            Self::Zustand(z) => &z[..],
        }
    }

    /// Dieselben, veraenderlich.
    pub fn matrizen_mut(&mut self) -> &mut [Vec<Master>] {
        match self {
            Self::Achtsamkeit(a) => &mut a[..],
            Self::Zustand(z) => &mut z[..],
        }
    }

    /// Die Kennung der `n`-ten Matrix.
    pub fn kennung(&self, n: usize) -> Matrixkennung {
        match self {
            Self::Achtsamkeit(_) => Matrixkennung::Aufmerksamkeit(n as u8),
            Self::Zustand(_) => Matrixkennung::Zustand(n as u8),
        }
    }
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
    /// Der geteilte Experte: 0 Gate, 1 Up, 2 Down, 3 sein Tor.
    ///
    /// ⚑ **Hinten angehaengt**, damit die Ordnung der uebrigen Kennungen
    /// bleibt, wie sie war.
    Geteilt(u8),
    /// Die Matrizen einer Zustandsschicht, in der Reihenfolge von
    /// [`Mischerstand::Zustand`]. Ebenfalls hinten angehaengt.
    Zustand(u8),
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
            Self::Gemisch { mischer, router, geteilt, experten } => Self::Gemisch {
                mischer: mischer.clone(),
                router: router.clone(),
                geteilt: geteilt.clone(),
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
            (Self::Gemisch { mischer: Mischerstand::Achtsamkeit(a), .. }, Matrixkennung::Aufmerksamkeit(n)) => {
                a.get_mut(n as usize)
            }
            (Self::Gemisch { mischer: Mischerstand::Zustand(z), .. }, Matrixkennung::Zustand(n)) => {
                z.get_mut(n as usize)
            }
            (Self::Gemisch { router, .. }, Matrixkennung::Router) => Some(router),
            (Self::Gemisch { geteilt: Some(g), .. }, Matrixkennung::Geteilt(n)) => g.get_mut(n as usize),
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
            Self::Gemisch { mischer, router, geteilt, experten } => {
                let mut aus: Vec<&[Master]> =
                    mischer.matrizen().iter().map(|v| v.as_slice()).collect();
                aus.push(router.as_slice());
                // ⚑ Der geteilte Experte hinter dem Router und vor den
                //   gewaehlten: Er ist fest da, die gewaehlten wachsen.
                if let Some(g) = geteilt {
                    aus.extend(g.iter().map(|v| v.as_slice()));
                }
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
            Self::Gemisch { mischer, router, geteilt, experten } => {
                let mut aus: Vec<&mut Vec<Master>> = mischer.matrizen_mut().iter_mut().collect();
                aus.push(router);
                if let Some(g) = geteilt {
                    aus.extend(g.iter_mut());
                }
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
    /// Die Form, mit der dieser Bereich trainiert.
    pub fn form(&self) -> Gewichtsform {
        self.form
    }

    /// **Setzt die Form fuer eine lokale Umwandlung**: ein int8-Modell,
    /// dessen Master aus den int8-Gewichten starten, ternaer nachtrainieren.
    ///
    /// ⛔️ **Nicht umgekehrt:** Ein ternaerer Bereich laesst sich nicht als
    /// int8 trainieren (seine Master tragen nur -a, 0 und +a); das wird
    /// abgewiesen.
    pub fn form_setzen(&mut self, form: Gewichtsform) -> Result<(), Shardfehler> {
        if self.form == Gewichtsform::Ternaer && form == Gewichtsform::Int8 {
            return Err(Shardfehler::TernaerNichtTrainierbar { ebene: self.von });
        }
        self.form = form;
        Ok(())
    }

    /// Liest die Gewichte des Bereichs aus dem Modell.
    pub fn aus_modell(m: &IntegerModel, von: usize, bis: usize) -> Result<Self, Shardfehler> {
        if von >= bis || bis > m.num_layers {
            return Err(Shardfehler::BereichUngueltig { von, bis, ebenen: m.num_layers });
        }
        let bereich: Vec<&crate::model::TransformerLayer> = m.layers.iter().take(bis).skip(von).collect();
        // ⚑ **Zustandsebenen traegt der Gemischweg seit dem 2026-10-02**
        //   (T5): vorwaerts ueber denselben Rumpf wie die Inferenz, rueckwaerts
        //   ueber `zustandstraining`. Mit einem dichten Block gibt es noch
        //   kein Modell und deshalb noch keinen Weg.
        if let Some(e) = bereich
            .iter()
            .find(|e| matches!(e.mischer, crate::model::Mischer::Zustand(_)) && matches!(e.ffn, Feedforward::Dense(_)))
        {
            return Err(Shardfehler::ZustandsschichtNichtGetragen { ebene: e.layer_idx });
        }
        // ⚑ **Fund 508 geschlossen (2026-10-02):** Tor am Ausgang der
        //   Achtsamkeit und Teildrehung rechnet der Trainingspfad selbst,
        //   vorwaerts Wert fuer Wert wie die Inferenz (das Tor ueber dieselbe
        //   Kernfunktion) und rueckwaerts. Hier wurden beide bis dahin
        //   abgewiesen. `VorwaertspfadNichtGetragen` bleibt fuer das
        //   naechste Bauteil, das die Inferenz kennt und das Training nicht.
        //
        // ⚑ **Fund 516 geschlossen (2026-10-02): der geteilte Experte** rechnet
        //   im Gemischweg mit, vorwaerts Wert fuer Wert wie die Inferenz und
        //   rueckwaerts; bis dahin fehlte er dort und wurde abgewiesen.
        //
        // ⛔️ **Fund 517 (2026-10-02): ternaere Betraege ueber 127.** Der
        //   Trainingspfad leitet ein ternaeres Gewicht als int8 mit Zeilenshift
        //   ab, der Betrag einer Gruppe ist dort hoechstens 127. Ein Import
        //   mit exakt uebernommenen Skalen (das 27B: Betraege bis 6 692, in
        //   jeder Gruppe ueber 127) liegt auf einem feineren Raster; das
        //   Training rundete ihn still auf ein groeberes und rechnete ein
        //   anderes Modell als die Inferenz (Ebene 3: 98 % der Werte
        //   verschieden). Gefunden mit `vorwaertsvergleich`, abgegrenzt am
        //   8B (Betraege bis 127, auch mit angehaengter Drehung Wert fuer Wert
        //   gleich).
        for e in &bereich {
            if ebene_zu_fein_fuer_das_training(e) {
                return Err(Shardfehler::VorwaertspfadNichtGetragen {
                    ebene: e.layer_idx,
                    was: "ternaeren Gruppenbetraegen ueber 127, also einem feineren Raster als dem des Trainings",
                });
            }
        }
        // ⚑ **Ganz ternaer oder gar nicht.** Ein Bereich mit beiden Arten
        //   braeuchte eine Form je Ebene; das kommt, wenn es ein Modell gibt,
        //   das es verlangt.
        let ternaer = bereich.iter().filter(|e| e.ist_ternaer()).count();
        if ternaer != 0 && ternaer != bereich.len() {
            let e = bereich.iter().find(|e| !e.ist_ternaer()).expect("eine ist nicht ternaer");
            return Err(Shardfehler::TernaerNichtTrainierbar { ebene: e.layer_idx });
        }
        if ternaer != 0 {
            if let Some(e) = bereich.iter().find(|e| matches!(e.ffn, Feedforward::Moe(_))) {
                return Err(Shardfehler::TernaerNichtTrainierbar { ebene: e.layer_idx });
            }
        }
        let form = if ternaer != 0 { Gewichtsform::Ternaer } else { Gewichtsform::Int8 };
        let mut master = Vec::with_capacity(bis - von);
        for ebene in m.layers.iter().take(bis).skip(von) {
            master.push(match &ebene.ffn {
                Feedforward::Dense(_) => Ebenenstand::Dicht(Box::new(
                    master_der_ebene(ebene).expect("dichte Ebene hat sieben Matrizen"),
                )),
                // ⚑ **Die Experten bleiben leer**, siehe [`Ebenenstand`]:
                // Sie kommen dazu, wenn der Router sie wählt.
                Feedforward::Moe(moe) => Ebenenstand::Gemisch {
                    mischer: match &ebene.mischer {
                        crate::model::Mischer::Achtsamkeit(a) => Mischerstand::Achtsamkeit(Box::new([
                            master_aus_gewicht(&a.q_proj),
                            master_aus_gewicht(&a.k_proj),
                            master_aus_gewicht(&a.v_proj),
                            master_aus_gewicht(&a.o_proj),
                        ])),
                        crate::model::Mischer::Zustand(zs) => Mischerstand::Zustand(Box::new([
                            master_aus_gewicht(&zs.in_proj_qkv),
                            master_aus_gewicht(&zs.in_proj_z),
                            master_aus_gewicht(&zs.in_proj_b),
                            master_aus_gewicht(&zs.in_proj_a),
                            master_aus_gewicht(&zs.out_proj),
                            master_aus_gewicht(&zs.conv1d),
                        ])),
                    },
                    router: master_aus_gewicht(&moe.router),
                    geteilt: moe.geteilter_experte.as_ref().map(|ge| {
                        Box::new([
                            master_aus_gewicht(&ge.mlp.gate_proj),
                            master_aus_gewicht(&ge.mlp.up_proj),
                            master_aus_gewicht(&ge.mlp.down_proj),
                            master_aus_gewicht(&ge.tor),
                        ])
                    }),
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
        Ok(Self { anfang: master.clone(), master, von, bis, form })
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
            form: self.form,
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
                (Some(Ebenenstand::Gemisch { mischer, router, geteilt, .. }),
                 Ebenenstand::Gemisch { experten, .. }) => {
                    let mut v: Vec<Vec<Master>> = mischer.matrizen().to_vec();
                    v.push(router.clone());
                    if let Some(g) = geteilt {
                        v.extend(g.iter().cloned());
                    }
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
    /// Die Spur des Aufmerksamkeitsblocks (leer bei einer Zustandsebene).
    pub aufmerksamkeit: integer_llm_kernels::trainingsschritt::Aufmerksamkeitsspur,
    /// Die Spur der Zustandsschicht, falls die Ebene rekurrent mischt.
    pub zustand: Option<Box<crate::model::Zustandsspur>>,
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
    /// Was der geteilte Experte an dieser Position getan hat, falls es ihn
    /// gibt.
    pub geteilt: Option<Geteiltteil>,
}

/// Was der **geteilte Experte** an einer Position getan hat.
pub struct Geteiltteil {
    /// Die Zwischenwerte seines MLP.
    pub spur: integer_llm_kernels::mlp::Mlpspur,
    /// Seine Ausgabe vor dem Tor, auf der Akkumulationsskala des Blocks.
    pub ausgabe: Vec<i16>,
    /// Der Faktor `sigmoid(tor)` auf den Bruchstellen der Sigmoid-Tabelle.
    pub faktor: i64,
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
    /// ⛔️ **Die Ebene passt nicht zur ternaeren Form**: ein Bereich, der
    /// ternaere und int8-Ebenen mischt, ein ternaerer Bereich mit
    /// Expertengemisch, oder ein ternaerer Bereich, der als int8 trainiert
    /// werden soll. Ein ternaerer Bereich trainiert ternaer: Der Master ist
    /// hochaufgeloest, die Rechnung sieht je Gruppe -a, 0 und +a.
    TernaerNichtTrainierbar { ebene: usize },
    /// ⛔️ **Eine rekurrente Zustandsschicht mit dichtem Block**: Den Weg
    /// gibt es nur mit Expertengemisch (T5, 2026-10-02), weil es nur solche
    /// Modelle gibt. Vorher war jede Zustandsebene abgewiesen, davor eine
    /// Panik beim Einlesen.
    ZustandsschichtNichtGetragen { ebene: usize },
    /// ⛔️ **Die Ebene rechnet vorwaerts etwas, das der Trainingspfad nicht
    /// rechnet** (Fund 508): ein Tor am Ausgang der Achtsamkeit, oder eine
    /// Positionsdrehung ueber nur einen Teil des Kopfes. (Die
    /// Eingangsdrehung vor einer Projektion gehoerte dazu, bis der
    /// Trainingspfad sie am selben Tag bekam.)
    ///
    /// 📌 **Bis zum 2026-09-30 lief ein solcher Bereich durch.** Der
    /// Trainingspfad hat seinen eigenen Vorwaertspass, und der kannte keine
    /// der drei: Gedrehte Gewichte bekamen eine ungedrehte Eingabe, das Tor
    /// fiele weg und seine Zeilen in `q_proj` laegen als Abfragen in den
    /// Koepfen, die Positionstabelle wuerde mit der falschen Breite gelesen.
    /// Jedes davon gibt Zahlen und keinen Fehler, also einen Gradienten zu
    /// einem Modell, das es nicht gibt. Derselbe Fall wie
    /// [`Shardfehler::QkNormNichtGetragen`], eine Bauart spaeter.
    VorwaertspfadNichtGetragen { ebene: usize, was: &'static str },
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
            Self::TernaerNichtTrainierbar { ebene } => write!(
                f,
                "Ebene {ebene} passt nicht zur ternaeren Form: Ein Bereich ist ganz ternaer \
                 oder gar nicht, ohne Expertengemisch, und ein ternaerer Bereich trainiert ternaer"
            ),
            Self::ZustandsschichtNichtGetragen { ebene } => write!(
                f,
                "Ebene {ebene} ist eine rekurrente Zustandsschicht mit dichtem Block; der \
                 Trainingspfad traegt sie bisher nur mit Expertengemisch"
            ),
            Self::VorwaertspfadNichtGetragen { ebene, was } => write!(
                f,
                "Ebene {ebene} rechnet vorwaerts mit {was}, und der Trainingspfad rechnet das \
                 NICHT. Ein Lauf darauf traeniert gegen ein anderes Modell als die Inferenz"
            ),
        }
    }
}

impl std::error::Error for Shardfehler {}

/// **Liegt eine ternaere Matrix auf einem feineren Raster, als das Training
/// darstellt?** Das Training leitet ein ternaeres Gewicht als int8 mit
/// Zeilenshift ab; ein Gruppenbetrag ueber 127 passt dort nicht (Fund 517).
pub fn ternaer_zu_fein(t: &crate::model::QTensor) -> bool {
    match &*t.data {
        crate::model::Gewichtsdaten::Ternaer(td) => td.matrix().betraege().iter().any(|b| b.unsigned_abs() > 127),
        _ => false,
    }
}

/// Ob irgendeine Matrix einer Ebene zu fein fuer das Training ist.
fn ebene_zu_fein_fuer_das_training(e: &crate::model::TransformerLayer) -> bool {
    let a = match &e.mischer {
        crate::model::Mischer::Achtsamkeit(_) => {
            let a = e.achtsamkeit();
            [&a.q_proj, &a.k_proj, &a.v_proj, &a.o_proj].into_iter().any(ternaer_zu_fein)
        }
        crate::model::Mischer::Zustand(_) => false,
    };
    let f = match &e.ffn {
        Feedforward::Dense(mlp) => [&mlp.gate_proj, &mlp.up_proj, &mlp.down_proj].into_iter().any(ternaer_zu_fein),
        Feedforward::Moe(moe) => moe
            .experts
            .iter()
            .any(|x| [&x.gate_proj, &x.up_proj, &x.down_proj].into_iter().any(ternaer_zu_fein)),
    };
    a || f
}

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
    let form = g.form;
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
                    .map(|n| gewicht_aus_master_als(&master[n], breiten[n], MASTER_FRAC, form))
                    .collect();
                let gew = gewichte_der_ebene(&umgerechnet, ebene);
                let vg = vorgaben_der_ebene(
                    m, &ebene.scales, e, is, v.schritt, v.lr_zaehler, v.lr_nenner, form,
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
                gemisch_vorwaerts(m, ebene, moe, &mut g.master[i], e, v, &strom, tab, form)?,
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
    form: Gewichtsform,
) -> Result<Gemischebenenspur, Shardfehler> {
    use integer_llm_kernels::fixed_point::rescale_i64;
    use integer_llm_kernels::mlp::{mlp_int_mit_spur, Mlpspur};
    use integer_llm_kernels::moe::{mische_experten, route_top_k};
    use integer_llm_kernels::trainingsschritt::vorwaerts_der_aufmerksamkeit;

    let Ebenenstand::Gemisch { mischer, router, geteilt, experten } = stand else {
        return Err(Shardfehler::ArtPasstNicht { ebene: e });
    };
    let sc = &ebene.scales;
    let cfg = &m.config;
    let hs = m.hidden_size;
    let is = moe.experts[0].gate_proj.shape[0];
    let vg = vorgaben_der_ebene(m, sc, e, is, v.schritt, v.lr_zaehler, v.lr_nenner, Gewichtsform::Int8);
    let (a_vorgaben, m_vorgaben) = vg.bloecke();
    // Der geteilte Experte, aus seinen Mastern, einmal fuer die ganze Folge.
    let geteilt_gew = geteilter_experte_gewichte(moe, geteilt.as_deref(), hs);
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

    // 2. Der Mischer, aus den Mastern: Achtsamkeit oder Zustandsschicht.
    let mischer_aus: Vec<Vec<i16>> = match mischer {
        Mischerstand::Achtsamkeit(aufmerksamkeit) => {
            let a_breiten = [hs, hs, hs, m.num_heads * m.head_dim];
            let a_umgerechnet: Vec<(Vec<i8>, Vec<u8>)> = (0..4)
                .map(|n| gewicht_aus_master_als(&aufmerksamkeit[n], a_breiten[n], MASTER_FRAC, form))
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
                drehung: crate::trainingsschleife::achtsamkeitsdrehungen(ebene),
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
                crate::trainingsschleife::tor_vorgaben(m),
                a_vorgaben);
            spur.aufmerksamkeit.y.clone()
        }
        Mischerstand::Zustand(z) => {
            // ⚑ **Derselbe Rumpf wie in der Inferenz**, mit den Gewichten aus
            //   den Mastern und ab leerem Zustand (T5).
            let crate::model::Mischer::Zustand(zs) = &ebene.mischer else {
                return Err(Shardfehler::ArtPasstNicht { ebene: e });
            };
            let zs_master = zustandsschicht_aus_mastern(zs, z, form);
            let zeilen: Vec<&[i16]> = spur.norm_ein.iter().map(Vec::as_slice).collect();
            let (y, zspur) = m.zustandsschicht_mit_spur(ebene, &zs_master, &zeilen, &acc_attn);
            spur.zustand = Some(Box::new(zspur));
            y
        }
    };

    // 3. Residual, zweite Normierung, Gemisch, zweite Residualaddition.
    let exp_shift = moe.router_frac.saturating_sub(cfg.exp_input_frac);
    // ⚑ **Der Router bleibt int8, auch in einem ternaeren Lauf** (L13): Eine
    //   ternaere Expertenwahl kippt (gemessen mit `routerumwandlung`), und die
    //   Literatur haelt ihn hochaufgeloest (MoTE). Ebenso der geteilte
    //   Experte mit seinem Tor, der hochaufgeloeste Pfad neben den ternaeren
    //   Experten.
    let (rw, rs) = gewicht_aus_master(router, hs, MASTER_FRAC);
    for (nr, h) in hidden.iter().enumerate() {
        let o_aus = &mischer_aus[nr];
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
            let (gw, gs) = gewicht_aus_master_als(&mm[0], hs, MASTER_FRAC, form);
            let (uw, us) = gewicht_aus_master_als(&mm[1], hs, MASTER_FRAC, form);
            let (dw, ds) = gewicht_aus_master_als(&mm[2], is, MASTER_FRAC, form);
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
        // ⚑ **Der geteilte Experte, Wert fuer Wert wie die Inferenz**
        //   (`geteilten_experten_addieren`): sein MLP auf demselben normierten
        //   Strom, sein Tor als eine Zeile, `gemischt + rshift(geteilt · s)`.
        let (block, geteilt_teil) = match &geteilt_gew {
            Some((ge, w)) => {
                let mut sp = Mlpspur::default();
                let aus = mlp_int_mit_spur(
                    &norm, &w[0].0, &w[1].0, &w[2].0, hs, ge.zwischen, &w[0].1, &w[1].1, &w[2].1, tab.silu,
                    m_vorgaben.act_frac, ge.gate_frac, ge.up_frac, ge.down_in_frac,
                    m_vorgaben.silu_in_frac, m_vorgaben.silu_lut_offset, m_vorgaben.silu_out_frac,
                    &acc_mlp, Some(&mut sp),
                );
                let tor_roh = integer_llm_kernels::linear::linear_w8a16(
                    &norm, &w[3].0, hs, &w[3].1, m_vorgaben.act_frac, ge.tor_frac,
                )[0];
                let s = integer_llm_kernels::integer_math::torfaktor(
                    tor_roh, ge.tor_frac, &m.sigmoid_lut, m.sigmoid_versatz, m.sigmoid_ein_frac, m.sigmoid_aus_frac,
                );
                let summe: Vec<i16> = block
                    .iter()
                    .zip(&aus)
                    .map(|(&g, &x)| {
                        let beitrag = integer_llm_kernels::fixed_point::rshift_round_i64(i64::from(x) * s, m.sigmoid_aus_frac);
                        integer_llm_kernels::fixed_point::clamp_i16_from_i64(i64::from(g) + beitrag)
                    })
                    .collect();
                (summe, Some(Geteiltteil { spur: sp, ausgabe: aus, faktor: s }))
            }
            None => (block, None),
        };

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
            geteilt: geteilt_teil,
        });
        spur.y.push(y);
    }
    Ok(spur)
}

/// Ein quantisiertes Gewicht mit seinen Zeilenskalen.
type Quantisiert = (Vec<i8>, Vec<u8>);

/// Der Abstand der gehaltenen Zustaende im Rueckwaertspass der Rekurrenz.
///
/// ⚑ **Aendert keine Zahl**, nur Speicher und Zeit (Probe
/// `der_abstand_aendert_keine_zahl`). Sechzehn halten bei 128 x 128 je Kopf
/// rund 2 MiB je Kopf und Abschnitt und rechnen jeden Zustand einmal neu.
const ZUSTAND_ABSTAND: usize = 16;

/// **Die Zustandsschicht einer Ebene mit den Gewichten aus ihren Mastern.**
///
/// Was nicht trainiert wird (Gamma der Norm, `dt_bias`, `exp_A`, die
/// Skalen), kommt aus dem Modell; die Drehungen ebenso, denn sie sind eine
/// Festlegung des Artefakts und kein Gewicht.
fn zustandsschicht_aus_mastern(
    zs: &crate::model::Zustandsschicht,
    z: &[Vec<Master>; 6],
    form: Gewichtsform,
) -> crate::model::Zustandsschicht {
    // ⚑ **Ternaer nur, was auch ein ternaeres Artefakt ternaer traegt**:
    //   `in_proj_qkv`, `in_proj_z`, `out_proj`. `in_proj_a` und `in_proj_b`
    //   (eine Zeile je Wertkopf, sie bestimmen Zerfall und Schreibstaerke)
    //   und die Faltung bleiben int8, wie beim gepackten 27B.
    let neu = |t: &crate::model::QTensor, mm: &[Master], f: Gewichtsform| -> crate::model::QTensor {
        let (w, s) = gewicht_aus_master_als(mm, t.cols(), MASTER_FRAC, f);
        crate::model::QTensor {
            data: std::sync::Arc::new(crate::model::Gewichtsdaten::Speicher(w)),
            shape: t.shape.clone(),
            shifts: s,
            drehung: t.drehung.clone(),
        }
    };
    crate::model::Zustandsschicht {
        in_proj_qkv: neu(&zs.in_proj_qkv, &z[0], form),
        in_proj_z: neu(&zs.in_proj_z, &z[1], form),
        in_proj_b: neu(&zs.in_proj_b, &z[2], Gewichtsform::Int8),
        in_proj_a: neu(&zs.in_proj_a, &z[3], Gewichtsform::Int8),
        out_proj: neu(&zs.out_proj, &z[4], form),
        conv1d: neu(&zs.conv1d, &z[5], Gewichtsform::Int8),
        exp_a: zs.exp_a.clone(),
        dt_bias: zs.dt_bias.clone(),
        norm_gamma: zs.norm_gamma.clone(),
        skalen: zs.skalen.clone(),
    }
}

/// **Die Gewichte des geteilten Experten aus seinen Mastern**: Gate, Up,
/// Down und die Torzeile, mit den Angaben des Modells dazu.
fn geteilter_experte_gewichte<'a>(
    moe: &'a crate::model::MoeLayer,
    geteilt: Option<&[Vec<Master>; 4]>,
    hs: usize,
) -> Option<(&'a crate::model::GeteilterExperte, [Quantisiert; 4])> {
    let ge = moe.geteilter_experte.as_ref()?;
    let gm = geteilt?;
    Some((
        ge,
        [
            gewicht_aus_master(&gm[0], hs, MASTER_FRAC),
            gewicht_aus_master(&gm[1], hs, MASTER_FRAC),
            gewicht_aus_master(&gm[2], ge.zwischen, MASTER_FRAC),
            gewicht_aus_master(&gm[3], hs, MASTER_FRAC),
        ],
    ))
}

/// **Rueckwaerts durch `block = gemischt + rshift(geteilt · s, f)`**, ohne
/// den Gemischanteil (der bekommt `g` unveraendert).
///
/// `g` liegt auf dem Bus des Blocks, `geteilt` je Kanal auf `acc`, `s` auf
/// `f` Bruchstellen. Zurueck kommen der Gradient nach dem MLP des geteilten
/// Experten (`g · s`, auf dem Bus) und der nach seinem rohen Tor
/// (`Σ g · geteilt · s · (1 − s)`, eine Zahl auf dem Bus).
///
/// ⚑ **Die Summe in `i128`**: `g · geteilt` erreicht 2^46, ueber alle
/// Kanaele summiert und mit `s` multipliziert waere `i64` zu klein.
fn torzweig_rueckwaerts(g: &[i32], geteilt: &[i16], s: i64, f: u8, acc: &[u8]) -> (Vec<i32>, i32) {
    use integer_llm_kernels::fixed_point::{rshift_round_i128, rshift_round_i64};
    use integer_llm_kernels::trainingsschritt::begrenze;
    assert_eq!(g.len(), geteilt.len(), "Torzweig: Gradient und Ausgabe verschieden lang");
    assert_eq!(g.len(), acc.len(), "Torzweig: eine Skala je Kanal");
    let g_mlp: Vec<i32> = g.iter().map(|x| begrenze(rshift_round_i64(i64::from(*x) * s, f))).collect();
    let mut summe: i128 = 0;
    for ((x, y), a) in g.iter().zip(geteilt).zip(acc) {
        summe += i128::from(rshift_round_i64(i64::from(*x) * i64::from(*y), *a));
    }
    let t2 = rshift_round_i128(summe * i128::from(s), u32::from(f));
    let t3 = rshift_round_i128(t2 * i128::from((1i64 << f) - s), u32::from(f));
    (g_mlp, t3.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32)
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
    /// Je Matrix des Mischers (Achtsamkeit oder Zustandsschicht).
    mischer: Vec<u64>,
    router: u64,
    experten_basis: u64,
    je_experte: u64,
    je_matrix: u64,
    /// Gate, Up, Down und Tor des geteilten Experten, hinter allen
    /// Experten; ohne ihn null und ungenutzt.
    geteilt: [u64; 4],
}

impl Gemischversatz {
    /// ⛔️ **Fund 518 (2026-10-02): die Bereiche aus den wirklichen Laengen.**
    ///
    /// Hier stand eine Formel mit `Q = hs × hs` und dem Vermerk „hier zaehlt
    /// nur, dass die Bereiche sich nicht ueberlappen“. Bei den Qwen3-Gemischen
    /// ist `num_heads · head_dim` aber nicht `hidden` (35B: 4 096 gegen
    /// 2 048), und mit dem Tor hat Q noch einmal doppelt so viele Zeilen: Die
    /// Wuerfelbereiche von Q, K, V und O ueberlappten, und das stochastische
    /// Runden war zwischen ihnen korreliert. Jetzt wird fortgezaehlt wie im
    /// dichten Weg. Der Router steht weiter vor den Experten, der geteilte
    /// Experte hinter ihnen: Beide Bereiche haengen nicht daran, welche
    /// Experten gewaehlt werden.
    fn neu(
        mischer: &[Vec<Master>],
        router: usize,
        hs: usize,
        is: usize,
        anzahl_experten: usize,
        geteilt: Option<&[Vec<Master>; 4]>,
    ) -> Self {
        let mut a = Vec::with_capacity(mischer.len());
        let mut p = 0u64;
        for m in mischer {
            a.push(p);
            p += m.len() as u64;
        }
        let router_ab = p;
        p += router as u64;
        let experten_basis = p;
        let je_experte = (hs * is * 3) as u64;
        p += je_experte * anzahl_experten as u64;
        let mut g = [0u64; 4];
        if let Some(gm) = geteilt {
            for (n, m) in gm.iter().enumerate() {
                g[n] = p;
                p += m.len() as u64;
            }
        }
        Self { mischer: a, router: router_ab, experten_basis, je_experte, je_matrix: (hs * is) as u64, geteilt: g }
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
    /// ⚑ **Alles ausser dem Router** (Messwerkzeug fuer L13, 2026-10-02):
    /// Ein ternaerer Lauf ueber vier Ebenen des 35B liess die Zahl der
    /// gewaehlten Experten je Durchgang von rund 150 auf 58 je Ebene fallen
    /// und die Haltemenge entgleisen; die Frage ist, ob der Router daran
    /// schuld ist.
    OhneRouter,
    /// Nur der Mischer: Achtsamkeit oder Zustandsschicht (Messwerkzeug fuer
    /// L13, dieselbe Frage von der anderen Seite).
    NurMischer,
    /// Nur die Zustandsschicht, alle sechs Matrizen.
    NurZustand,
    /// Nur deren grosse Projektionen (`in_proj_qkv`, `in_proj_z`, `out_proj`),
    /// ohne die Torzeilen `in_proj_b`, `in_proj_a` und ohne die Faltung.
    NurZustandsprojektionen,
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
            (Self::OhneRouter, k) => k != Matrixkennung::Router,
            (Self::NurMischer, k) => matches!(k, Matrixkennung::Aufmerksamkeit(_) | Matrixkennung::Zustand(_)),
            (Self::NurZustand, k) => matches!(k, Matrixkennung::Zustand(_)),
            (Self::NurZustandsprojektionen, k) => matches!(k, Matrixkennung::Zustand(0 | 1 | 4)),
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
            // ⚑ **Der geteilte Experte gehoert zum MLP-Teil**, seine drei
            //   Matrizen wie die eines gewaehlten; sein Tor (3) entscheidet
            //   wie der Router nur, wie viel er beitraegt, und bleibt stehen.
            (Self::NurMlp, Matrixkennung::Geteilt(n)) => n < 3,
            (Self::NurAbwaerts, Matrixkennung::Geteilt(n)) => n == 2,
            (Self::NurMlp | Self::NurAbwaerts, _) => false,
        }
    }

    /// Der Name fuer die Ausgabe.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Alles => "alles",
            Self::NurMlp => "nur MLP",
            Self::NurAbwaerts => "nur down_proj",
            Self::OhneRouter => "alles ausser dem Router",
            Self::NurMischer => "nur der Mischer",
            Self::NurZustand => "nur die Zustandsschicht",
            Self::NurZustandsprojektionen => "nur die Projektionen der Zustandsschicht",
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
/// | `Geteilt(0, 1, 3)`, `Geteilt(2)` | `hidden`; die Zwischenbreite des geteilten Experten fuer down |
/// | `Zustand(0..3)`, `Zustand(4)`, `Zustand(5)` | `hidden` fuer die vier Eingangsprojektionen; `Wertkoepfe · wert_dim` fuer `out_proj`; die Kernbreite der Faltung |
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
    // ⚑ **Der geteilte Experte, falls das Modell einen hat** (Fund 516):
    //   gate, up und das Tor lesen `hidden`, down seine eigene
    //   Zwischenbreite. Ohne Eintrag fiele er still auf die Matrixnormierung
    //   zurueck, dieselbe Luecke wie vor Fund 206.
    let zwischen = m.layers.iter().find_map(|l| match &l.ffn {
        Feedforward::Moe(g) => g.geteilter_experte.as_ref().map(|ge| ge.zwischen),
        _ => None,
    });
    if let Some(zw) = zwischen {
        for (n, b) in [(0u8, hs), (1, hs), (2, zw), (3, hs)] {
            aus.insert(Matrixkennung::Geteilt(n), b);
        }
    }
    // ⚑ **Die Zustandsschicht, falls das Modell eine hat** (T5), aus den
    //   Tensoren selbst und nicht aus einer zweiten Tabelle.
    if let Some(zs) = m.layers.iter().find_map(|l| match &l.mischer {
        crate::model::Mischer::Zustand(zs) => Some(zs),
        _ => None,
    }) {
        for (n, t) in [&zs.in_proj_qkv, &zs.in_proj_z, &zs.in_proj_b, &zs.in_proj_a, &zs.out_proj, &zs.conv1d]
            .into_iter()
            .enumerate()
        {
            aus.insert(Matrixkennung::Zustand(n as u8), t.cols());
        }
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
    let form = gew_stand.form;
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
                    .map(|n| gewicht_aus_master_als(&master[n], breiten[n], MASTER_FRAC, form))
                    .collect();
                let gew = gewichte_der_ebene(&umgerechnet, ebene);
                let vg = vorgaben_der_ebene(
                    m, &ebene.scales, e, is, v.schritt, v.lr_zaehler, v.lr_nenner, form,
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
                form,
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
    form: Gewichtsform,
) -> Result<Vec<Vec<i32>>, Shardfehler> {
    use integer_llm_kernels::fixed_point::rescale_i64;
    use integer_llm_kernels::trainingsschritt::{
        begrenze, gradienten_der_aufmerksamkeit_aus_gradient, gradienten_des_gemisches,
        normrueckwaerts, Expertengewichte, Gemischvorgaben,
    };
    use std::collections::BTreeMap;

    let Ebenenstand::Gemisch { mischer, router, geteilt, experten } = stand else {
        return Err(Shardfehler::ArtPasstNicht { ebene: e });
    };
    let sc = &ebene.scales;
    let cfg = &m.config;
    let hs = m.hidden_size;
    let is = moe.experts[0].gate_proj.shape[0];
    let n = moe.experts.len();
    let geteilt_gew = geteilter_experte_gewichte(moe, geteilt.as_deref(), hs);
    let mut g_geteilt: Option<[Vec<i64>; 4]> = geteilt
        .as_deref()
        .map(|gm| [vec![0i64; gm[0].len()], vec![0i64; gm[1].len()], vec![0i64; gm[2].len()], vec![0i64; gm[3].len()]]);
    let vg = vorgaben_der_ebene(m, sc, e, is, v.schritt, v.lr_zaehler, v.lr_nenner, Gewichtsform::Int8);
    let (a_vorgaben, m_vorgaben) = vg.bloecke();
    let acc_mlp = vg.acc_mlp();
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
                let (gw, gs) = gewicht_aus_master_als(&mm[0], hs, MASTER_FRAC, form);
                let (uw, us) = gewicht_aus_master_als(&mm[1], hs, MASTER_FRAC, form);
                let (dw, ds) = gewicht_aus_master_als(&mm[2], is, MASTER_FRAC, form);
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
        // ⚑ **Der geteilte Experte rueckwaerts.** `block = gemischt +
        //   geteilt · s`: Der Gemischanteil bekommt `g` unveraendert (oben),
        //   das MLP `g · s`, das Tor `Σ g · geteilt · s · (1 − s)` als eine
        //   Zahl, die durch seine eine Zeile laeuft. Die Eingangsgradienten
        //   addieren sich, alle auf `act_frac`.
        let mut eingang_mlp: Vec<i64> = gr.eingang.iter().map(|x| i64::from(*x)).collect();
        if let (Some((ge, w)), Some(gt), Some(gsum)) = (&geteilt_gew, &teil.geteilt, g_geteilt.as_mut()) {
            let (g_mlp, g_tor) = torzweig_rueckwaerts(&g_block, &gt.ausgabe, gt.faktor, m.sigmoid_aus_frac, &acc_mlp);
            let mv = integer_llm_kernels::trainingsschritt::Mlpvorgaben {
                intermediate_size: ge.zwischen,
                gate_frac: ge.gate_frac,
                up_frac: ge.up_frac,
                down_in_frac: ge.down_in_frac,
                ..m_vorgaben
            };
            let mg = integer_llm_kernels::trainingsschritt::gradienten_des_mlp_aus_gradient(
                &g_mlp, &spur.norm_mitte[nr], &gt.spur,
                &w[0].0, &w[0].1, &w[1].0, &w[1].1, &w[2].0, &w[2].1,
                tab.silu, tab.silu_grad, Default::default(), mv,
            );
            for (ziel, quelle) in gsum.iter_mut().take(3).zip([&mg.gate, &mg.up, &mg.down]) {
                for (z, p) in ziel.iter_mut().zip(quelle.iter()) {
                    *z = z.saturating_add(i64::from(*p));
                }
            }
            let gx_tor = integer_llm_kernels::backward::linear_backward_summierend(
                &[g_tor], &spur.norm_mitte[nr], &w[3].0, hs, &w[3].1, m_bus, m_vorgaben.act_frac, &mut gsum[3],
            );
            for ((z, a), b) in eingang_mlp.iter_mut().zip(&mg.eingang).zip(&gx_tor) {
                *z += i64::from(*a) + i64::from(*b);
            }
        }
        let eingang_mlp: Vec<i32> = eingang_mlp.into_iter().map(begrenze).collect();
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
            &eingang_mlp,
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

    // Durch den Mischer: Achtsamkeit oder Zustandsschicht. Beide bekommen
    // den Gradienten auf demselben Bus und geben den nach ihrem normierten
    // Eingang auf `act_frac` zurueck.
    let g_attn: Vec<Vec<i32>> = g_residual
        .iter()
        .map(|z| {
            z.iter()
                .enumerate()
                .map(|(i, x)| begrenze(rescale_i64(i64::from(*x), sc.residual_mid_frac[i], a_bus)))
                .collect()
        })
        .collect();
    let (mischer_eingang, mischer_gradienten): (Vec<Vec<i32>>, Vec<Vec<i32>>) = match &*mischer {
        Mischerstand::Achtsamkeit(aufmerksamkeit) => {
            let a_breiten = [hs, hs, hs, m.num_heads * m.head_dim];
            let a_umgerechnet: Vec<(Vec<i8>, Vec<u8>)> = (0..4)
                .map(|n| gewicht_aus_master_als(&aufmerksamkeit[n], a_breiten[n], MASTER_FRAC, form))
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
                drehung: crate::trainingsschleife::achtsamkeitsdrehungen(ebene),
            };
            let a_grad = gradienten_der_aufmerksamkeit_aus_gradient(
                &g_attn,
                &spur.norm_ein,
                &spur.aufmerksamkeit,
                a_gew,
                tab.cos,
                tab.sin,
                crate::trainingsschleife::qk_vorgaben_der_ebene(m, e),
                crate::trainingsschleife::tor_vorgaben(m),
                a_vorgaben);
            (a_grad.eingang, vec![a_grad.q, a_grad.k, a_grad.v, a_grad.o])
        }
        Mischerstand::Zustand(z) => {
            let crate::model::Mischer::Zustand(zs) = &ebene.mischer else {
                return Err(Shardfehler::ArtPasstNicht { ebene: e });
            };
            let Some(zspur) = spur.zustand.as_deref() else {
                return Err(Shardfehler::ArtPasstNicht { ebene: e });
            };
            let zs_master = zustandsschicht_aus_mastern(zs, z, form);
            let gr = crate::zustandstraining::rueckwaerts(m, ebene, &zs_master, zspur, &g_attn, a_bus, ZUSTAND_ABSTAND);
            let als_grad = |v: Vec<i64>| -> Vec<i32> { v.into_iter().map(begrenze).collect() };
            (
                gr.eingang,
                vec![
                    als_grad(gr.in_proj_qkv),
                    als_grad(gr.in_proj_z),
                    als_grad(gr.in_proj_b),
                    als_grad(gr.in_proj_a),
                    als_grad(gr.out_proj),
                    als_grad(gr.faltung),
                ],
            )
        }
    };

    // Durch die erste Normierung und die erste Residualaddition.
    let mut eingang: Vec<Vec<i32>> = Vec::with_capacity(hidden.len());
    for nr in 0..hidden.len() {
        let g_norm = normrueckwaerts(
            &mischer_eingang[nr],
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
    let versatz = Gemischversatz::neu(mischer.matrizen(), router.len(), hs, is, n, geteilt.as_deref());
    let kn = vg.aufmerksamkeit.kennung;
    let kennungen: Vec<Matrixkennung> = (0..mischer.matrizen().len()).map(|nr| mischer.kennung(nr)).collect();
    for (nr, (mm, gg)) in mischer.matrizen_mut().iter_mut().zip(&mischer_gradienten).enumerate() {
        fort.tue(
            i,
            kennungen[nr],
            mm,
            gg,
            Schrittkennung { index_versatz: versatz.mischer[nr], ..kn },
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
    if let (Some(gsum), Some(gm)) = (&g_geteilt, geteilt.as_mut()) {
        for (nr, (mm, gg)) in gm.iter_mut().zip(gsum.iter()).enumerate() {
            let g32: Vec<i32> = gg.iter().copied().map(begrenze).collect();
            fort.tue(
                i,
                Matrixkennung::Geteilt(nr as u8),
                mm,
                &g32,
                Schrittkennung { index_versatz: versatz.geteilt[nr], ..kn },
                v.lr_nenner,
            );
        }
    }
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


#[cfg(test)]
mod torzweig {
    use super::*;

    /// ⚑ **Der Torzweig gegen die exakte Rechnung**, mit gemeinsamem Nenner
    /// in `i128` statt in Gleitkomma: `g · s / 2^f` je Kanal und
    /// `Σ g · geteilt / 2^acc · s · (2^f − s) / 2^(2f)`. Gerundet wird im
    /// Rechenpfad dreimal, also darf das Ergebnis um wenige Einheiten
    /// abweichen und nicht mehr.
    #[test]
    fn der_torzweig_rechnet_die_ableitung() {
        let f = 14u8;
        let g = [12_345i32, -9_876, 4_321, -32_000, 77, 0];
        let geteilt = [3_000i16, -2_500, 100, 1_234, -32_000, 9_999];
        let acc = [8u8, 9, 10, 8, 11, 7];
        for s in [0i64, 1, 5_000, 8_192, 11_000, 16_384] {
            let (g_mlp, g_tor) = torzweig_rueckwaerts(&g, &geteilt, s, f, &acc);
            for (i, x) in g.iter().enumerate() {
                let exakt = i128::from(*x) * i128::from(s);
                let ist = i128::from(g_mlp[i]) << f;
                assert!((ist - exakt).abs() <= 1i128 << (f - 1), "s {s}, Kanal {i}: {} statt {exakt}/2^f", g_mlp[i]);
            }
            // Gemeinsamer Nenner 2^(12 + 2f), 12 >= jedes acc.
            let mut zaehler: i128 = 0;
            for i in 0..g.len() {
                zaehler += (i128::from(g[i]) * i128::from(geteilt[i])) << (12 - acc[i]);
            }
            let zaehler = zaehler * i128::from(s) * i128::from((1i64 << f) - s);
            let nenner_bits = 12 + 2 * u32::from(f);
            let ist = i128::from(g_tor) << nenner_bits;
            let abw = (ist - zaehler).abs();
            // Drei Rundungen, jede hoechstens eine halbe Einheit ihrer Stufe;
            // die erste je Kanal, also bis zu sechs halbe Einheiten.
            assert!(abw <= 5i128 << nenner_bits, "s {s}: Tor {g_tor}, exakt {zaehler} / 2^{nenner_bits}");
        }
        // Ohne Tor (s = 0) und bei offenem Tor (s = 2^f) ist die Ableitung null.
        assert_eq!(torzweig_rueckwaerts(&g, &geteilt, 0, f, &acc).1, 0);
        assert_eq!(torzweig_rueckwaerts(&g, &geteilt, 1 << f, f, &acc).1, 0);
    }
}

#[cfg(test)]
mod raster {
    use super::*;

    /// Eine ternaere 1x128-Matrix mit dem Betrag `b` in ihrer einen Gruppe.
    fn ternaer(b: i16) -> crate::model::QTensor {
        let muster = vec![0b1010_1010u8 as i8; integer_llm_kernels::ternaer::BYTES_JE_GRUPPE];
        let daten = crate::model::Ternaerdaten::neu(
            crate::model::Gewichtsdaten::Speicher(muster),
            crate::loader::Kopfdaten::Speicher(vec![b]),
            1,
            integer_llm_kernels::ternaer::GRUPPE,
        )
        .expect("Ternaerdaten");
        crate::model::QTensor {
            data: std::sync::Arc::new(crate::model::Gewichtsdaten::Ternaer(Box::new(daten))),
            shape: vec![1, integer_llm_kernels::ternaer::GRUPPE],
            shifts: vec![7],
            drehung: None,
        }
    }

    /// ⛔️ **Fund 517:** Ein Betrag bis 127 ist im Raster des Trainings, einer
    /// darueber nicht; int8 ist es ohnehin.
    #[test]
    fn ein_betrag_ueber_127_ist_zu_fein() {
        assert!(!ternaer_zu_fein(&ternaer(127)));
        assert!(!ternaer_zu_fein(&ternaer(-127)));
        assert!(ternaer_zu_fein(&ternaer(128)));
        assert!(ternaer_zu_fein(&ternaer(-6692)));
        let int8 = crate::model::QTensor::aus_speicher(vec![127; 4], vec![2, 2], vec![0, 0]);
        assert!(!ternaer_zu_fein(&int8));
    }
}

#[cfg(test)]
mod zustandsmischer {
    use super::*;

    fn matrix(n: usize) -> Vec<Master> {
        vec![0; n]
    }

    /// ⚑ **Die Wuerfelbereiche eines Zustandsmischers folgen den Laengen**
    /// wie die der Achtsamkeit (Fund 518): sechs Bereiche ohne Ueberlappung,
    /// dahinter Router, Experten und der geteilte Experte.
    #[test]
    fn der_versatz_eines_zustandsmischers_folgt_den_laengen() {
        let mischer = Mischerstand::Zustand(Box::new([
            matrix(12 * 4),
            matrix(6 * 4),
            matrix(2 * 4),
            matrix(2 * 4),
            matrix(4 * 6),
            matrix(12 * 4),
        ]));
        let v = Gemischversatz::neu(mischer.matrizen(), 3 * 4, 4, 5, 2, None);
        assert_eq!(v.mischer, vec![0, 48, 72, 80, 88, 112]);
        assert_eq!(v.router, 160);
        assert_eq!(v.experte(0, 0), 172);
        assert_eq!(v.experte(1, 2), 172 + 60 + 40);
    }

    /// ⚑ **Die Kennungen haengen am Mischer**, und die Auswahl zaehlt die
    /// Zustandsschicht wie die Achtsamkeit nicht zum MLP-Teil.
    #[test]
    fn die_kennungen_des_zustandsmischers() {
        let mischer = Mischerstand::Zustand(Box::new(std::array::from_fn(|_| matrix(4))));
        assert_eq!(mischer.kennung(4), Matrixkennung::Zustand(4));
        let achtsam = Mischerstand::Achtsamkeit(Box::new(std::array::from_fn(|_| matrix(4))));
        assert_eq!(achtsam.kennung(3), Matrixkennung::Aufmerksamkeit(3));
        for n in 0..6 {
            assert!(Auswahl::Alles.erlaubt(Matrixkennung::Zustand(n)));
            assert!(!Auswahl::NurMlp.erlaubt(Matrixkennung::Zustand(n)));
            assert!(!Auswahl::NurAbwaerts.erlaubt(Matrixkennung::Zustand(n)));
        }
        let mut stand = Ebenenstand::Gemisch {
            mischer,
            router: matrix(4),
            geteilt: None,
            experten: std::collections::BTreeMap::new(),
        };
        assert!(stand.matrix_mut(Matrixkennung::Zustand(5)).is_some());
        assert!(stand.matrix_mut(Matrixkennung::Aufmerksamkeit(0)).is_none());
        assert_eq!(stand.matrizen().len(), 7, "sechs des Mischers und der Router");
    }
}
