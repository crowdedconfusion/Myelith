//! Die Trainingsschleife über eine Ebene: das, was zwei Maschinen
//! vergleichen (TRAINING V, dritter Teil).
//!
//! # ⚑ Was hier steht und warum es hier steht
//!
//! Bis zum 2026-09-04 lag diese Schleife in einer Testdatei, und der
//! Testclient hätte sie ein zweites Mal schreiben müssen. **Fund 178
//! hat am selben Tag gezeigt, was daraus wird**: Der LM-Kopf stand
//! dreimal wortgleich im Modul, und Fund 176 war die Rechnung dafür.
//! Also einmal, hier, und beide rufen es.
//!
//! # ⚑ Warum kein Verlustwert herauskommt
//!
//! Die Kreuzentropie braucht einen Logarithmus, **ihre Ableitung
//! nicht**: Sie ist `p − onehot`, und `p` liefert
//! [`softmax_ueber_vokabular`]. Der Gradient und damit jedes Gewicht
//! bleiben ganzzahlig.
//!
//! ⚑ **Der Verlustwert ist Diagnose für Menschen und steht deshalb
//! draussen.** Diese Datei liegt im Ganzzahl-Audit
//! (`tests/audit/test_no_float.py`), und das ist kein Formalismus,
//! sondern die Bauweise: Wer den Verlustwert zum Vergleichswert machte,
//! hinge die Bitgleichheit an einen Logarithmus. Was zwei Maschinen
//! vergleichen, ist [`Trainingsergebnis::abdruck`].
//!
//! Wer die Kreuzentropie sehen will, rechnet sie aus
//! [`Trainingsergebnis::logits_erst`] und
//! [`Trainingsergebnis::logits_letzt`]; die beiden liegen bei, genau
//! dafür.
//!
//! # ⚑ Was der Lauf zeigt und was nicht
//!
//! Trainiert wird die **letzte** Ebene gegen das nächste Token. Das ist
//! eine bewusste Verkürzung: Alle vierundzwanzig Ebenen als Master zu
//! halten kostet rund 1,4 GB, und der Gradientenweg wäre derselbe.
//!
//! **Gezeigt ist:** Ein echtes Ziel bewegt echte Gewichte. **Nicht
//! gezeigt ist:** dass ein Modell lernt. Dafür braucht es einen Korpus,
//! viele Schritte und eine Haltemenge, und genau daran ist am
//! 2026-08-22 schon einmal eine Messung gescheitert, die den
//! Trainingsverlust für den Beleg hielt.
//!
//! [`softmax_ueber_vokabular`]: integer_llm_kernels::softmax::softmax_ueber_vokabular

use integer_llm_kernels::backward::{
    kreuzentropie_gradient, linear_backward, silu_grad_aus_lut, rmsnorm_backward, Normskalen,
};
use integer_llm_kernels::optimierer::{
    ausserhalb_der_form, delta, schritt, trainingsabdruck, Master, Schrittkennung, MASTER_FRAC,
};
use integer_llm_kernels::rmsnorm::{rmsnorm_i16, Rmsnormspur};
use integer_llm_kernels::softmax::softmax_ueber_vokabular;
use integer_llm_kernels::trainingsschritt::{
    gewicht_aus_master, gradienten_der_ebene_aus_gradient, vorwaerts_der_ebene,
    Aufmerksamkeitsgewichte, Aufmerksamkeitsvorgaben, Ebenengewichte, Ebenentabellen,
    Ebenenvorgaben, Mlpvorgaben, Vorspannungen,
};

use crate::kv_cache::KVCache;
use crate::mitschnitt::Zwischenwerte;
use crate::model::{Feedforward, IntegerModel, QTensor};
use integer_llm_kernels::mlp::Mlpspur;

/// Was der Lauf rechnen soll.
#[derive(Debug, Clone)]
pub struct Trainingsvorgaben {
    /// Die Tokenfolge, auf der trainiert wird.
    pub folge: Vec<usize>,
    /// Das Wort, das nach der Folge kommen soll.
    pub ziel: usize,
    /// Wie viele Schritte.
    pub schritte: u64,
    /// Der Nenner der Schrittweite.
    ///
    /// # ⚑ Gemessen, nicht geraten (2026-09-04, Qwen2.5-0,5B)
    ///
    /// | Nenner | Kreuzentropie nach 30 Schritten | bewegte Gewichte in `o_proj` |
    /// |---|---|---|
    /// | 2^18 | 9,2204 auf 9,2099 | 192.649 von 802.816 |
    /// | 2^12 | 9,2204 auf **0,1371** | 751.918 von 802.816 |
    /// | 2^8 | 9,2204 auf 0,0008 nach fünf Schritten | 790.572 von 802.816 |
    ///
    /// ⚑ **Der erste Lauf sah aus wie ein kaputter Gradient und war eine
    /// zu kleine Schrittweite.** Bei 2^18 bleibt der grösste Gradient
    /// nach der Division unter der letzten Stelle des Masters, und nur
    /// ein Viertel der Gewichte bewegt sich überhaupt.
    pub lr_nenner: i64,
    /// **Der Zaehler der Lernrate.**
    ///
    /// ⛑ **Bis zum 2026-09-11 stand hier eine 1 im Rumpf**, und damit
    /// war `lr_nenner = 1` die groesste erreichbare Schrittweite. Auf
    /// Qwen3-0,6B verlaesst der Lauf damit die Uebertragungsform
    /// **nie**, auch nicht in dreihundert Schritten; die Gegenprobe zur
    /// Schranke prueft dort also nichts.
    ///
    /// ⚑ **Eine Schranke ohne erreichbaren Fall ist eine Behauptung.**
    /// Mit einem Zaehler laesst sich der Fall wieder herstellen, und
    /// zwar ohne den Normalbetrieb anzufassen: Die Vorgabe ist eins.
    pub lr_zaehler: i64,
    /// Die Skala, auf der der Kopf liefert.
    ///
    /// ⚑ **Nicht `config.logit_frac_bits`** (6, Fund 177): Die ist für
    /// Argmax gedacht und liest die exp-Tabelle viermal zu flach.
    pub logit_frac: u8,
    /// Die Skala der Wahrscheinlichkeiten und damit des Gradienten.
    ///
    /// ⚑ Muss über der Wortzahl liegen, sonst fällt jedes Wort auf null
    /// (Fund 177). `2^24 / 151.936 = 110` Einheiten je Wort im Mittel.
    pub prob_frac: u8,
    /// Der Lastausgleich, nur für ein Expertengemisch.
    pub lastausgleich: Lastausgleich,
}

/// Der Lastausgleich eines Expertengemisches, **ohne Zufall und ohne
/// Batch-Statistik**.
///
/// # ⚑ Warum die üblichen Verfahren hier ausscheiden
///
/// Der übliche Lastausgleich addiert einen Hilfsverlust über
/// **Batch-Statistiken** und oft Rauschen im Router. Beides ist hier
/// verboten: Rauschen ist nicht deterministisch, und eine Grösse über
/// den Batch machte das Ergebnis an Position *i* davon abhängig, welche
/// anderen Token zufällig danebenlagen. Das ist dieselbe Klasse wie das
/// für den Vorwärtspfad bereits verbotene Token-Dropping.
///
/// ⚑ **Die Antwort steht seit dem 2026-08-28 in `kernels::moe` und hatte
/// bis heute keinen Aufrufer.** [`Expertenwacht`] mittelt nicht über den
/// Batch, sondern **zählt über die Segmentfolge**, und das ist der
/// Unterschied ums Ganze: Die Batch-Zusammensetzung wählt der Miner, die
/// Segmentfolge legt das Protokoll fest. Zwei redundante Miner sehen
/// dieselbe, in derselben Reihenfolge.
///
/// **Dieselbe Einsicht wie „Loss-Free Balancing"** (Wang u.a. 2024, in
/// DeepSeek-V3 im Produktionseinsatz), unabhängig gefunden und mit einer
/// schärferen Begründung: Dort ist der Grund Kausalität, hier
/// Determinismus.
///
/// # ⚑ Zwei Zustände, zwei Mittel
///
/// | Zustand | Was passiert | Gegenmittel |
/// |---|---|---|
/// | **gewählt, aber Gewicht auf null** | Softmax-Gradient exakt null | Boden (θ_v 0.18.0) und [`router_spreizung`] |
/// | **nie gewählt** | nie gerechnet, nie ein Gradient, **still tot** | [`Expertenwacht`] |
///
/// Der zweite ist der stillere: Bei 128 Experten und Top-8 ist „nicht
/// gewählt" der Normalzustand für 120 von ihnen je Token; tot ist ein
/// Experte erst, wenn es über **viele** Token so bleibt. Keine Zahl
/// weicht dabei ab.
///
/// [`Expertenwacht`]: integer_llm_kernels::moe::Expertenwacht
/// [`router_spreizung`]: integer_llm_kernels::backward::router_spreizung
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lastausgleich {
    /// Nach wie vielen Segmenten ohne Wahl ein Experte als hungernd
    /// gilt.
    pub geduld: u32,
    /// Wie stark ein hungernder Experte geschoben wird, in
    /// Logit-Einheiten. **Null schaltet die Wacht ab.**
    pub wacht_staerke: i32,
    /// Ab welchem Logit-Abstand die Spreizungsstrafe greift. **Null
    /// schaltet sie ab.**
    pub spreizung_schwelle: i32,
    /// Wie stark die Spreizungsstrafe gedämpft wird (Rechtsschieber).
    pub spreizung_daempfung: u8,
}

