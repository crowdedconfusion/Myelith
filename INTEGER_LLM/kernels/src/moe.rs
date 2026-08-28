//! Mixture-of-Experts: Router und Mischung, vollständig ganzzahlig.
//!
//! ## Warum dieses Modul überhaupt heikel ist
//!
//! Whitepaper Kap. 10.1 schließt MoE heute aus, weil „der Datenpfad je
//! Token variiert". Der Einwand trifft die **expertenparallele**
//! Verteilung, bei der Experten auf verschiedenen Pod-Mitgliedern liegen.
//! Hält dagegen jeder Knoten alle Experten seiner Layer, bleibt die
//! Pod-Kette unverändert, und bei Top-k-Routing feuern je Layer und Token
//! **exakt k Experten**, also eine Konstante aus der Modellkonfiguration.
//!
//! Damit verlagert sich das Risiko von der Architektur in **diese Datei**.
//! Der Router trifft je Token und Layer eine diskrete Auswahl. Weicht sie
//! zwischen zwei ehrlichen Knoten auch nur einmal ab, divergiert alles
//! Folgende, und der Redundanzvergleich meldet beide als fehlerhaft. Eine
//! Abweichung um ein Bit im letzten Kanal ist ein Qualitätsproblem; eine
//! Abweichung in der Expertenauswahl ist ein Fork.
//!
//! ## Die drei Festlegungen, an denen das hängt
//!
//! **1. Ausgewählt wird über die Logits, nicht über die
//! Wahrscheinlichkeiten.** Softmax ist streng monoton, die Auswahl wäre
//! also dieselbe, aber der Weg über die exp-Tabelle **erzeugt
//! Gleichstände, die es vorher nicht gab**: Die Tabelle bildet einen
//! ganzen Eingangsbereich auf denselben Ausgangswert ab. Wer über die
//! Wahrscheinlichkeiten auswählt, entscheidet also häufiger per
//! Tie-Break, und jeder Tie-Break ist eine Stelle, an der zwei
//! Implementierungen auseinandergehen können. Über die Logits sind es
//! nachweislich weniger, siehe [`randgleichstaende`].
//!
//! **2. Bei Gleichstand gewinnt der kleinere Expertenindex.** Das ist
//! keine Geschmacksfrage, sondern die einzige Regel, die ohne
//! Zusatzinformation auskommt und auf jeder Maschine dasselbe liefert.
//! Eine unstabile Sortierung wäre hier ein Fehler, den keine Messung an
//! einer einzelnen Maschine je zeigt: `sort_unstable_by` darf gleiche
//! Elemente beliebig anordnen, und „beliebig" heißt bibliotheksabhängig.
//! Deshalb sortiert dieses Modul gar nicht, sondern wählt k-mal das
//! Maximum über einen **vollständigen** Vergleich `(Logit absteigend,
//! Index aufsteigend)`.
//!
//! **3. Die Mischgewichte kommen aus [`softmax_int`], nicht aus einer
//! eigenen Rechnung.** Bei `normieren = true` (in den Referenzmodellen
//! `norm_topk_prob`) ist der Softmax über alle Experten mit
//! anschließender Neunormierung auf die k gewählten **exakt gleich** dem
//! Softmax über nur die k gewählten Logits:
//!
//! ```text
//!     p_j / Σ_{i∈K} p_i  =  (e^{l_j}/Z) / (Σ_{i∈K} e^{l_i}/Z)
//!                        =   e^{l_j} / Σ_{i∈K} e^{l_i}
//! ```
//!
//! Der Bruch kürzt sich, `Z` fällt heraus. Das spart nicht nur 120 von
//! 128 Tabellenzugriffen, es vermeidet auch die Summe über alle Experten
//! und damit eine Überlaufstelle. Vor allem aber: **eine Rundungsregel im
//! Projekt statt zwei.**
//!
//! **Die Gleichung gilt in den reellen Zahlen.** Ganzzahlig sind die
//! beiden Wege *nicht* bitgleich: Der Umweg über alle Experten rundet
//! zweimal, einmal im Softmax und einmal bei der Neunormierung.
//! Genommen wird deshalb der Weg mit **einer** Rundung, nicht der mit
//! zwei. Das ist keine Näherung, die man in Kauf nimmt, sondern die
//! genauere von zwei Rechnungen.
//!
//! ## Was dieses Modul ausdrücklich nicht kann, und warum das Absicht ist
//!
//! **Kein Token-Dropping bei Expertenüberlauf.** Viele
//! MoE-Implementierungen verwerfen Token, wenn ein Experte im Batch eine
//! Kapazitätsgrenze überschreitet. Dann hängt das Ergebnis an Position
//! *i* davon ab, **welche anderen Token im selben Batch lagen**. Am
//! 2026-08-23 wurde festgelegt, dass ein Segment eine Position ist; zwei
//! redundante Pods bilden verschiedene Batches, und der
//! Redundanzvergleich meldete zwei ehrliche Pods als abweichend. Es gibt
//! hier deshalb keinen Kapazitätsparameter, den jemand setzen könnte.
//! Dieselbe Klasse wie Fund 39: eine Achse, die zwei Seiten verschieden
//! lesen.

use crate::fixed_point::clamp_i16_from_i64;
use crate::softmax::softmax_int;

