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

#[cfg(test)]
mod tests {
    use super::*;

    /// exp(-i/16) · 256, ganzzahlig vorberechnet. Bewusst als Konstante
    /// und nicht per `f64` erzeugt: Diese Datei steht im Rechenpfad und
    /// damit in der Liste von `tests/audit/test_no_float.py`. Der Audit
    /// entfernt `#[cfg(test)]`-Module zwar, aber eine Testhilfe, die
    /// ohne Gleitkomma auskommt, muss die Ausnahme gar nicht erst
    /// beanspruchen.
    const EXP_LUT: [i16; 65] = [
        256, 240, 226, 212, 199, 187, 176, 165, 155, 146, 137, 129, 121, 114,
        107, 100, 94, 88, 83, 78, 73, 69, 65, 61, 57, 54, 50, 47, 44, 42, 39,
        37, 35, 33, 31, 29, 27, 25, 24, 22, 21, 20, 19, 17, 16, 15, 14, 14, 13,
        12, 11, 11, 10, 9, 9, 8, 8, 7, 7, 6, 6, 6, 5, 5, 5,
    ];
    const FRAC: u8 = 8;

    fn route(logits: &[i32], k: usize, normieren: bool) -> Routing {
        route_top_k(logits, k, &EXP_LUT, 0, FRAC, normieren)
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