impl Lastausgleich {
    /// Abgeschaltet: der Vergleichsfall, gegen den gemessen wird.
    pub fn aus() -> Self {
        Self { geduld: 0, wacht_staerke: 0, spreizung_schwelle: 0, spreizung_daempfung: 0 }
    }
}

impl Trainingsvorgaben {
    /// Die Vorgaben, mit denen die Messungen dieses Projekts laufen.
    ///
    /// ⚑ **Eine Quelle, damit Test und Testclient dasselbe rechnen.**
    /// Zwei getippte Fassungen derselben Zahlen ergäben zwei Abdrücke,
    /// und niemand wüsste, welcher gemeint war.
    pub fn vorgabe() -> Self {
        Self {
            folge: vec![9707, 374, 264, 1273, 315, 279],
            ziel: 4108,
            schritte: 30,
            // ⛑ **Neu gemessen am 2026-09-11**, als das Ankermodell von
            // Qwen2.5-0,5B auf Qwen3-0,6B wechselte. Mit `1 << 12` fiel
            // die Kreuzentropie in dreissig Schritten nur von 8,00 auf
            // 7,29, und der Argmax traf das Ziel nicht. Gemessen mit
            // `examples/lernrate_messen.rs`:
            //
            // | Nenner | 30 Schritte | 60 Schritte |
            // |---|---|---|
            // | `1 << 12` | 7,2865 (daneben) | 5,5108 (daneben) |
            // | `1 << 11` | 5,5903 (daneben) | 0,4456 (trifft) |
            // | `1 << 10` | **0,4681 (trifft)** | 0,1818 |
            // | `1 << 9`  | 0,1609 (trifft) | 0,1353 |
            //
            // ⚑ **Der sanfteste Wert, der in dreissig Schritten trifft.**
            // `1 << 9` kaeme naeher an die 0,137 des alten Ankers heran
            // und ist doppelt so grob; eine Lernrate wird nicht dadurch
            // besser, dass die Endzahl huebscher aussieht.
            //
            // ⚠️ **Eine Lernrate gilt fuer ein Modell.** Sie haengt an
            // Tiefe und Breite, und beides hat sich geaendert (24 auf
            // 28 Ebenen, 896 auf 1024). Wer das naechste Modell
            // aufnimmt, misst sie neu.
            lr_nenner: 1 << 10,
            lr_zaehler: 1,
            logit_frac: 16,
            prob_frac: 24,
            // ⚑ **Gemessen und nicht geraten**, siehe den Changelog von
            // `kernels` v0.42.0. Geduld 4 heisst: Wer vier Segmente in
            // Folge nicht drankam, hungert.
            lastausgleich: Lastausgleich {
                geduld: 4,
                wacht_staerke: 64,
                spreizung_schwelle: 8192,
                spreizung_daempfung: 4,
            },
        }
    }
}

/// Was der Lauf hinterlässt. ⚑ **Alles ganzzahlig.**
#[derive(Debug, Clone)]
pub struct Trainingsergebnis {
    /// **Der Wert, den zwei Maschinen vergleichen.**
    pub abdruck: String,
    /// Die Logits der letzten Position **vor** dem ersten Schritt.
    pub logits_erst: Vec<i32>,
    /// Die Logits der letzten Position **nach** dem letzten Schritt.
    pub logits_letzt: Vec<i32>,
    /// Wie viele der fortgeschriebenen Gewichte sich insgesamt bewegt
    /// haben, gegenüber dem Stand vor dem ersten Schritt.
    pub bewegte_gewichte: usize,
    /// Wie viele Gewichte die Ebene hat.
    pub gewichte_gesamt: usize,
    /// Welche Ebene trainiert wurde.
    pub ebene: usize,
    /// Wie viele **verschiedene** Experten der Router im Lauf gewählt
    /// hat, und wie viele es überhaupt gibt.
    ///
    /// ⚑ **Die Zahl, an der der Lastausgleich gemessen wird.** Ein
    /// Experte, den der Router nie wählt, wird nie gerechnet, bekommt
    /// nie einen Gradienten und ändert sich nie: **still tot**. Bei 128
    /// Experten und Top-8 ist „nicht gewählt" der Normalzustand für 120
    /// je Token; tot ist er erst, wenn es über viele Segmente so bleibt.
    ///
    /// `None` bei einer dichten Ebene, dort gibt es keine Experten.
    pub experten_beruehrt: Option<(usize, usize)>,
    /// Bei welchem Schritt der Lauf die Übertragungsform verlassen hat,
    /// falls er es getan hat.
    ///
    /// # ⚑ Warum die Schleife dann aufhört, statt weiterzurechnen
    ///
    /// Ein Master jenseits von [`ausserhalb_der_form`] lässt sich nicht
    /// mehr als `i8` mit Zeilenversatz ins Artefakt zurückschreiben.
    /// Weitere Schritte darauf sind verlorene Rechenzeit, und das
    /// Ergebnis wäre ohnehin nicht übertragbar.
    ///
    /// ⚑ **Der Abbruch ist deterministisch und deshalb unschädlich für
    /// den Konsens.** Die Bedingung ist eine reine Funktion der
    /// Gewichte; zwei Maschinen, die dasselbe Segment rechnen, hören am
    /// selben Schritt auf und kommen zum selben Abdruck.
    ///
    /// ⚑ **Und deshalb steht sie hier und nicht als Panik.** Gemessen am
    /// 2026-09-05 auf dem echten 30B verlässt der Router bei Lernrate
    /// 2⁻¹⁰ den Bereich bei Schritt 67. Ein Pod, dem das passiert, muss
    /// **melden** können; ein Absturz ist von einem ausgefallenen Miner
    /// nicht zu unterscheiden.
    pub aus_der_form: Option<u64>,
    /// Das Commitment über **Δm**, die Änderung an den Gewichten.
    ///
    /// # ⚑ Warum über die Differenz und nicht über den Endzustand
    ///
    /// [`Trainingsergebnis::abdruck`] geht über die fortgeschriebenen
    /// Gewichte und beantwortet die Frage des Miettags: **Haben zwei
    /// Maschinen dieselbe Arbeit gleich gerechnet?**
    ///
    /// Die Aggregation stellt eine andere Frage:
    ///
    /// ```text
    /// m_{v+1} = klemmen(m_v + Σ Δm_i)
    /// ```
    ///
    /// Dabei rechnen viele Miner **verschiedene** Chargen gegen
    /// **denselben** Ausgangszustand. Ihre Endzustände addieren sich
    /// nicht; ihre Differenzen tun es. Deshalb trägt ein
    /// Trainingssegment ein Commitment über Δm, und deshalb steht es
    /// hier **daneben** statt an seiner Stelle.
    ///
    /// ⚑ **Beide werden gebraucht, und keines ersetzt das andere.**
    pub delta_commitment: String,
}

impl Trainingsergebnis {
    /// Das Wort mit dem grössten Logit, vor und nach dem Lauf.
    pub fn argmax(&self) -> (usize, usize) {
        let groesstes = |l: &[i32]| {
            l.iter().enumerate().max_by_key(|(_, v)| **v).map(|(i, _)| i).unwrap_or(0)
        };
        (groesstes(&self.logits_erst), groesstes(&self.logits_letzt))
    }
}

/// Hebt ein int8-Gewicht mit Zeilenversatz auf den Master.
pub(crate) fn master_aus_gewicht(t: &QTensor) -> Vec<Master> {
    let in_features = t.shape[1];
    t.data
        .iter()
        .enumerate()
        .map(|(i, w)| i32::from(*w) << (MASTER_FRAC - t.shifts[i / in_features]))
        .collect()
}