/// Das Ergebnis einer Routing-Entscheidung für **eine** Position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routing {
    /// Die gewählten Expertenindizes, in Auswahlreihenfolge (bestes
    /// zuerst). Diese Reihenfolge ist Teil der Festlegung: Sie geht in
    /// die Berechnungsspur ein, und zwei Knoten müssen dieselbe liefern.
    pub experten: Vec<u16>,
    /// Die Mischgewichte, in der Skala `1 << gewicht_frac_bits`,
    /// stellungsgleich zu [`Routing::experten`].
    pub gewichte: Vec<i32>,
}

impl Routing {
    /// Die Summe der Mischgewichte. Bei `normieren = true` ist sie
    /// **exakt** `1 << gewicht_frac_bits`, siehe [`route_top_k`].
    pub fn gewichtssumme(&self) -> i64 {
        self.gewichte.iter().map(|g| *g as i64).sum()
    }
}

/// Wählt die k besten Experten und berechnet ihre Mischgewichte.
///
/// **Parameter:**
/// - `router_logits`: ein Wert je Experte, Ausgabe der Router-Projektion
/// - `k`: wie viele Experten feuern. Wird an die Expertenzahl
///   angeschnitten, falls größer
/// - `exp_lut`, `lut_shift`: die exp-Tabelle wie in [`softmax_int`]
/// - `gewicht_frac_bits`: Skala der Ausgabegewichte
/// - `normieren`: entspricht `norm_topk_prob` der Referenzmodelle. Bei
///   `true` summieren sich die Gewichte **exakt** auf
///   `1 << gewicht_frac_bits`
///
/// **Zur exakten Summe bei `normieren = true`:** `softmax_int` rundet je
/// Eintrag kaufmännisch zur geraden Zahl; die Summe der gerundeten Werte
/// verfehlt die Eins deshalb um wenige Einheiten der letzten Stelle. Das
/// ist je Layer belanglos und **über achtundvierzig Layer nicht mehr**,
/// denn der Fehler ist nicht zufällig, sondern hängt an der
/// Werteverteilung und wiederholt sich. Der Rest wird deshalb dem
/// **größten** Gewicht zugeschlagen (bei Gleichstand dem kleineren
/// Index): Dort ist die relative Verzerrung am kleinsten, und die Regel
/// braucht keine zweite Rundung.
pub fn route_top_k(
    router_logits: &[i32],
    k: usize,
    exp_lut: &[i16],
    lut_shift: u8,
    gewicht_frac_bits: u8,
    normieren: bool,
) -> Routing {
    let n = router_logits.len();
    let k = k.min(n);
    if k == 0 {
        return Routing { experten: Vec::new(), gewichte: Vec::new() };
    }

    let experten = waehle_top_k(router_logits, k);

    let gewichte = if normieren {
        // Softmax über nur die Gewählten: mathematisch identisch zur
        // Neunormierung des vollen Softmax, siehe Modulkopf.
        let gewaehlte: Vec<i32> =
            experten.iter().map(|e| router_logits[*e as usize]).collect();
        let mut w = softmax_int(&gewaehlte, exp_lut, lut_shift, gewicht_frac_bits);
        korrigiere_summe(&mut w, gewicht_frac_bits);
        w
    } else {
        let alle = softmax_int(router_logits, exp_lut, lut_shift, gewicht_frac_bits);
        experten.iter().map(|e| alle[*e as usize]).collect()
    };

    Routing { experten, gewichte }
}

/// Die k besten Indizes nach `(Logit absteigend, Index aufsteigend)`.
///
/// **Bewusst keine Sortierung.** Ein `sort_unstable_by` über einen
/// Vergleich, der nur den Logit ansieht, ordnet gleiche Elemente
/// beliebig an; „beliebig" heißt hier bibliotheks- und
/// längenabhängig, und auf einer einzelnen Maschine fällt das nie auf.
/// Diese Schleife wählt k-mal das Maximum über einen vollständigen
/// Vergleich und ist damit von der Sortierimplementierung unabhängig.
///
/// Bei k ≪ n ist sie außerdem billiger: 8 Durchläufe über 128 Einträge
/// statt einer Sortierung von 128.
fn waehle_top_k(logits: &[i32], k: usize) -> Vec<u16> {
    let mut vergeben = vec![false; logits.len()];
    let mut gewaehlt: Vec<u16> = Vec::with_capacity(k);
    for _ in 0..k {
        let mut bester: Option<usize> = None;
        for (i, l) in logits.iter().enumerate() {
            if vergeben[i] {
                continue;
            }
            match bester {
                None => bester = Some(i),
                // Streng größer: bei Gleichstand bleibt der zuerst
                // gesehene, und das ist der kleinere Index.
                Some(b) if *l > logits[b] => bester = Some(i),
                _ => {}
            }
        }
        match bester {
            Some(i) => {
                vergeben[i] = true;
                gewaehlt.push(i as u16);
            }
            None => break,
        }
    }
    gewaehlt
}