/// Der Weg vom Ausgang der Ebene bis zum Gradienten, der dorthin
/// zurückkommt: Abschlussnormierung, Kopf, Softmax, Kreuzentropie, und
/// denselben Weg zurück.
///
/// Gibt die Logits mit heraus, damit der Aufrufer daraus eine Diagnose
/// rechnen kann, ohne den Weg ein zweites Mal zu gehen.
/// Der Verlustgradient an **jeder** Position, gegen das jeweils nächste
/// Wort.
///
/// # ⚑ Teacher Forcing, und warum es mehr ist als eine Bequemlichkeit
///
/// [`gradient_vom_ziel`] bedient **eine** Position: Aus einer Folge der
/// Länge `L` lernt ein Schritt genau ein Wort. Der Vorwärtspass rechnet
/// aber alle `L` Positionen ohnehin, und an jeder steht ein Ziel fest,
/// nämlich das nächste Wort der Folge.
///
/// ⚑ **Damit lernt derselbe Vorwärtspass `L−1` Mal so viel.** Das ist
/// keine Feinheit: Ein Trainingssegment kostet Rechenzeit für den
/// Vorwärtspass, und wer davon nur eine Position auswertet, bezahlt das
/// Ganze und nimmt einen Bruchteil mit.
///
/// ⚑ **Und ein Qualitätsmass wird erst dadurch möglich.** Perplexität
/// ist der Verlust über **alle** Positionen; ein Training, das nur die
/// letzte kennt, lässt sich daran nicht messen.
///
/// **Returns:** je Position der Gradient auf dem Residualstrom, und je
/// Position die Logits, aus denen ein Aufrufer einen Verlust messen kann.
///
/// ⚑ **Die letzte Position hat kein Ziel** und bekommt einen
/// Nullgradienten: Was nach ihr kommt, steht nicht in der Folge.
///
/// ⚑ **Der Verlust wird hier NICHT gerechnet.** Er wäre eine
/// Gleitkommazahl, und diese Datei liegt im Heisspfad des
/// Gleitkomma-Audits. Wer ihn braucht, nimmt
/// [`crate::messung::kreuzentropie_aus_logits`]; ein erster Entwurf am
/// 2026-09-06 rechnete ihn hier, und das Audit hat ihn zurückgewiesen.
pub fn gradienten_je_position(
    m: &IntegerModel,
    y: &[Vec<i16>],
    v: &Trainingsvorgaben,
) -> (Vec<Vec<i32>>, Vec<Vec<i32>>) {
    gradienten_je_position_mit_kopf(m, y, v, None)
}

/// Wie [`gradienten_je_position`], sammelt aber zugleich den Gradienten
/// der verfolgten Kopfzeilen ein (siehe [`Kopfsammlung`]).
pub fn gradienten_je_position_mit_kopf(
    m: &IntegerModel,
    y: &[Vec<i16>],
    v: &Trainingsvorgaben,
    mut kopf: Option<&mut Kopfsammlung>,
) -> (Vec<Vec<i32>>, Vec<Vec<i32>>) {
    let mut aus: Vec<Vec<i32>> = Vec::with_capacity(y.len());
    let mut alle_logits: Vec<Vec<i32>> = Vec::with_capacity(y.len());
    for (i, zeile) in y.iter().enumerate() {
        // Das Ziel der Position `i` ist das Wort an `i+1`.
        let Some(ziel) = v.folge.get(i + 1).copied() else {
            aus.push(vec![0i32; m.hidden_size]);
            alle_logits.push(Vec::new());
            continue;
        };
        let vorgabe = Trainingsvorgaben { ziel, ..v.clone() };
        let (logits, g) =
            gradient_vom_ziel_mit_kopf(m, zeile, &vorgabe, kopf.as_deref_mut());
        alle_logits.push(logits);
        aus.push(g);
    }
    (aus, alle_logits)
}

/// Der Sammler fuer die **Zeilen des Ablesekopfes**.
///
/// # ⚑ Warum der Kopf einen eigenen Sammler braucht
///
/// Bis zum 2026-09-08 war der Kopf **gar nicht im Trainingspfad**.
/// [`crate::shardtraining::Ebenenstand::matrizen_veraenderlich`] gibt
/// die Aufmerksamkeit, den Router und die Experten heraus, und damit
/// endet die Liste: Das Modell konnte verstellen, **was** der verborgene
/// Zustand ist, aber nie, **wie** ein Zustand auf ein Token zeigt.
///
/// ⚑ **Genau das ist die Konzentration, die Fund 201 vermisst.** Drei
/// Verfahren nacheinander zeigten dasselbe: Je gleichmaessiger ein
/// Schritt ueber die Ebenen verteilt wird, desto schlechter lernt das
/// Modell eine einzelne Tatsache. Die Kopfzeile eines Tokens sind
/// `hidden_size` Zahlen, die unmittelbar auf genau dieses Token wirken,
/// und sie lagen fest.
///
/// # Warum nur ein Teil der Zeilen
///
/// Die volle Kopfmatrix ist `vocab × hidden`, bei Qwen2.5-0,5B also
/// 136 Millionen Werte, und ihr Gradient als `i64` waere ein Gigabyte
/// **je Position**. Er ist aber ein aeusseres Produkt,
/// `dL/dW[i][j] = g[i] · x[j]`, und `g[i] = p[i] − 1[i = Ziel]`: gross
/// in der Zielzeile, winzig ueberall sonst. Verfolgt werden deshalb die
/// Zeilen der Token, die im Korpus **vorkommen**; jede andere Zeile
/// bekaeme ohnehin nur ihr `p[i]`.
///
/// ⚑ **Kosten:** Der Vorwaertslauf des Kopfes rechnet `vocab × hidden`
/// je Position und ist damit teurer als alle vier Ebenen zusammen. Ein
/// paar tausend verfolgte Zeilen sind daneben ein Prozentbruchteil.
pub struct Kopfsammlung {
    /// Token zu seiner laufenden Nummer in [`Self::summen`].
    zeile_von_token: std::collections::BTreeMap<u32, usize>,
    /// Je verfolgter Zeile `hidden` Summen, hintereinander.
    ///
    /// ⚑ Wie bei [`integer_llm_kernels::optimierer::sammle_roh`] steht
    /// hier der **negative** Gradient, denn der Schritt addiert.
    summen: Vec<i64>,
    hidden: usize,
    /// Wie viele Positionen eingegangen sind, fuer den Bericht.
    positionen: u64,
}

impl Kopfsammlung {
    /// `tokens` sind die Zeilen, die verfolgt werden; Duplikate schaden
    /// nicht.
    pub fn neu(tokens: &[u32], hidden: usize) -> Self {
        let mut zeile_von_token = std::collections::BTreeMap::new();
        for t in tokens {
            let n = zeile_von_token.len();
            zeile_von_token.entry(*t).or_insert(n);
        }
        let summen = vec![0i64; zeile_von_token.len() * hidden];
        Self { zeile_von_token, summen, hidden, positionen: 0 }
    }

    pub fn zeilen(&self) -> usize {
        self.zeile_von_token.len()
    }

    pub fn positionen(&self) -> u64 {
        self.positionen
    }

    /// Die verfolgten Token in aufsteigender Ordnung.
    pub fn token(&self) -> Vec<u32> {
        self.zeile_von_token.keys().copied().collect()
    }

    /// Die Summen einer Zeile, zum Anwenden des Schritts.
    pub fn summe_mut(&mut self, token: u32) -> Option<&mut [i64]> {
        let i = *self.zeile_von_token.get(&token)?;
        Some(&mut self.summen[i * self.hidden..(i + 1) * self.hidden])
    }

    /// Alles auf null, fuer den naechsten Durchgang.
    pub fn leeren(&mut self) {
        self.summen.iter_mut().for_each(|v| *v = 0);
        self.positionen = 0;
    }

    /// Eine Position aufnehmen.
    ///
    /// `g_logits` ist der Gradient auf den Logits ueber das ganze
    /// Vokabular, `normed` der **normierte** Strom, auf dem der Kopf
    /// rechnet. Beides zusammen ist das aeussere Produkt.
    pub fn aufnehmen(&mut self, g_logits: &[i32], normed: &[i16]) {
        debug_assert_eq!(normed.len(), self.hidden, "Kopfsammlung: Breite passt nicht");
        for (tok, zeile) in &self.zeile_von_token {
            let Some(g) = g_logits.get(*tok as usize).copied() else { continue };
            if g == 0 {
                continue;
            }
            let g = g as i64;
            let ab = zeile * self.hidden;
            for (j, x) in normed.iter().enumerate() {
                let s = &mut self.summen[ab + j];
                *s = s.saturating_sub(g * *x as i64);
            }
        }
        self.positionen += 1;
    }
}


pub fn gradient_vom_ziel(
    m: &IntegerModel,
    y: &[i16],
    v: &Trainingsvorgaben,
) -> (Vec<i32>, Vec<i32>) {
    gradient_vom_ziel_mit_kopf(m, y, v, None)
}

/// Wie [`gradient_vom_ziel`], nimmt aber den Kopfgradienten mit.
///
/// ⚑ Er faellt im Rueckwaertslauf **ohnehin an**: `linear_backward`
/// gibt `(dL/dx, dL/dW)` zurueck, und der Aufrufer hat den zweiten Wert
/// bis zum 2026-09-08 weggeworfen. Hier wird er nicht neu gerechnet,
/// sondern aus `g_logits` und `normed` fuer die verfolgten Zeilen
/// gebildet; die volle Matrix zu materialisieren waere ein Gigabyte.
pub fn gradient_vom_ziel_mit_kopf(
    m: &IntegerModel,
    y: &[i16],
    v: &Trainingsvorgaben,
    kopf: Option<&mut Kopfsammlung>,
) -> (Vec<i32>, Vec<i32>) {
    let mut spur = Rmsnormspur::Leer;
    let logits = m.head_logits_mit_spur(y, v.logit_frac, Some(&mut spur));
    let p = softmax_ueber_vokabular(
        &logits,
        v.logit_frac,
        &m.exp_lut,
        m.config.exp_input_frac,
        v.prob_frac,
    );
    let g_logits = kreuzentropie_gradient(&p, v.ziel, v.prob_frac);

    // ⚑ Der Kopf rechnet auf dem **normierten** Strom; `linear_backward`
    // braucht genau den, nicht `y`.
    let normed = rmsnorm_i16(
        y,
        &m.final_residual_frac,
        &m.final_norm_gamma.data,
        &m.final_norm_gamma.shifts,
        &m.rsqrt_lut,
        m.config.rsqrt_input_shift,
        m.config.rsqrt_output_frac,
        m.inv_n_q20,
        m.final_norm_frac,
    );
    if let Some(k) = kopf {
        k.aufnehmen(&g_logits, &normed);
    }

    let (g_normed, _) = linear_backward(
        &g_logits,
        &normed,
        &m.lm_head.data,
        m.hidden_size,
        &m.lm_head.shifts,
        v.prob_frac,
        m.final_norm_frac,
    );
    let Rmsnormspur::Wert { r, norm_frac, ref_shift } = spur else {
        // Erreichbar nur, wenn der Strom durchweg null ist; dann ist auch
        // der Gradient null, und ein Schritt darauf bewegt nichts.
        return (logits, vec![0i32; m.hidden_size]);
    };
    let (g_y, _) = rmsnorm_backward(
        &g_normed,
        y,
        &m.final_residual_frac,
        &m.final_norm_gamma.data,
        &m.final_norm_gamma.shifts,
        Normskalen {
            r,
            norm_frac,
            ref_shift,
            inv_n_q20: m.inv_n_q20,
            g_frac: m.final_norm_frac,
            gx_frac: m.final_residual_frac[0],
        },
    );
    (logits, g_y)
}

// ⛑ **Hier stand bis zum 2026-09-07 ein verwaister Doc-Kommentar**
// („Trainiert die letzte Ebene gegen das naechste Token"). Die
// Funktion, die er beschrieb, gibt es seit der Umstellung auf
// `shardtraining` nicht mehr; der Kommentar haftete seither an
// `master_der_ebene` und beschrieb dort etwas voellig anderes.
// ===========================================================================
// Der Zuschnitt einer Ebene, an **einer** Stelle
// ===========================================================================
//
// ⚑ **Herausgezogen am 2026-09-05, und zwar bevor der zweite Aufrufer
// entstand.** `shardtraining` braucht denselben Zuschnitt, und ein
// zweiter Aufbau derselben Vorgaben waere eine zweite Wahrheit ueber die
// Zahlenskalen einer Ebene. Genau diese Sorte Doppelung hat dieses
// Projekt als Fund 34, 111 und 143 gefunden.

/// Die sieben Gewichtsmatrizen einer dichten Ebene als Master.
pub(crate) fn master_der_ebene(ebene: &crate::model::TransformerLayer) -> Option<[Vec<Master>; 7]> {
    let Feedforward::Dense(mlp) = &ebene.ffn else {
        return None;
    };
    Some([
        master_aus_gewicht(&ebene.q_proj),
        master_aus_gewicht(&ebene.k_proj),
        master_aus_gewicht(&ebene.v_proj),
        master_aus_gewicht(&ebene.o_proj),
        master_aus_gewicht(&mlp.gate_proj),
        master_aus_gewicht(&mlp.up_proj),
        master_aus_gewicht(&mlp.down_proj),
    ])
}

/// Die Zeilenbreiten der sieben Matrizen, in derselben Reihenfolge.
pub(crate) fn breiten_der_ebene(m: &IntegerModel, is: usize) -> [usize; 7] {
    [
        m.hidden_size,
        m.hidden_size,
        m.hidden_size,
        m.num_heads * m.head_dim,
        m.hidden_size,
        m.hidden_size,
        is,
    ]
}

/// Die Vorgaben einer Ebene für einen Trainingsschritt.
///
/// ⚑ **`ebene` ist die GLOBALE Ebenennummer**, und daran hängt die
/// Bitgleichheit zwischen geshardetem und Einzelknoten-Training: Der
/// Würfel des stochastischen Rundens ist eine reine Funktion aus
/// `(ebene, schritt, index)`. Ein Shard, der seine Ebenen bei null
/// durchnummerierte, würfelte anders, und niemand sähe es dem Ergebnis
/// an: Es wäre ein plausibles Delta, nur ein anderes.
/// Die QK-Normierung einer Ebene, falls das Modell sie hat (Qwen3).
///
/// ⚑ **Eine Stelle und nicht zwei.** Die Vorgaben werden an zwei Orten
/// gebaut, und eine zweite Abschrift dieser Zuordnung liefe irgendwann
/// auseinander: Dann traeniert der eine Weg mit Normierung und der
/// andere ohne, und beide sehen richtig aus.
pub(crate) fn qk_vorgaben_der_ebene(
    m: &IntegerModel,
    e: usize,
) -> Option<integer_llm_kernels::trainingsschritt::QkNormVorgaben<'_>> {
    let qkn = m.layers[e].qk_norm.as_ref()?;
    Some(integer_llm_kernels::trainingsschritt::QkNormVorgaben {
        q_gamma: &qkn.q_gamma.data,
        q_gamma_shifts: &qkn.q_gamma.shifts,
        k_gamma: &qkn.k_gamma.data,
        k_gamma_shifts: &qkn.k_gamma.shifts,
        q_out_frac: qkn.q_out_frac,
        k_out_frac: qkn.k_out_frac,
        rsqrt_lut: &m.rsqrt_lut,
        rsqrt_input_shift: m.config.rsqrt_input_shift,
        rsqrt_output_frac: m.config.rsqrt_output_frac,
    })
}