/// Schlägt den Rundungsrest dem größten Gewicht zu, damit die Summe
/// exakt `1 << frac_bits` ergibt.
fn korrigiere_summe(gewichte: &mut [i32], frac_bits: u8) {
    if gewichte.is_empty() {
        return;
    }
    let soll = 1i64 << frac_bits;
    let ist: i64 = gewichte.iter().map(|g| *g as i64).sum();
    let rest = soll - ist;
    if rest == 0 {
        return;
    }
    let mut groesster = 0usize;
    for (i, g) in gewichte.iter().enumerate() {
        if *g > gewichte[groesster] {
            groesster = i;
        }
    }
    gewichte[groesster] = (gewichte[groesster] as i64 + rest) as i32;
}

/// Zählt die Gleichstände, die die **Auswahl tatsächlich verändern**.
///
/// Für die Machbarkeitsmessung (INTEGER_LLM Phase 12.81c) ist nicht
/// interessant, wie viele Logits zufällig gleich sind, sondern wie oft
/// ein Gleichstand **an der Auswahlgrenze** liegt. Ein Gleichstand
/// zwischen Rang 1 und 2 ändert nichts, beide feuern. Ein Gleichstand
/// zwischen Rang k und k+1 entscheidet, welcher von beiden feuert, und
/// **nur dort** trägt der Tie-Break die Last.
///
/// Rückgabe: die Zahl der Experten, die denselben Logit tragen wie der
/// zuletzt gewählte, aber nicht mehr hineinpassen. Null heißt: Die
/// Auswahl war eindeutig, ganz ohne Tie-Break.
pub fn randgleichstaende(router_logits: &[i32], k: usize) -> usize {
    let n = router_logits.len();
    let k = k.min(n);
    if k == 0 || k == n {
        return 0;
    }
    let gewaehlt = waehle_top_k(router_logits, k);
    let schwelle = router_logits[*gewaehlt.last().unwrap() as usize];
    let mut ist_gewaehlt = vec![false; n];
    for e in &gewaehlt {
        ist_gewaehlt[*e as usize] = true;
    }
    router_logits
        .iter()
        .enumerate()
        .filter(|(i, l)| **l == schwelle && !ist_gewaehlt[*i])
        .count()
}

/// Mischt die Ausgaben der gefeuerten Experten zu einer Aktivierung.
///
/// **Voraussetzung, die der Aufrufer einhalten muss:** Alle Ausgaben
/// stammen aus derselben Layer und tragen deshalb dieselbe Ausgangsskala
/// je Kanal. Eine Umskalierung findet hier nicht statt, und sie wäre auch
/// falsch: Die Experten einer Layer schreiben alle in denselben
/// Residualstrom, ihre Skala ist eine Eigenschaft dieses Stroms und nicht
/// des einzelnen Experten.
///
/// Akkumuliert in i64. Bei k = 8, Ausgaben bis 2^15 und Gewichten bis
/// 2^15 liegt das Zwischenergebnis bei 2^33 und spränge aus i32.
pub fn mische_experten(
    ausgaben: &[Vec<i16>],
    gewichte: &[i32],
    gewicht_frac_bits: u8,
) -> Vec<i16> {
    debug_assert_eq!(
        ausgaben.len(),
        gewichte.len(),
        "je Ausgabe genau ein Gewicht"
    );
    let breite = ausgaben.first().map(|a| a.len()).unwrap_or(0);
    let mut acc = vec![0i64; breite];
    for (ausgabe, gewicht) in ausgaben.iter().zip(gewichte.iter()) {
        debug_assert_eq!(ausgabe.len(), breite, "alle Ausgaben gleich breit");
        let g = *gewicht as i64;
        for (a, x) in acc.iter_mut().zip(ausgabe.iter()) {
            *a += (*x as i64) * g;
        }
    }
    acc.into_iter()
        .map(|a| clamp_i16_from_i64(crate::fixed_point::rshift_round_i64(a, gewicht_frac_bits)))
        .collect()
}

/// Der Trainingszustand, der einen hungernden Experten am Leben hält.
///
/// ## ⚑ Der zweite absorbierende Zustand
///
/// [`saettigungsabstand`] beschreibt den ersten: Ein **gewählter**
/// Experte, dessen Gewicht auf null rundet, bekommt keinen Gradienten
/// mehr. Dagegen hilft die Spreizungsstrafe.
///
/// **Der zweite ist stiller.** Ein Experte, dessen Logit so weit unter
/// den übrigen liegt, dass er nie in die Top-k kommt, wird nie
/// gerechnet, bekommt nie einen Gradienten und ändert sich nie. Er ist
/// tot, ohne dass irgendeine Zahl davon abweicht. **Bei 128 Experten und
/// Top-8 ist das kein Randfall**, sondern der Normalzustand für 120 von
/// ihnen je Token; tot ist er erst, wenn es über **viele** Token so
/// bleibt.
///
/// ## Warum das hier ohne Batch-Statistik geht
///
/// Der übliche Lastausgleich mittelt über den Batch, und genau das ist
/// hier verboten: Das Ergebnis an Position *i* hinge davon ab, welche
/// anderen Token zufällig danebenlagen. **Diese Wacht mittelt nicht über
/// den Batch, sondern zählt über die Segmentfolge**, und das ist ein
/// Unterschied ums Ganze:
///
/// - Die Batch-Zusammensetzung wählt der Miner. Sie ist willkürlich,
///   und zwei ehrliche Miner können verschieden batchen.
/// - Die **Segmentfolge** legt das Protokoll fest. Zwei redundante
///   Miner sehen dieselbe, in derselben Reihenfolge.
///
/// Der Zähler ist damit so deterministisch wie die Gewichte selbst und
/// gehört wie sie in den Trainingszustand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expertenwacht {
    /// Segmente seit der letzten Wahl, je Experte.
    seit: Vec<u32>,
    /// Ab wie vielen Segmenten ohne Wahl geschoben wird.
    geduld: u32,
}

impl Expertenwacht {
    /// Neu, mit allen Zählern auf null.
    pub fn neu(anzahl_experten: usize, geduld: u32) -> Self {
        Self {
            seit: vec![0; anzahl_experten],
            geduld,
        }
    }

    /// Die Zähler je Experte, für Protokoll und Prüfung.
    pub fn seit(&self) -> &[u32] {
        &self.seit
    }

    /// Ein Segment ist gerechnet: Die gewählten fangen von vorn an, die
    /// übrigen zählen weiter.
    pub fn segment(&mut self, experten: &[u16]) {
        for s in self.seit.iter_mut() {
            *s = s.saturating_add(1);
        }
        for e in experten.iter() {
            let i = *e as usize;
            assert!(i < self.seit.len(), "Expertenwacht: Index ausserhalb");
            self.seit[i] = 0;
        }
    }

    /// Wie viele Experten gerade hungern.
    pub fn hungernde(&self) -> usize {
        self.seit.iter().filter(|s| **s > self.geduld).count()
    }

    /// Der Schub auf die Router-Logits, in Logit-Einheiten.
    ///
    /// Hungernde Experten bekommen `staerke`, und die Gegenbuchung
    /// verteilt sich auf die **übrigen**, nach der Hausregel: abrunden,
    /// den Rest an den zuletzt gewählten. Damit ist die Summe **exakt
    /// null** und der Logit-Mittelwert bleibt, wo er war.
    ///
    /// ⚑ **Hungern alle oder keiner, ist der Schub überall null.** Im
    /// ersten Fall gäbe es niemanden zum Gegenbuchen, im zweiten nichts
    /// zu tun. Beides ist richtig und beides hat einen Test.
    pub fn schub(&self, staerke: i32) -> Vec<i32> {
        let n = self.seit.len();
        let mut d = vec![0i32; n];
        if staerke <= 0 {
            return d;
        }
        let hungernd: Vec<usize> = (0..n).filter(|i| self.seit[*i] > self.geduld).collect();
        if hungernd.is_empty() || hungernd.len() == n {
            return d;
        }
        let satt: Vec<usize> = (0..n).filter(|i| self.seit[*i] <= self.geduld).collect();

        let gesamt = (hungernd.len() as i64) * (staerke as i64);
        for i in hungernd.iter() {
            d[*i] = staerke;
        }
        // Abrunden je Sattem, der Rest an den zuletzt gewählten. Bei
        // Gleichstand der kleinste Index, damit es deterministisch ist.
        let je = gesamt / (satt.len() as i64);
        let mut rest = gesamt - je * (satt.len() as i64);
        let empfaenger = *satt
            .iter()
            .min_by_key(|i| (self.seit[**i], **i))
            .expect("nicht leer");
        for i in satt.iter() {
            let mut ab = je;
            if *i == empfaenger {
                ab += rest;
                rest = 0;
            }
            d[*i] = -(ab as i32);
        }
        d
    }
}

/// Hängt einen Experten ein, ohne die Ausgabe zu ändern.
///
/// ## ⚑ Warum das nicht dasselbe ist wie Breitenwachstum
///
/// Der Wachstumsoperator für dichte Schichten teilt eine Zeile in `a`
/// und `b` mit `a + b = m` und bricht die Symmetrie über das letzte Bit.
/// **Beim Routing trägt das nicht**: Dort ist die Ausgabe eine Summe
/// über die **ausgewählten** Experten, nicht über alle, und zwei Kopien
/// mit gleichem Logit verdrängen einen dritten aus der Top-k.
///
/// **Der einzige exakt funktionserhaltende Weg ist der hier**: Der neue
/// Experte bekommt ein Logit unter allen anderen und wird deshalb nie
/// gewählt. Die Ausgabe ändert sich um **exakt nichts**.
///
/// ⚑ **Und genau deshalb war er bis zur [`Expertenwacht`] wertlos:** Wer
/// nie gewählt wird, bekommt nie einen Gradienten und bleibt für immer
/// eine tote Kopie. Erst der Hungerzähler holt ihn zurück, und damit ist
/// aus einer unlösbaren Aufgabe eine gelöste geworden. Der Test
/// `ein_neuer_experte_wird_von_der_wacht_zurueckgeholt` fährt das durch.
///
/// **Gibt den Index des neuen Experten zurück.** Die Gewichte des
/// Vorbilds kopiert der Aufrufer; diese Funktion kennt nur die Logits.
pub fn experte_einhaengen(router_logits: &mut Vec<i32>) -> usize {
    let kleinstes = router_logits.iter().copied().min().unwrap_or(0);
    router_logits.push(kleinstes.saturating_sub(1));
    router_logits.len() - 1
}