pub(crate) fn vorgaben_der_ebene<'a>(
    m: &'a IntegerModel,
    sc: &'a crate::model::LayerScales,
    e: usize,
    is: usize,
    schritt: u64,
    lr_zaehler: i64,
    lr_nenner: i64,
) -> Ebenenvorgaben<'a> {
    let kennung = Schrittkennung { ebene: e as u32, schritt, index_versatz: 0 };
    Ebenenvorgaben {
        qk_norm: qk_vorgaben_der_ebene(m, e),
        aufmerksamkeit: Aufmerksamkeitsvorgaben {
            hidden_size: m.hidden_size,
            num_heads: m.num_heads,
            num_kv_heads: m.num_kv_heads,
            head_dim: m.head_dim,
            act_frac: sc.norm_attn_frac,
            q_frac: sc.q_frac,
            k_frac: sc.k_frac,
            v_frac: sc.v_frac,
            attn_out_frac: sc.attn_out_frac,
            aus_frac: 0,
            master_frac: MASTER_FRAC,
            score_frac: m.config.score_frac_bits,
            prob_frac: m.config.prob_frac_bits,
            exp_input_frac: m.config.exp_input_frac,
            rope_frac: m.config.rope_frac_bits,
            positionsversatz: 0,
            lr_zaehler,
            lr_nenner,
            kennung,
        },
        mlp: Mlpvorgaben {
            hidden_size: m.hidden_size,
            intermediate_size: is,
            act_frac: sc.norm_mlp_frac,
            gate_frac: sc.gate_frac,
            up_frac: sc.up_frac,
            down_in_frac: sc.down_in_frac,
            aus_frac: 0,
            master_frac: MASTER_FRAC,
            silu_in_frac: m.config.silu_in_frac,
            silu_lut_offset: m.config.silu_lut_offset,
            silu_out_frac: m.config.silu_out_frac,
            lr_zaehler,
            lr_nenner,
            kennung,
        },
        residual_in_frac: &sc.residual_in_frac,
        residual_mid_frac: &sc.residual_mid_frac,
        // ⚑ **Die Ausgangsskala ist die EINGANGSSKALA DER NÄCHSTEN
        // EBENE**, nicht die des Modellausgangs (Fund 188, 2026-09-06).
        //
        // Hier stand `&m.final_residual_frac` für **jede** Ebene. Das
        // war richtig, solange nur die **letzte** trainiert wurde, denn
        // dort fallen beide zusammen. Sobald Ebenen verkettet werden,
        // schreibt jede auf eine fremde Skala, und der Residualstrom ist
        // ab der ersten unbrauchbar.
        //
        // ⚑ **Gemessen:** Eine Perplexität von 30 474 statt 15 auf
        // WikiText-2. `model.rs` macht es seit jeher richtig
        // (`layers[i+1].scales.residual_in_frac`, sonst
        // `final_residual_frac`); der Trainingspfad hatte eine zweite,
        // falsche Fassung derselben Regel.
        aus_frac: if e + 1 < m.num_layers {
            &m.layers[e + 1].scales.residual_in_frac
        } else {
            &m.final_residual_frac
        },
        rsqrt_input_shift: m.config.rsqrt_input_shift,
        rsqrt_output_frac: m.config.rsqrt_output_frac,
        inv_n_q20: m.inv_n_q20,
    }
}

/// Die Gewichte einer Ebene, aus den umgerechneten Mastern.
pub(crate) fn gewichte_der_ebene<'a>(
    umgerechnet: &'a [(Vec<i8>, Vec<u8>)],
    ebene: &'a crate::model::TransformerLayer,
) -> Ebenengewichte<'a> {
    Ebenengewichte {
        aufmerksamkeit: Aufmerksamkeitsgewichte {
            q: &umgerechnet[0].0,
            q_skalen: &umgerechnet[0].1,
            k: &umgerechnet[1].0,
            k_skalen: &umgerechnet[1].1,
            v: &umgerechnet[2].0,
            v_skalen: &umgerechnet[2].1,
            o: &umgerechnet[3].0,
            o_skalen: &umgerechnet[3].1,
        },
        gate: &umgerechnet[4].0,
        gate_skalen: &umgerechnet[4].1,
        up: &umgerechnet[5].0,
        up_skalen: &umgerechnet[5].1,
        down: &umgerechnet[6].0,
        down_skalen: &umgerechnet[6].1,
        gamma_ein: &ebene.input_layernorm_gamma.data,
        gamma_ein_skalen: &ebene.input_layernorm_gamma.shifts,
        gamma_mitte: &ebene.post_attention_layernorm_gamma.data,
        gamma_mitte_skalen: &ebene.post_attention_layernorm_gamma.shifts,
    }
}

/// Die Vorspannungen einer Ebene, falls sie welche hat.
pub(crate) fn vorspannungen_der_ebene(
    ebene: &crate::model::TransformerLayer,
) -> Option<Vorspannungen<'_>> {
    match (&ebene.q_bias, &ebene.k_bias, &ebene.v_bias) {
        (Some(qb), Some(kb), Some(vb)) => Some(Vorspannungen {
            q: &qb.data,
            q_skalen: &qb.shifts,
            k: &kb.data,
            k_skalen: &kb.shifts,
            v: &vb.data,
            v_skalen: &vb.shifts,
        }),
        _ => None,
    }
}

pub fn trainingsschleife(
    m: &IntegerModel,
    v: &Trainingsvorgaben,
) -> Result<Trainingsergebnis, String> {
    if v.folge.is_empty() {
        return Err("Trainingsschleife: die Folge ist leer".to_string());
    }
    if v.ziel >= m.vocab_size {
        return Err(format!(
            "Trainingsschleife: Zielwort {} liegt ausserhalb des Vokabulars ({})",
            v.ziel, m.vocab_size
        ));
    }
    let e = m.num_layers - 1;
    let ebene = &m.layers[e];
    let mlp = match &ebene.ffn {
        Feedforward::Dense(mlp) => mlp,
        // ⚑ **Seit dem 2026-09-05 kein Abbruch mehr, sondern ein
        // zweiter Weg.** Er trainiert den Expertenblock statt der
        // dichten Einheit; alles davor und danach ist dasselbe.
        Feedforward::Moe(_) => return gemischschleife(m, v),
    };

    // Die eingefrorenen Ebenen einmal rechnen.
    let mut cache = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();
    for (pos, tid) in v.folge.iter().enumerate() {
        let start = m.embed_token(*tid);
        let _ = m.run_layers_mit_mitschnitt(start, pos, &mut cache, 0, m.num_layers, &mut auf);
    }
    let hidden: Vec<Vec<i16>> = (0..v.folge.len())
        .map(|p| auf.ebenen()[p * m.num_layers + e].residual_ein.clone())
        .collect();

    let sc = &ebene.scales;
    let is = mlp.gate_proj.shape[0];
    let mut master = [
        master_aus_gewicht(&ebene.q_proj),
        master_aus_gewicht(&ebene.k_proj),
        master_aus_gewicht(&ebene.v_proj),
        master_aus_gewicht(&ebene.o_proj),
        master_aus_gewicht(&mlp.gate_proj),
        master_aus_gewicht(&mlp.up_proj),
        master_aus_gewicht(&mlp.down_proj),
    ];
    let anfang: Vec<Vec<Master>> = master.to_vec();
    let grad_lut = silu_grad_aus_lut(&m.silu_lut);
    let vorsp = match (&ebene.q_bias, &ebene.k_bias, &ebene.v_bias) {
        (Some(qb), Some(kb), Some(vb)) => Some(Vorspannungen {
            q: &qb.data,
            q_skalen: &qb.shifts,
            k: &kb.data,
            k_skalen: &kb.shifts,
            v: &vb.data,
            v_skalen: &vb.shifts,
        }),
        _ => None,
    };
    let breiten = [
        m.hidden_size,
        m.hidden_size,
        m.hidden_size,
        m.num_heads * m.head_dim,
        m.hidden_size,
        m.hidden_size,
        is,
    ];

    let mut logits_erst = Vec::new();
    let mut logits_letzt = Vec::new();
    let mut aus_der_form: Option<u64> = None;
    for s in 0..v.schritte {
        let vg = Ebenenvorgaben {
            qk_norm: qk_vorgaben_der_ebene(m, 0),
            aufmerksamkeit: Aufmerksamkeitsvorgaben {
                hidden_size: m.hidden_size,
                num_heads: m.num_heads,
                num_kv_heads: m.num_kv_heads,
                head_dim: m.head_dim,
                act_frac: sc.norm_attn_frac,
                q_frac: sc.q_frac,
                k_frac: sc.k_frac,
                v_frac: sc.v_frac,
                attn_out_frac: sc.attn_out_frac,
                aus_frac: 0,
                master_frac: MASTER_FRAC,
                score_frac: m.config.score_frac_bits,
                prob_frac: m.config.prob_frac_bits,
                exp_input_frac: m.config.exp_input_frac,
                rope_frac: m.config.rope_frac_bits,
                positionsversatz: 0,
                lr_zaehler: 1,
                lr_nenner: v.lr_nenner,
                kennung: Schrittkennung { ebene: e as u32, schritt: s, index_versatz: 0 },
            },
            mlp: Mlpvorgaben {
                hidden_size: m.hidden_size,
                intermediate_size: is,
                act_frac: sc.norm_mlp_frac,
                gate_frac: sc.gate_frac,
                up_frac: sc.up_frac,
                down_in_frac: sc.down_in_frac,
                aus_frac: 0,
                master_frac: MASTER_FRAC,
                silu_in_frac: m.config.silu_in_frac,
                silu_lut_offset: m.config.silu_lut_offset,
                silu_out_frac: m.config.silu_out_frac,
                lr_zaehler: 1,
                lr_nenner: v.lr_nenner,
                kennung: Schrittkennung { ebene: e as u32, schritt: s, index_versatz: 0 },
            },
            residual_in_frac: &sc.residual_in_frac,
            residual_mid_frac: &sc.residual_mid_frac,
            aus_frac: &m.final_residual_frac,
            rsqrt_input_shift: m.config.rsqrt_input_shift,
            rsqrt_output_frac: m.config.rsqrt_output_frac,
            inv_n_q20: m.inv_n_q20,
        };
        let umgerechnet: Vec<(Vec<i8>, Vec<u8>)> = (0..7)
            .map(|n| gewicht_aus_master(&master[n], breiten[n], MASTER_FRAC))
            .collect();
        let gew = Ebenengewichte {
            aufmerksamkeit: Aufmerksamkeitsgewichte {
                q: &umgerechnet[0].0,
                q_skalen: &umgerechnet[0].1,
                k: &umgerechnet[1].0,
                k_skalen: &umgerechnet[1].1,
                v: &umgerechnet[2].0,
                v_skalen: &umgerechnet[2].1,
                o: &umgerechnet[3].0,
                o_skalen: &umgerechnet[3].1,
            },
            gate: &umgerechnet[4].0,
            gate_skalen: &umgerechnet[4].1,
            up: &umgerechnet[5].0,
            up_skalen: &umgerechnet[5].1,
            down: &umgerechnet[6].0,
            down_skalen: &umgerechnet[6].1,
            gamma_ein: &ebene.input_layernorm_gamma.data,
            gamma_ein_skalen: &ebene.input_layernorm_gamma.shifts,
            gamma_mitte: &ebene.post_attention_layernorm_gamma.data,
            gamma_mitte_skalen: &ebene.post_attention_layernorm_gamma.shifts,
        };
        let tab = Ebenentabellen {
            cos: &m.cos_lut,
            sin: &m.sin_lut,
            exp: &m.exp_lut,
            rsqrt: &m.rsqrt_lut,
            silu: &m.silu_lut,
            silu_grad: &grad_lut,
        };
        let spur = vorwaerts_der_ebene(gew, &hidden, vorsp, tab, vg);
        let letzte = v.folge.len() - 1;
        let (logits, g_y) = gradient_vom_ziel(m, &spur.y[letzte], v);
        if s == 0 {
            logits_erst = logits.clone();
        }
        logits_letzt = logits;

        let mut g_aus: Vec<Vec<i32>> =
            (0..v.folge.len()).map(|_| vec![0i32; m.hidden_size]).collect();
        g_aus[letzte] = g_y;
        let gr = gradienten_der_ebene_aus_gradient(gew, &spur, &hidden, &g_aus, tab, vg);

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
        for (mm, gg) in master.iter_mut().zip(gradienten.iter()) {
            let laenge = mm.len() as u64;
            schritt(
                mm,
                gg,
                Schrittkennung { index_versatz: versatz, ..kn },
                v.lr_zaehler,
                v.lr_nenner,
            );
            versatz += laenge;
        }
        if master.iter().any(|mm| ausserhalb_der_form(mm).is_some()) {
            aus_der_form = Some(s);
            break;
        }
    }

    let bewegte_gewichte = anfang
        .iter()
        .zip(master.iter())
        .map(|(a, b)| a.iter().zip(b.iter()).filter(|(x, y)| x != y).count())
        .sum();
    let gewichte_gesamt = master.iter().map(|m| m.len()).sum();
    let matrizen: Vec<&[Master]> = master.iter().map(|v| v.as_slice()).collect();
    // ⚑ **Dieselbe Reihenfolge wie der Abdruck**, sonst beschriebe das
    // Commitment eine andere Aufteilung derselben Zahlen.
    let deltas: Vec<Vec<Master>> = anfang
        .iter()
        .zip(master.iter())
        .map(|(a, b)| delta(a, b))
        .collect();
    let delta_scheiben: Vec<&[Master]> = deltas.iter().map(|v| v.as_slice()).collect();
    Ok(Trainingsergebnis {
        abdruck: trainingsabdruck(&matrizen),
        delta_commitment: trainingsabdruck(&delta_scheiben),
        logits_erst,
        logits_letzt,
        bewegte_gewichte,
        gewichte_gesamt,
        ebene: e,
        experten_beruehrt: None,
        aus_der_form,
    })
}

// ===========================================================================
// Der zweite Weg: ein Expertengemisch
// ===========================================================================