/// `ln 2` in Q8, also `round(0,6931 · 256)`.
///
/// Eingefroren, weil daraus eine Schranke des Konsensvertrags folgt und
/// eine zur Laufzeit gerechnete Konstante plattformabhängig wäre.
pub const LN2_Q8: i64 = 177;

/// Ab welchem Logit-Abstand der Ganzzahl-Softmax sättigt, in
/// Eingangseinheiten der exp-Tabelle.
///
/// ## ⚑ Warum es diese Funktion gibt (Fund 79)
///
/// `softmax_int` gibt Gewichte auf `1 << prob_frac_bits` aus. Das
/// kleinste darstellbare Gewicht ungleich null ist `1`; alles unter der
/// halben Stufe rundet auf **null**. Zwei Logits mit genügend Abstand
/// ergeben deshalb nicht „fast alles" und „fast nichts", sondern
/// `(1 << frac, 0)`.
///
/// **Und an dieser Stelle steht der Router still**, denn dann ist der
/// Gradient jedes Logits exakt null: für den Verlierer, weil `p_i = 0`
/// ist, und für den Gewinner, weil `p_0 = 2^frac` die Klammer
/// `g_0 − Σ_j g_j p_j / 2^frac` zu null macht.
///
/// **Die Schranke, hergeleitet statt geraten:** Sättigung tritt ein, wenn
/// `p_min < 2^-(frac+1)`, also ab einem Abstand von
/// `(frac + 1) · ln 2` nats. In Eingangseinheiten der Tabelle mal
/// `2^exp_input_frac_bits`.
///
/// | Aufbau | Abstand |
/// |---|---|
/// | `prob_frac_bits = 8` | 6,24 nats |
/// | `prob_frac_bits = 14` (θ_v 0.16.0) | 10,40 nats |
/// | `f32` zum Vergleich | 104 nats |
/// | `f64` zum Vergleich | 745 nats |
///
/// ⚑ **Der Ganzzahlpfad kollabiert damit rund zehnmal früher als `f32`
/// und siebzigmal früher als `f64`.** Router-Kollaps ist ein bekanntes
/// Problem von Expertengemischen und **kein Erzeugnis dieses Projekts**;
/// die Ganzzahltabelle macht ihn nur um Größenordnungen leichter
/// erreichbar. Wer diese Zahl liest, soll den Unterschied sehen und
/// nicht den Eindruck bekommen, Gleitkomma sei immun.
///
/// **Dieselbe Mechanik hat das Projekt schon einmal getroffen**, in der
/// Attention: Fund 29 hob `prob_frac_bits` von 8 auf 14, weil jede
/// Position unter `1/512` einzeln auf null rundete und die
/// Aufmerksamkeit auf die Spitzenposition kollabierte. Der Router hat
/// dieselbe Krankheit an anderer Stelle.
pub fn saettigungsabstand(prob_frac_bits: u8, exp_input_frac_bits: u8) -> i32 {
    // (frac + 1) · ln2 · 2^exp_input_frac, alles in Q8 gerechnet und
    // am Ende einmal geschoben.
    let nats_q8 = (prob_frac_bits as i64 + 1) * LN2_Q8;
    let skaliert = nats_q8 << exp_input_frac_bits;
    (skaliert >> 8) as i32
}

/// Eine kleine exp-Tabelle für Tests, geteilt mit `backward.rs`.
///
/// Sie liegt hier und nicht dort, weil sie zum Routing gehört: Wer die
/// Tabelle ändert, ändert die Mischgewichte, und dann sollen die Tests
/// beider Module gemeinsam anschlagen.
#[cfg(test)]
pub(crate) fn tests_exp_lut() -> Vec<i16> {
    tests::EXP_LUT.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// exp(-i/16) · 256, ganzzahlig vorberechnet. Bewusst als Konstante
    /// und nicht per `f64` erzeugt: Diese Datei steht im Rechenpfad und
    /// damit in der Liste von `tests/audit/test_no_float.py`. Der Audit
    /// entfernt `#[cfg(test)]`-Module zwar, aber eine Testhilfe, die
    /// ohne Gleitkomma auskommt, muss die Ausnahme gar nicht erst
    /// beanspruchen.
    pub(super) const EXP_LUT: [i16; 65] = [
        256, 240, 226, 212, 199, 187, 176, 165, 155, 146, 137, 129, 121, 114,
        107, 100, 94, 88, 83, 78, 73, 69, 65, 61, 57, 54, 50, 47, 44, 42, 39,
        37, 35, 33, 31, 29, 27, 25, 24, 22, 21, 20, 19, 17, 16, 15, 14, 14, 13,
        12, 11, 11, 10, 9, 9, 8, 8, 7, 7, 6, 6, 6, 5, 5, 5,
    ];
    const FRAC: u8 = 8;

    fn route(logits: &[i32], k: usize, normieren: bool) -> Routing {
        route_top_k(logits, k, &EXP_LUT, 0, FRAC, normieren)
    }

    // ---- Expertenwacht und Wachstum ---------------------------------

    /// Ein eingehängter Experte ändert die Ausgabe **exakt** nicht.
    ///
    /// Nicht „kaum": Dieselben gewählten Experten, dieselben Gewichte,
    /// dieselben Bytes. Das ist das Akzeptanzkriterium für
    /// funktionserhaltendes Wachstum.
    #[test]
    fn ein_eingehaengter_experte_aendert_die_ausgabe_exakt_nicht() {
        let mut logits: Vec<i32> = vec![120, 40, 200, 80];
        let ausgaben: Vec<Vec<i16>> = (0..4).map(|i| vec![i * 10, -i * 5, i + 1]).collect();
        let k = 2;

        let vorher = route(&logits, k, true);
        let y_vorher = mische_experten(
            &vorher.experten.iter().map(|e| ausgaben[*e as usize].clone()).collect::<Vec<_>>(),
            &vorher.gewichte,
            FRAC,
        );

        let neu = experte_einhaengen(&mut logits);
        let mut ausgaben2 = ausgaben.clone();
        ausgaben2.push(ausgaben[0].clone()); // eine Kopie als Vorbild
        assert_eq!(neu, 4);

        let nachher = route(&logits, k, true);
        let y_nachher = mische_experten(
            &nachher.experten.iter().map(|e| ausgaben2[*e as usize].clone()).collect::<Vec<_>>(),
            &nachher.gewichte,
            FRAC,
        );

        assert_eq!(vorher.experten, nachher.experten, "die Wahl hat sich geaendert");
        assert_eq!(vorher.gewichte, nachher.gewichte, "die Gewichte haben sich geaendert");
        assert_eq!(y_vorher, y_nachher, "die Ausgabe hat sich geaendert");
        assert!(!nachher.experten.contains(&(neu as u16)), "der Neue wurde gewaehlt");
    }

    /// ⚑ **Der Nachweis, der Punkt 5.3 schließt: Die Wacht holt den
    /// neuen Experten zurück.**
    ///
    /// Eingehängt ist er exakt funktionserhaltend und damit tot. Der
    /// Hungerzähler zieht ihn Segment für Segment hoch, bis er zum
    /// ersten Mal gewählt wird. **Damit ist Expertenwachstum keine
    /// unlösbare Aufgabe mehr, sondern ein Lauf**, und der Test fährt
    /// ihn.
    #[test]
    fn ein_neuer_experte_wird_von_der_wacht_zurueckgeholt() {
        let mut logits: Vec<i32> = vec![120, 40, 200, 80];
        let neu = experte_einhaengen(&mut logits);
        let k = 2;

        let mut wacht = Expertenwacht::neu(logits.len(), 3);
        let mut gewaehlt_in = None;
        for schritt in 0..500 {
            let r = route(&logits, k, true);
            wacht.segment(&r.experten);
            if r.experten.contains(&(neu as u16)) {
                gewaehlt_in = Some(schritt);
                break;
            }
            let schub = wacht.schub(2);
            for (z, dz) in logits.iter_mut().zip(schub.iter()) {
                *z += *dz;
            }
        }
        assert!(
            gewaehlt_in.is_some(),
            "der neue Experte wurde nie gewaehlt: {logits:?}"
        );

        // Gegenprobe: Ohne die Wacht bleibt er für immer draußen.
        let mut ohne: Vec<i32> = vec![120, 40, 200, 80];
        let neu2 = experte_einhaengen(&mut ohne);
        for _ in 0..500 {
            let r = route(&ohne, k, true);
            assert!(
                !r.experten.contains(&(neu2 as u16)),
                "ohne Wacht wurde er doch gewaehlt, dann beweist der Test oben nichts"
            );
        }
    }

    /// ⚑ Der Schub summiert sich exakt zu null: Er verteilt um und
    /// verschiebt den Logit-Mittelwert nicht.
    #[test]
    fn der_schub_summiert_sich_zu_null() {
        for (n, hungrig, staerke) in [(4usize, 1usize, 5i32), (8, 3, 7), (16, 5, 3)] {
            let mut wacht = Expertenwacht::neu(n, 2);
            // Die ersten `n - hungrig` bleiben satt, die übrigen hungern.
            for _ in 0..10 {
                let gewaehlt: Vec<u16> = (0..(n - hungrig) as u16).collect();
                wacht.segment(&gewaehlt);
            }
            assert_eq!(wacht.hungernde(), hungrig, "der Aufbau stimmt nicht");
            let d = wacht.schub(staerke);
            let summe: i64 = d.iter().map(|x| *x as i64).sum();
            assert_eq!(summe, 0, "n={n}: Summe {summe} statt null");
            assert!(d.iter().any(|x| *x > 0), "niemand wurde hochgeschoben");
        }
    }

    /// Randfälle: Hungert niemand oder hungern alle, ist der Schub
    /// überall null. Ohne den zweiten Fall gäbe es niemanden zum
    /// Gegenbuchen, und die Summe wäre nicht mehr null.
    #[test]
    fn ohne_hunger_und_bei_hunger_aller_ist_der_schub_null() {
        let mut satt = Expertenwacht::neu(4, 2);
        satt.segment(&[0, 1, 2, 3]);
        assert!(satt.schub(5).iter().all(|x| *x == 0), "ohne Hunger geschoben");

        let mut alle = Expertenwacht::neu(4, 2);
        for _ in 0..10 {
            alle.segment(&[]);
        }
        assert_eq!(alle.hungernde(), 4, "es hungern nicht alle");
        assert!(alle.schub(5).iter().all(|x| *x == 0), "bei Hunger aller geschoben");
    }

    /// Die Wacht zählt über die Segmentfolge, nicht über einen Batch:
    /// Wer gewählt wird, fängt bei null an, alle anderen zählen weiter.
    #[test]
    fn die_wacht_zaehlt_ueber_die_segmentfolge() {
        let mut w = Expertenwacht::neu(3, 100);
        w.segment(&[0]);
        assert_eq!(w.seit(), &[0, 1, 1]);
        w.segment(&[1]);
        assert_eq!(w.seit(), &[1, 0, 2]);
        w.segment(&[0, 1]);
        assert_eq!(w.seit(), &[0, 0, 3]);
    }

    /// Zwei Läufe, dasselbe Ergebnis. Ohne das wäre die Wacht kein
    /// zulässiger Teil eines verifizierbaren Trainingsschritts.
    #[test]
    fn die_wacht_ist_deterministisch() {
        let lauf = || {
            let mut w = Expertenwacht::neu(6, 2);
            for i in 0..20u16 {
                w.segment(&[i % 3]);
            }
            (w.seit().to_vec(), w.schub(4))
        };
        assert_eq!(lauf(), lauf());
    }

    #[test]
    fn die_auswahl_folgt_dem_logit() {
        let r = route(&[10, 50, 30, 40], 2, true);
        assert_eq!(r.experten, vec![1, 3]);
    }

    #[test]
    fn bei_gleichstand_gewinnt_der_kleinere_index() {
        // Drei gleiche Spitzenwerte, zwei Plätze.
        let r = route(&[7, 7, 7, 0], 2, true);
        assert_eq!(r.experten, vec![0, 1], "der kleinere Index gewinnt");
    }

    /// **Gegenprobe zur Tie-Break-Regel.** Eine unstabile Sortierung
    /// liefert für gleiche Elemente eine implementierungsabhängige
    /// Reihenfolge; auf einer Maschine fällt das nie auf, zwischen zwei
    /// Knoten ist es ein Fork. Dieser Test verlangt, dass **jede**
    /// Anordnung gleicher Werte dieselbe Auswahl ergibt.
    #[test]
    fn gleiche_werte_an_anderer_stelle_aendern_die_regel_nicht() {
        // Vier gleiche Werte, zwei Plätze: immer die beiden kleinsten
        // Indizes, egal wie viele gleich sind.
        for n in 2..=8usize {
            let logits: Vec<i32> = vec![5; n];
            let r = route(&logits, 2, true);
            assert_eq!(
                r.experten,
                vec![0, 1],
                "bei {n} gleichen Werten müssen es 0 und 1 sein"
            );
        }
    }

    #[test]
    fn die_gewichte_summieren_sich_exakt_auf_eins() {
        let faelle: [&[i32]; 5] = [
            &[10, 50, 30, 40],
            &[0, 0, 0, 0],
            &[100, 1, 1, 1],
            &[-5, -60, -7, -8, -9, -10],
            &[3, 3, 3, 9, 9, 1],
        ];
        for logits in faelle {
            for k in 1..=logits.len() {
                let r = route(logits, k, true);
                assert_eq!(
                    r.gewichtssumme(),
                    1i64 << FRAC,
                    "Logits {logits:?}, k = {k}"
                );
            }
        }
    }

    /// **Gegenprobe zur Summenkorrektur.** Ohne sie verfehlt die Summe
    /// die Eins; dieser Test hält fest, dass es überhaupt etwas zu
    /// korrigieren gab, sonst prüfte der Test oben eine Eigenschaft, die
    /// sich von selbst einstellt.
    #[test]
    fn ohne_korrektur_verfehlt_die_summe_die_eins() {
        let logits = [3, 3, 3, 3, 3, 3, 3];
        let roh = softmax_int(&logits, &EXP_LUT, 0, FRAC);
        let summe: i64 = roh.iter().map(|g| *g as i64).sum();
        assert_ne!(
            summe,
            1i64 << FRAC,
            "sonst korrigiert korrigiere_summe nie etwas und der Test darüber ist wertlos"
        );
        let r = route(&logits, 7, true);
        assert_eq!(r.gewichtssumme(), 1i64 << FRAC);
    }

    /// Die Identität aus dem Modulkopf, geprüft in der Größenordnung:
    /// Softmax über die Gewählten gegen Softmax über alle mit
    /// anschließender Neunormierung. Reell sind sie gleich, ganzzahlig
    /// trennt sie ein Rundungsschritt.
    #[test]
    fn softmax_ueber_die_gewaehlten_entspricht_dem_normierten_ueber_alle() {
        let logits = [40, 12, 33, 5, 28, 1];
        let k = 3;
        let ueber_gewaehlte = route(&logits, k, true);

        let alle = softmax_int(&logits, &EXP_LUT, 0, FRAC);
        let gewaehlt = waehle_top_k(&logits, k);
        let teilsumme: i64 =
            gewaehlt.iter().map(|e| alle[*e as usize] as i64).sum();
        let ueber_alle: Vec<i64> = gewaehlt
            .iter()
            .map(|e| (alle[*e as usize] as i64 * (1i64 << FRAC)) / teilsumme)
            .collect();

        for (a, b) in ueber_gewaehlte.gewichte.iter().zip(ueber_alle.iter()) {
            let abstand = (*a as i64 - *b).abs();
            assert!(
                abstand <= 2,
                "beide Wege müssen bis auf Rundung übereinstimmen: {a} gegen {b}"
            );
        }
    }

    /// **Gegenprobe zur Festlegung „über die Logits auswählen".** Zwei
    /// verschiedene Logits, die durch die Tabelle auf dieselbe
    /// Wahrscheinlichkeit fallen: Über die Logits ist die Auswahl
    /// eindeutig, über die Wahrscheinlichkeiten entscheidet der
    /// Tie-Break.
    #[test]
    fn ueber_logits_gibt_es_weniger_gleichstaende_als_ueber_wahrscheinlichkeiten() {
        // LUT[46] und LUT[47] sind beide 14.
        assert_eq!(EXP_LUT[46], EXP_LUT[47], "Voraussetzung des Tests");
        let logits = [0, -46, -47, -60];

        assert_eq!(
            randgleichstaende(&logits, 2),
            0,
            "über die Logits ist die Auswahl eindeutig"
        );

        let wahrscheinlichkeiten: Vec<i32> =
            softmax_int(&logits, &EXP_LUT, 0, FRAC);
        assert!(
            randgleichstaende(&wahrscheinlichkeiten, 2) > 0,
            "über die Wahrscheinlichkeiten hängt die Auswahl am Tie-Break"
        );
    }

    #[test]
    fn randgleichstaende_zaehlt_nur_die_an_der_grenze() {
        // Gleichstand zwischen Rang 1 und 2 ändert nichts, beide feuern.
        assert_eq!(randgleichstaende(&[9, 9, 1, 0], 2), 0);
        // Gleichstand zwischen Rang 2 und 3 entscheidet.
        assert_eq!(randgleichstaende(&[9, 5, 5, 0], 2), 1);
        // Zwei weitere auf der Schwelle.
        assert_eq!(randgleichstaende(&[9, 5, 5, 5], 2), 2);
        // Alle gewählt: es gibt keine Grenze.
        assert_eq!(randgleichstaende(&[9, 5, 5, 5], 4), 0);
    }

    #[test]
    fn ohne_normierung_bleiben_die_rohen_wahrscheinlichkeiten() {
        let logits = [40, 12, 33, 5];
        let r = route(&logits, 2, false);
        let alle = softmax_int(&logits, &EXP_LUT, 0, FRAC);
        assert_eq!(r.gewichte, vec![alle[0], alle[2]]);
        assert!(
            r.gewichtssumme() < 1i64 << FRAC,
            "ohne Normierung fehlt der Anteil der nicht gewählten Experten"
        );
    }

    #[test]
    fn mischen_mit_einem_experten_und_gewicht_eins_ist_die_identitaet() {
        let ausgabe = vec![vec![-300i16, 0, 17, 32767]];
        let gemischt = mische_experten(&ausgabe, &[1 << FRAC], FRAC);
        assert_eq!(gemischt, ausgabe[0]);
    }

    /// **Gegenprobe zur i64-Akkumulation.** Mit i32 liefe das
    /// Zwischenergebnis über: 8 Experten à 32767 mal Gewicht 4096 sind
    /// rund 2^30 je Summand und 2^33 in der Summe.
    #[test]
    fn mischen_akkumuliert_ohne_ueberlauf() {
        let ausgaben: Vec<Vec<i16>> = (0..8).map(|_| vec![32767i16; 4]).collect();
        let gewichte = vec![(1 << FRAC) / 8; 8];
        let gemischt = mische_experten(&ausgaben, &gewichte, FRAC);
        // Acht Achtel von 32767, bis auf Rundung.
        for wert in gemischt {
            assert!(
                (wert as i32 - 32767).abs() <= 2,
                "erwartet nahe 32767, war {wert}"
            );
        }
    }

    #[test]
    fn mischen_saettigt_statt_umzulaufen() {
        let ausgaben = vec![vec![32767i16; 2], vec![32767i16; 2]];
        let gewichte = vec![1 << FRAC, 1 << FRAC]; // Summe 2, nicht 1
        let gemischt = mische_experten(&ausgaben, &gewichte, FRAC);
        assert_eq!(gemischt, vec![i16::MAX, i16::MAX]);
    }

    #[test]
    fn k_groesser_als_die_expertenzahl_wird_angeschnitten() {
        let r = route(&[1, 2, 3], 10, true);
        assert_eq!(r.experten.len(), 3);
        assert_eq!(r.gewichtssumme(), 1i64 << FRAC);
    }

    #[test]
    fn k_null_liefert_nichts() {
        let r = route(&[1, 2, 3], 0, true);
        assert!(r.experten.is_empty());
        assert!(r.gewichte.is_empty());
        assert_eq!(r.gewichtssumme(), 0);
    }

    #[test]
    fn ohne_experten_bricht_nichts() {
        let r = route(&[], 4, true);
        assert!(r.experten.is_empty());
        assert_eq!(mische_experten(&[], &[], FRAC), Vec::<i16>::new());
    }
}