/// Trainiert den **Expertenblock** der letzten Ebene gegen das nächste
/// Token.
///
/// # ⚑ Was hier trainiert wird und was eingefroren bleibt
///
/// **Trainiert:** der Router und jeder Experte, den er wählt.
/// **Eingefroren:** die Aufmerksamkeit derselben Ebene und alles davor.
///
/// ⚑ **Das ist keine Verkürzung, sondern der Zuschnitt der Frage.** Die
/// Aufmerksamkeit ist Zeile für Zeile derselbe Code wie im dichten Lauf,
/// und der belegt sie. Neu und unbelegt ist der Weg durch Mischung und
/// Router, und der läuft hier über echte Gewichte gegen ein echtes Ziel.
///
/// # ⚑ Warum die Master der Experten erst bei Bedarf entstehen
///
/// Qwen3-30B-A3B hat **128 Experten je Ebene**, und ein Master über alle
/// kostet 2,4 GB für eine einzige Ebene. Gebraucht werden sie nicht:
/// **Ein Experte, den der Router nie wählt, hat den Gradienten exakt
/// null.** Nicht „fast null", sondern null, denn er kommt in der Ausgabe
/// gar nicht vor.
///
/// Deshalb entsteht ein Master beim **ersten Mal**, dass sein Experte
/// gewählt wird. Die Menge wächst, während der Router sich bewegt, und
/// sie bleibt eine Teilmenge dessen, was ohnehin gerechnet wurde.
///
/// ⚑ **Und die Reihenfolge der Erzeugung geht nirgends ein.** Der Würfel
/// des Optimierers ist eine reine Funktion aus Ebene, Schritt und Index;
/// der Index eines Experten folgt aus **seiner Nummer**, nicht daraus,
/// wann er zum ersten Mal drankam. Andernfalls hinge das Ergebnis daran,
/// welchen Weg der Router zufällig zuerst nahm.
fn gemischschleife(
    m: &IntegerModel,
    v: &Trainingsvorgaben,
) -> Result<Trainingsergebnis, String> {
    use integer_llm_kernels::backward::router_spreizung;
    use integer_llm_kernels::moe::{mische_experten, route_top_k, Expertenwacht};
    use integer_llm_kernels::trainingsschritt::{
        gradienten_des_gemisches, Expertengewichte, Gemischvorgaben,
    };
    use std::collections::BTreeMap;

    let e = m.num_layers - 1;
    let ebene = &m.layers[e];
    let Feedforward::Moe(moe) = &ebene.ffn else {
        return Err("gemischschleife: die Ebene ist kein Expertengemisch".to_string());
    };
    let sc = &ebene.scales;
    let cfg = &m.config;
    let is = moe.experts[0].gate_proj.shape[0];
    let n = moe.experts.len();

    // Der eingefrorene Teil, einmal gerechnet.
    let mut cache = KVCache::for_range(0, m.num_layers, m.num_kv_heads);
    let mut auf = Zwischenwerte::neu();
    for (pos, tid) in v.folge.iter().enumerate() {
        let start = m.embed_token(*tid);
        let _ = m.run_layers_mit_mitschnitt(start, pos, &mut cache, 0, m.num_layers, &mut auf);
    }
    let letzte = v.folge.len() - 1;
    let mitschnitt = &auf.ebenen()[letzte * m.num_layers + e];
    let x = mitschnitt.norm_mitte.clone();
    let residual = mitschnitt.residual_mitte.clone();

    // ⚑ **Die Akkumulationsskala des Blocks**, kanalweise wie im
    // Vorwärtspass, und für den Rückwärtspass auf ihr Maximum gebracht.
    // Dieselbe Regel wie `Ebenenvorgaben::bloecke`: So ist jede
    // Umrechnung auf den Gradientenbus ein exakter Linksschieber.
    let acc: Vec<u8> = sc
        .residual_mid_frac
        .iter()
        .zip(m.final_residual_frac.iter())
        .map(|(a, b)| *a.min(b))
        .collect();
    let acc_skalar = acc.iter().copied().max().unwrap_or(0);

    let master_von = |t: &QTensor| -> Vec<Master> {
        let in_features = t.shape[1];
        t.data
            .iter()
            .enumerate()
            .map(|(i, w)| i32::from(*w) << (MASTER_FRAC - t.shifts[i / in_features]))
            .collect()
    };
    let mut router = master_von(&moe.router);
    let router_anfang = router.clone();
    // ⚑ **`BTreeMap` und nicht `HashMap`.** Die Ausgabe wird geordnet
    // durchlaufen, und eine Hashtabelle gäbe dabei eine Reihenfolge, die
    // an Adressen hängt. Das ist derselbe Satz wie überall sonst in
    // diesem Projekt, nur an einer neuen Stelle.
    let mut experten: BTreeMap<u16, [Vec<Master>; 3]> = BTreeMap::new();
    let mut experten_anfang: BTreeMap<u16, [Vec<Master>; 3]> = BTreeMap::new();

    let grad_lut = silu_grad_aus_lut(&m.silu_lut);
    let exp_shift = moe.router_frac.saturating_sub(cfg.exp_input_frac);
    let mlp_vorgaben = Mlpvorgaben {
        hidden_size: m.hidden_size,
        intermediate_size: is,
        act_frac: sc.norm_mlp_frac,
        gate_frac: sc.gate_frac,
        up_frac: sc.up_frac,
        down_in_frac: sc.down_in_frac,
        aus_frac: acc_skalar,
        master_frac: MASTER_FRAC,
        silu_in_frac: cfg.silu_in_frac,
        silu_lut_offset: cfg.silu_lut_offset,
        silu_out_frac: cfg.silu_out_frac,
        lr_zaehler: 1,
        lr_nenner: v.lr_nenner,
        kennung: Schrittkennung { ebene: e as u32, schritt: 0, index_versatz: 0 },
    };
    let gv = Gemischvorgaben {
        anzahl_experten: n,
        gewicht_frac: cfg.prob_frac_bits,
        logit_zusatz_bits: 6,
    };

    let mut logits_erst = Vec::new();
    let mut logits_letzt = Vec::new();
    // ⚑ **Der Lastausgleich, und er hatte bis heute keinen Aufrufer.**
    // `Expertenwacht` steht seit dem 2026-08-28 in `kernels::moe`; ein
    // Kern ohne Aufrufer ist in diesem Projekt viermal falsch gewesen,
    // ohne dass es jemandem auffiel.
    let mut wacht = Expertenwacht::neu(n, v.lastausgleich.geduld);
    let mut je_experte_gewaehlt = vec![0u32; n];

    let mut aus_der_form: Option<u64> = None;
    for s in 0..v.schritte {
        let mut vg = mlp_vorgaben;
        vg.kennung.schritt = s;

        let (rw, rs) =
            gewicht_aus_master(&router, m.hidden_size, MASTER_FRAC);
        let roh: Vec<i32> = integer_llm_kernels::linear::linear_w8a16(
            &x, &rw, m.hidden_size, &rs, sc.norm_mlp_frac, moe.router_frac,
        )
        .iter()
        .map(|l| i32::from(*l))
        .collect();

        // ⚑ **Der Schub der Wacht kommt VOR der Auswahl**, denn er soll
        // genau die Auswahl ändern: Ein Experte, der nie gewählt wird,
        // bekommt nie einen Gradienten. Seine Summe ist exakt null, der
        // Logit-Mittelwert bleibt also, wo er war.
        let schub = wacht.schub(v.lastausgleich.wacht_staerke);
        let logits: Vec<i32> = roh
            .iter()
            .zip(schub.iter())
            .map(|(l, d)| l.saturating_add(*d))
            .collect();

        let routing = route_top_k(
            &logits, moe.top_k, &m.exp_lut, exp_shift, cfg.prob_frac_bits,
            moe.norm_topk_prob,
        );
        wacht.segment(&routing.experten);
        for i in &routing.experten {
            je_experte_gewaehlt[*i as usize] += 1;
        }

        // Neu gewählte Experten bekommen jetzt ihren Master.
        for i in &routing.experten {
            experten.entry(*i).or_insert_with(|| {
                let ex = &moe.experts[*i as usize];
                let drei = [
                    master_von(&ex.gate_proj),
                    master_von(&ex.up_proj),
                    master_von(&ex.down_proj),
                ];
                experten_anfang.insert(*i, drei.clone());
                drei
            });
        }

        // Vorwärts durch die gewählten Experten.
        let mut spuren: Vec<Mlpspur> = Vec::with_capacity(routing.experten.len());
        let mut ausgaben: Vec<Vec<i16>> = Vec::with_capacity(routing.experten.len());
        let mut halde: Vec<Uebertragungsform> = Vec::new();
        for i in &routing.experten {
            let mm = &experten[i];
            let (gw, gs) = gewicht_aus_master(&mm[0], m.hidden_size, MASTER_FRAC);
            let (uw, us) = gewicht_aus_master(&mm[1], m.hidden_size, MASTER_FRAC);
            let (dw, ds) = gewicht_aus_master(&mm[2], is, MASTER_FRAC);
            let mut sp = Mlpspur::default();
            let aus = integer_llm_kernels::mlp::mlp_int_mit_spur(
                &x, &gw, &uw, &dw, m.hidden_size, is, &gs, &us, &ds, &m.silu_lut,
                sc.norm_mlp_frac, sc.gate_frac, sc.up_frac, sc.down_in_frac,
                cfg.silu_in_frac, cfg.silu_lut_offset, cfg.silu_out_frac,
                &vec![acc_skalar; m.hidden_size], Some(&mut sp),
            );
            ausgaben.push(aus);
            spuren.push(sp);
            halde.push((gw, gs, uw, us, dw, ds));
        }
        let block = mische_experten(&ausgaben, &routing.gewichte, cfg.prob_frac_bits);

        // Die Residualaddition, wie `forward_layer` sie rechnet: beide
        // Operanden auf der groeberen Skala, EINE Summe, EINE Klemmung.
        let y: Vec<i16> = (0..m.hidden_size)
            .map(|i| {
                let r = rescale_i64_hilf(
                    i64::from(residual[i]),
                    sc.residual_mid_frac[i],
                    acc_skalar,
                );
                let summe = r + i64::from(block[i]);
                klemme_i16(rescale_i64_hilf(summe, acc_skalar, m.final_residual_frac[i]))
            })
            .collect();

        let (logits_kopf, g_y) = gradient_vom_ziel(m, &y, v);
        if s == 0 {
            logits_erst = logits_kopf.clone();
        }
        logits_letzt = logits_kopf;

        // ⚑ **Durch die Residualaddition ist der Gradient die
        // Identitaet**, und nur die Darstellung wechselt: `g_y` liegt auf
        // `final_residual_frac`, der Block auf `acc_skalar`.
        let g_block: Vec<i32> = (0..m.hidden_size)
            .map(|i| {
                klemme_i32(rescale_i64_hilf(
                    i64::from(g_y[i]),
                    m.final_residual_frac[i],
                    acc_skalar,
                ))
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
        let mut gr = gradienten_des_gemisches(
            &g_block, &x, &routing.experten, &routing.gewichte, &ausgaben, &spuren, &ew,
            &rw, &rs, &m.silu_lut, &grad_lut, vg, gv,
        );

        // ⚑ **Die Spreizungsstrafe, und sie liest die Logit-ABSTÄNDE**,
        // nicht die quantisierten Gewichte. Deshalb hat sie ihren
        // grössten Wert genau dort, wo der Softmax-Gradient
        // verschwunden ist. Der Boden aus θ_v 0.18.0 hält den Gradienten
        // von der exakten Null fern; gross macht ihn erst sie.
        if v.lastausgleich.spreizung_schwelle > 0 {
            let strafe = router_spreizung(
                &logits,
                &routing.experten,
                v.lastausgleich.spreizung_schwelle,
                v.lastausgleich.spreizung_daempfung,
            );
            // Der Weg vom Logitgradienten in die Routergewichte, wie
            // oben: `dL/dW = gᵀ·x`, dieselbe Skala wie der übrige
            // Logitgradient.
            let (_gx, gw) = integer_llm_kernels::backward::linear_backward(
                &strafe, &x, &rw, m.hidden_size, &rs,
                acc_skalar + gv.logit_zusatz_bits, sc.norm_mlp_frac,
            );
            for (ziel, teil) in gr.router.iter_mut().zip(gw.iter()) {
                *ziel = ziel.saturating_add(
                    (*teil).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                );
            }
        }

        // ⚑ **Der Indexversatz folgt der Expertennummer**, nicht der
        // Reihenfolge des Erstkontakts.
        let je_experte = (m.hidden_size * is * 3) as u64;
        for (i, mg) in &gr.experten {
            let mm = experten.get_mut(i).expect("gerade erzeugt");
            let basis = u64::from(*i) * je_experte;
            let teile = [&mg.gate, &mg.up, &mg.down];
            for (nr, teil) in teile.iter().enumerate() {
                let k = Schrittkennung {
                    index_versatz: basis + (nr as u64) * (m.hidden_size * is) as u64,
                    ..vg.kennung
                };
                schritt(&mut mm[nr], teil, k, 1, v.lr_nenner);
            }
        }
        let k = Schrittkennung {
            index_versatz: u64::from(n as u16) * je_experte,
            ..vg.kennung
        };
        schritt(&mut router, &gr.router, k, 1, v.lr_nenner);

        if ausserhalb_der_form(&router).is_some()
            || experten.values().any(|mm| mm.iter().any(|t| ausserhalb_der_form(t).is_some()))
        {
            aus_der_form = Some(s);
            break;
        }
    }

    // Der Abdruck: der Router, dann die berührten Experten **nach ihrer
    // Nummer geordnet**, und die Nummernliste selbst.
    let nummern: Vec<Master> = experten.keys().map(|i| i32::from(*i)).collect();
    let mut matrizen: Vec<&[Master]> = vec![&nummern, &router];
    for mm in experten.values() {
        matrizen.push(&mm[0]);
        matrizen.push(&mm[1]);
        matrizen.push(&mm[2]);
    }
    let bewegte = router_anfang
        .iter()
        .zip(router.iter())
        .filter(|(a, b)| a != b)
        .count()
        + experten
            .iter()
            .map(|(i, jetzt)| {
                let vorher = &experten_anfang[i];
                (0..3)
                    .map(|nr| {
                        vorher[nr]
                            .iter()
                            .zip(jetzt[nr].iter())
                            .filter(|(a, b)| a != b)
                            .count()
                    })
                    .sum::<usize>()
            })
            .sum::<usize>();
    let gesamt = router.len() + experten.values().map(|mm| mm.iter().map(|v| v.len()).sum::<usize>()).sum::<usize>();

    // Das Δm, in derselben Ordnung wie der Abdruck.
    let router_delta = delta(&router_anfang, &router);
    let experten_delta: Vec<[Vec<Master>; 3]> = experten
        .iter()
        .map(|(i, jetzt)| {
            let vorher = &experten_anfang[i];
            [
                delta(&vorher[0], &jetzt[0]),
                delta(&vorher[1], &jetzt[1]),
                delta(&vorher[2], &jetzt[2]),
            ]
        })
        .collect();
    let mut delta_scheiben: Vec<&[Master]> = vec![&nummern, &router_delta];
    for dd in &experten_delta {
        delta_scheiben.push(&dd[0]);
        delta_scheiben.push(&dd[1]);
        delta_scheiben.push(&dd[2]);
    }

    Ok(Trainingsergebnis {
        abdruck: trainingsabdruck(&matrizen),
        delta_commitment: trainingsabdruck(&delta_scheiben),
        logits_erst,
        logits_letzt,
        bewegte_gewichte: bewegte,
        gewichte_gesamt: gesamt,
        ebene: e,
        aus_der_form,
        experten_beruehrt: Some((
            je_experte_gewaehlt.iter().filter(|c| **c > 0).count(),
            n,
        )),
    })
}

/// Die drei Matrizen eines Experten in Übertragungsform, je Werte und
/// Zeilenskalen.
type Uebertragungsform = (Vec<i8>, Vec<u8>, Vec<i8>, Vec<u8>, Vec<i8>, Vec<u8>);

/// Umskalieren mit kaufmännischer Rundung nach rechts, exakt nach links.
fn rescale_i64_hilf(v: i64, ein: u8, aus: u8) -> i64 {
    let s = i32::from(ein) - i32::from(aus);
    if s >= 0 {
        integer_llm_kernels::fixed_point::rshift_round_i64(v, s as u8)
    } else {
        v << (-s) as u32
    }
}

fn klemme_i16(v: i64) -> i16 {
    v.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
}

fn klemme_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod kopfsammlung {
    use super::*;

    /// ⚑ **Das Vorzeichen ist die halbe Richtigkeit dieser Klasse.**
    ///
    /// Der Sammler haelt den **negativen** Gradienten, wie
    /// `sammle_roh`, denn `schritt_normiert` **addiert**. Fuer das
    /// Zieltoken ist `g = p − 1 < 0`, also wandert seine Kopfzeile
    /// **auf den verborgenen Zustand zu**, und genau das soll ein
    /// Lernschritt tun. Ein Vorzeichenfehler hier liefe stumm und
    /// verschoebe das Ziel weg statt hin.
    #[test]
    fn die_zielzeile_wandert_auf_den_zustand_zu() {
        let mut k = Kopfsammlung::neu(&[7], 4);
        let mut g = vec![0i32; 16];
        g[7] = -100; // wie beim Ziel: p − 1
        k.aufnehmen(&g, &[2, 0, -3, 1]);
        let s = k.summe_mut(7).expect("Zeile 7");
        assert_eq!(s, &[200, 0, -300, 100], "die Zielzeile wandert nicht auf den Zustand zu");
    }

    /// Ein Mitbewerber hat `g = p > 0` und wird **weggeschoben**.
    #[test]
    fn eine_mitbewerberzeile_wandert_weg() {
        let mut k = Kopfsammlung::neu(&[3], 2);
        let mut g = vec![0i32; 8];
        g[3] = 5;
        k.aufnehmen(&g, &[10, -4]);
        assert_eq!(k.summe_mut(3).expect("Zeile 3"), &[-50, 20]);
    }

    /// ⚑ **Was nicht verfolgt wird, bleibt unberuehrt**, und das ist
    /// die Voraussetzung dafuer, ueberhaupt nur einen Teil des
    /// Vokabulars zu fuehren.
    #[test]
    fn unverfolgte_zeilen_bleiben_draussen() {
        let mut k = Kopfsammlung::neu(&[1, 2], 3);
        assert_eq!(k.zeilen(), 2);
        assert!(k.summe_mut(9).is_none(), "eine unverfolgte Zeile hat eine Summe");
        assert_eq!(k.token(), vec![1, 2]);
    }

    #[test]
    fn doppelte_token_geben_eine_zeile() {
        let k = Kopfsammlung::neu(&[5, 5, 5], 2);
        assert_eq!(k.zeilen(), 1);
    }

    /// Mehrere Positionen summieren sich, und `leeren` setzt zurueck.
    #[test]
    fn positionen_summieren_sich_und_leeren_setzt_zurueck() {
        let mut k = Kopfsammlung::neu(&[0], 2);
        let g = vec![-1i32, 0];
        k.aufnehmen(&g, &[3, 5]);
        k.aufnehmen(&g, &[3, 5]);
        assert_eq!(k.positionen(), 2);
        assert_eq!(k.summe_mut(0).expect("Zeile 0"), &[6, 10]);
        k.leeren();
        assert_eq!(k.positionen(), 0);
        assert_eq!(k.summe_mut(0).expect("Zeile 0"), &[0, 0]);
    }

    /// ⚑ Ein Gradient von null kostet keine Arbeit und aendert nichts;
    /// bei einem Vokabular von 151 936 ist das der Normalfall.
    #[test]
    fn ein_nullgradient_bewegt_nichts() {
        let mut k = Kopfsammlung::neu(&[4], 3);
        k.aufnehmen(&[0i32; 8], &[7, 7, 7]);
        assert_eq!(k.summe_mut(4).expect("Zeile 4"), &[0, 0, 0]);
    }

    /// Ein Token jenseits der Logitliste wird uebersprungen statt zu
    /// stuerzen: Der Korpus kann Token nennen, die der Kopf nicht hat.
    #[test]
    fn ein_token_ausserhalb_der_logits_stuerzt_nicht() {
        let mut k = Kopfsammlung::neu(&[99], 2);
        k.aufnehmen(&[1, 2, 3], &[1, 1]);
        assert_eq!(k.summe_mut(99).expect("Zeile 99"), &[0, 0]);
    }
}
