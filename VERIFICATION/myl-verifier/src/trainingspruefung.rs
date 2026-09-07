//! Die Prüfung von Trainingssegmenten (Whitepaper Kap. 5.5 und 7.2).
//!
//! # ⚑ Dieselbe Mechanik wie bei der Inferenz, und das ist der Punkt
//!
//! Zwei Pods rechnen dasselbe Segment, ihre Commitments werden
//! verglichen, bei Abweichung sagt die **Spur**, welcher Shard es war,
//! und ein Nachrechner entscheidet, wer recht hat. Genau der Weg der
//! Inferenz, nur ist die Spur hier nicht über Layer, sondern über
//! **Shards**.
//!
//! # ⚑ Was ein Vergleich sagt und was nicht
//!
//! Ein Vergleich sagt **dass** jemand falsch liegt, nie **wer**. Zwei
//! Pods, die sich widersprechen, sind zwei Behauptungen; erst das
//! Nachrechnen ist eine dritte, unabhängige. Wer aus dem blossen
//! Widerspruch eine Schuld ableitete, bestrafte in der Hälfte der Fälle
//! den Ehrlichen.
//!
//! # ⚑ Der Nachrechner benutzt nicht das Werkzeug des Geprüften
//!
//! Naheliegend wäre, `myl_pod::trainingswerk` zu rufen. **Dann prüfte
//! der Prüfer den Geprüften mit dessen eigenem Werkzeug**, und ein
//! übereinstimmendes Ergebnis hiesse nur, dass dieselbe Funktion zweimal
//! dasselbe tut.
//!
//! [`Modelltrainingsauditor`] geht deshalb **selbst** durch
//! `integer_llm_runtime::shardtraining`. Geteilt wird nur der
//! Spur-Vertrag [`Trainingssegment::commitment_aus_spur`], und der ist
//! ein Konsensdatum.
//!
//! # ⚑ Was der Prüfer wissen muss und was nicht im Segment steht
//!
//! Ein `Trainingssegment` nennt Charge, Modellfassung, Startschritt,
//! Schrittzahl und Lernrate. Es nennt **nicht** den Zuschnitt der
//! Shards. Der kommt vom Aufrufer, genau wie beim Inferenz-Nachrechner,
//! und wer ihn falsch angibt, bekommt eine Abweichung, die keine ist.

use myl_types::hash::Hash;
use myl_types::trainingssegment::Trainingssegment;

/// Was ein Vergleich zweier Segmente ergibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trainingsbefund {
    /// Beide Pods kamen zum selben Commitment.
    Einig,
    /// Die Commitments weichen ab.
    ///
    /// ⚑ **Ohne Schuldzuweisung.** Wer falsch liegt, sagt erst das
    /// Nachrechnen.
    Uneinig,
    /// Die beiden Segmente beschreiben gar nicht denselben Auftrag.
    ///
    /// ⚑ **Das ist kein Streitfall, sondern ein Fehler des Aufrufers.**
    /// Zwei Pods mit verschiedener Charge oder Lernrate haben
    /// verschiedene Arbeit getan; ihre Ergebnisse zu vergleichen wäre
    /// sinnlos, und eine daraus abgeleitete Schuld wäre erfunden.
    VerschiedeneAuftraege {
        /// Welches Feld auseinanderläuft.
        feld: &'static str,
    },
}

/// Wo zwei Spuren zuerst auseinandergehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spurbefund {
    /// Beide Spuren sind gleich.
    Gleich,
    /// Der erste abweichende Shard.
    Abweichung {
        /// Index des Shards, 0-basiert.
        shard: usize,
    },
    /// Die Spuren haben verschiedene Länge.
    ///
    /// ⚑ **Eigener Fall und keine Abweichung an Position `min`.** Zwei
    /// Pods mit verschieden vielen Shards haben nicht dasselbe
    /// gerechnet; das ist ein Zuschnittsfehler und kein Rechenstreit.
    VerschiedeneLaenge { a: usize, b: usize },
}

/// Fehler der Trainingsprüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pruefungsfehler {
    /// Die Spur ist leer, es gibt nichts zu prüfen.
    SpurLeer,
    /// Die Spur passt nicht zum Commitment des Segments.
    ///
    /// ⚑ **Das ist ein Fund am Pod, nicht am Prüfer.** Wer eine Spur
    /// liefert, aus der sein eigenes Commitment nicht folgt, hat
    /// entweder das Commitment erfunden oder die Spur nachträglich
    /// geändert.
    SpurPasstNichtZumCommitment,
    /// Das Nachrechnen ist gescheitert.
    Nachrechnen(String),
}

impl std::fmt::Display for Pruefungsfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpurLeer => f.write_str("die Spur ist leer"),
            Self::SpurPasstNichtZumCommitment => {
                f.write_str("die Spur ergibt nicht das Commitment des Segments")
            }
            Self::Nachrechnen(e) => write!(f, "Nachrechnen gescheitert: {e}"),
        }
    }
}

impl std::error::Error for Pruefungsfehler {}

/// Vergleicht zwei Segmente desselben Auftrags.
///
/// ⚑ **Zuerst wird geprüft, ob es überhaupt derselbe Auftrag ist.**
/// Sonst hiesse „uneinig" nur, dass zwei Pods Verschiedenes gerechnet
/// haben, und genau das sollen sie bei verschiedenen Aufträgen.
pub fn segmente_vergleichen(a: &Trainingssegment, b: &Trainingssegment) -> Trainingsbefund {
    if a.charge != b.charge {
        return Trainingsbefund::VerschiedeneAuftraege { feld: "charge" };
    }
    if a.modell_version != b.modell_version {
        return Trainingsbefund::VerschiedeneAuftraege { feld: "modell_version" };
    }
    if a.startschritt != b.startschritt || a.schrittzahl != b.schrittzahl {
        return Trainingsbefund::VerschiedeneAuftraege { feld: "schritte" };
    }
    if a.lr_zaehler != b.lr_zaehler || a.lr_nenner != b.lr_nenner {
        return Trainingsbefund::VerschiedeneAuftraege { feld: "lernrate" };
    }
    // ⚑ **Die Wirkung gehört zum Vergleich, nicht zum Auftrag.** Zwei
    // Pods mit demselben Auftrag und demselben Δm müssen **dieselbe
    // Zahl bewegter Gewichte** melden; sie folgt aus dem Δm. Weichen
    // sie ab, ist das eine Uneinigkeit über das Ergebnis und keine über
    // die Bestellung, also `Uneinig` und nicht `VerschiedeneAuftraege`.
    if a.bewegte_gewichte != b.bewegte_gewichte {
        return Trainingsbefund::Uneinig;
    }
    if a.delta_commitment == b.delta_commitment {
        Trainingsbefund::Einig
    } else {
        Trainingsbefund::Uneinig
    }
}

/// Findet den ersten Shard, an dem zwei Spuren auseinandergehen.
///
/// ⚑ **Das ersetzt die Bisektion, es ergänzt sie nicht.** Die Spur ist
/// so lang wie der Pod Shards hat, also vier bis acht Einträge; sie
/// vollständig zu übertragen kostet weniger als eine einzige
/// Bisektionsrunde. Erst **innerhalb** eines Shards, über seine Ebenen,
/// lohnt das Halbieren.
pub fn spuren_vergleichen(a: &[Hash], b: &[Hash]) -> Spurbefund {
    if a.len() != b.len() {
        return Spurbefund::VerschiedeneLaenge { a: a.len(), b: b.len() };
    }
    match a.iter().zip(b.iter()).position(|(x, y)| x != y) {
        Some(shard) => Spurbefund::Abweichung { shard },
        None => Spurbefund::Gleich,
    }
}

/// Prüft, ob eine gelieferte Spur zum Commitment des Segments passt.
///
/// ⚑ **Vor jedem Nachrechnen.** Wer eine Spur liefert, aus der sein
/// eigenes Commitment nicht folgt, hat sie nachträglich geändert, und
/// dann wäre jedes Nachrechnen gegen die falsche Behauptung geführt.
pub fn spur_gehoert_zum_segment(
    segment: &Trainingssegment,
    spur: &[Hash],
) -> Result<(), Pruefungsfehler> {
    if spur.is_empty() {
        return Err(Pruefungsfehler::SpurLeer);
    }
    if Trainingssegment::commitment_aus_spur(spur) != segment.delta_commitment {
        return Err(Pruefungsfehler::SpurPasstNichtZumCommitment);
    }
    Ok(())
}

/// Der Auftrag eines einzelnen Shards, so wie ihn ein Prüfer nachrechnet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shardauftrag {
    /// Erste Ebene des Bereichs, **global**.
    pub von: usize,
    /// Erste Ebene **nach** dem Bereich.
    pub bis: usize,
    /// Die Tokenfolge der Charge.
    ///
    /// ⚑ **Sie steht nicht im Segment**, sondern folgt aus der Charge
    /// über den verankerten Korpus. Wer sie falsch beschafft, bekommt
    /// eine Abweichung, die keine ist.
    pub folge: Vec<usize>,
    /// Das Zielwort.
    pub ziel: usize,
    /// Wie viele Schritte.
    pub schritte: u64,
    /// Nenner der Lernrate.
    pub lr_nenner: i64,
}

/// Wer ein Trainingssegment nachrechnen kann.
pub trait Trainingsauditor {
    /// Rechnet die Spur eines Segments nach: je Shard ein Δ-Commitment.
    fn nachrechnen(&self, auftraege: &[Shardauftrag]) -> Result<Vec<Hash>, Pruefungsfehler>;
}

/// Das Urteil über einen einzelnen Shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shardurteil {
    /// Der nachgerechnete Wert stimmt mit dem behaupteten überein.
    Bestaetigt,
    /// Er stimmt nicht.
    Widerlegt,
}

/// Rechnet eine Spur nach und urteilt Shard für Shard.
///
/// ⚑ **Ein Urteil je Shard und nicht eines fürs Segment.** Ein Pod, bei
/// dem ein Shard falsch rechnet, hat nicht überall falsch gerechnet, und
/// die Haftung trifft den Shard, nicht den Pod: Der `pod_pfad` sagt, wer
/// auf welcher Position sass.
pub fn spur_nachrechnen(
    auditor: &impl Trainingsauditor,
    behauptet: &[Hash],
    auftraege: &[Shardauftrag],
) -> Result<Vec<Shardurteil>, Pruefungsfehler> {
    if behauptet.is_empty() {
        return Err(Pruefungsfehler::SpurLeer);
    }
    let nachgerechnet = auditor.nachrechnen(auftraege)?;
    if nachgerechnet.len() != behauptet.len() {
        return Err(Pruefungsfehler::Nachrechnen(format!(
            "der Nachrechner lieferte {} Shards, behauptet sind {}",
            nachgerechnet.len(),
            behauptet.len()
        )));
    }
    Ok(behauptet
        .iter()
        .zip(nachgerechnet.iter())
        .map(|(b, n)| if b == n { Shardurteil::Bestaetigt } else { Shardurteil::Widerlegt })
        .collect())
}

/// Ein Nachrechner, der ein echtes Modell fährt.
///
/// # ⚑ Er braucht nichts, was der Beschuldigte liefert
///
/// Das ist der Unterschied zur Inferenzprüfung, und er ist ein Vorteil.
/// Dort beginnt die Nachrechnung mit den **Eingangsaktivierungen aus der
/// Spur des Beschuldigten**; wer sie fälscht, verschiebt die Prüfung.
///
/// Ein Trainingssegment ist vollständig bestimmt durch die Gewichte der
/// Modellfassung, die Charge, die Lernrate und die Schrittzahl. **Alle
/// vier stehen im Segment oder folgen aus dem verankerten Korpus.** Der
/// Nachrechner fängt beim Artefakt an und rechnet den ganzen Lauf.
///
/// ⚑ **Der Preis ist, dass er den ganzen Lauf rechnet** und nicht nur
/// den beschuldigten Shard. Das ist kein Versäumnis: Der Δ eines Shards
/// hängt über den Rückwärtsweg an allen Shards hinter ihm, und wer nur
/// einen nachrechnete, müsste dessen Ausgangsgradienten vom
/// Beschuldigten nehmen.
pub struct Modelltrainingsauditor {
    modell: std::sync::Arc<integer_llm_runtime::model::IntegerModel>,
}

impl Modelltrainingsauditor {
    /// Neu, mit geladenem Modell.
    pub fn neu(modell: std::sync::Arc<integer_llm_runtime::model::IntegerModel>) -> Self {
        Self { modell }
    }
}

impl Trainingsauditor for Modelltrainingsauditor {
    fn nachrechnen(&self, auftraege: &[Shardauftrag]) -> Result<Vec<Hash>, Pruefungsfehler> {
        use integer_llm_runtime::shardtraining::{
            rueckwaerts, vorwaerts, Shardgewichte, Shardvorgaben,
        };

        if auftraege.is_empty() {
            return Err(Pruefungsfehler::SpurLeer);
        }
        let m = &self.modell;
        let erster = &auftraege[0];
        // ⚑ **Alle Aufträge eines Segments teilen Folge, Ziel, Schritte
        // und Lernrate.** Wichen sie ab, beschrieben sie verschiedene
        // Segmente, und das Ergebnis wäre keine Nachrechnung.
        for a in auftraege {
            if a.folge != erster.folge
                || a.ziel != erster.ziel
                || a.schritte != erster.schritte
                || a.lr_nenner != erster.lr_nenner
            {
                return Err(Pruefungsfehler::Nachrechnen(
                    "die Shardauftraege beschreiben verschiedene Segmente".to_string(),
                ));
            }
        }
        if erster.folge.is_empty() {
            return Err(Pruefungsfehler::Nachrechnen("die Folge ist leer".to_string()));
        }
        if erster.ziel >= m.vocab_size {
            return Err(Pruefungsfehler::Nachrechnen(format!(
                "Zielwort {} liegt ausserhalb des Vokabulars",
                erster.ziel
            )));
        }

        let mut gewichte: Vec<Shardgewichte> = Vec::with_capacity(auftraege.len());
        for a in auftraege {
            gewichte.push(
                Shardgewichte::aus_modell(m, a.von, a.bis)
                    .map_err(|e| Pruefungsfehler::Nachrechnen(e.to_string()))?,
            );
        }
        let v = integer_llm_runtime::trainingsschleife::Trainingsvorgaben {
            folge: erster.folge.clone(),
            ziel: erster.ziel,
            schritte: erster.schritte,
            lr_nenner: erster.lr_nenner,
            ..integer_llm_runtime::trainingsschleife::Trainingsvorgaben::vorgabe()
        };
        let letzte = v.folge.len() - 1;

        for s in 0..v.schritte {
            let mut strom: Vec<Vec<i16>> =
                v.folge.iter().map(|t| m.embed_token(*t)).collect();
            let mut mitschnitte = Vec::with_capacity(auftraege.len());
            for (j, a) in auftraege.iter().enumerate() {
                let vg = Shardvorgaben {
                    von: a.von,
                    bis: a.bis,
                    schritt: s,
                    lr_zaehler: 1,
                    lr_nenner: a.lr_nenner,
                };
                let ms = vorwaerts(m, &mut gewichte[j], &vg, &strom)
                    .map_err(|e| Pruefungsfehler::Nachrechnen(format!("Shard {j}: {e}")))?;
                strom = ms.ausgang.clone();
                mitschnitte.push(ms);
            }
            let (_logits, g_y) = integer_llm_runtime::trainingsschleife::gradient_vom_ziel(
                m,
                &strom[letzte],
                &v,
            );
            let mut g: Vec<Vec<i32>> =
                (0..v.folge.len()).map(|_| vec![0i32; m.hidden_size]).collect();
            g[letzte] = g_y;
            for (j, a) in auftraege.iter().enumerate().rev() {
                let vg = Shardvorgaben {
                    von: a.von,
                    bis: a.bis,
                    schritt: s,
                    lr_zaehler: 1,
                    lr_nenner: a.lr_nenner,
                };
                let erg = rueckwaerts(m, &mut gewichte[j], &vg, &mitschnitte[j], &g)
                    .map_err(|e| Pruefungsfehler::Nachrechnen(format!("Shard {j}: {e}")))?;
                g = erg.eingang;
            }
        }

        Ok(gewichte.iter().map(delta_abdruck).collect())
    }
}

/// Der Δ-Abdruck eines Shards, in derselben Form wie im Pod.
///
/// ⚑ **Dieselbe Reihenfolge und dieselbe Funktion.** Wer hier anders
/// verkettete, bekäme einen anderen Abdruck über dieselbe Arbeit, und
/// jeder Streit wäre unentscheidbar.
fn delta_abdruck(g: &integer_llm_runtime::shardtraining::Shardgewichte) -> Hash {
    // ⚑ **`Shardgewichte::deltas` und keine eigene Schleife.** Bei einer
    // Gemischebene haengt die Reihenfolge an den Expertennummern, und
    // wer sie hier nachbaute, baute die Konsensordnung ein zweites Mal.
    let deltas = g.deltas();
    let scheiben: Vec<&[i32]> = deltas.iter().map(|v| v.as_slice()).collect();
    let hex = integer_llm_kernels::optimierer::trainingsabdruck(&scheiben);
    let mut b = [0u8; 32];
    for (i, paar) in hex.as_bytes().chunks(2).enumerate() {
        b[i] = u8::from_str_radix(std::str::from_utf8(paar).expect("Hex ist ASCII"), 16)
            .expect("trainingsabdruck liefert Hex");
    }
    Hash(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::ids::{MerkleRoot, MinerId, SegmentId};

    fn segment(charge: u8, commitment: u8) -> Trainingssegment {
        Trainingssegment {
            id: SegmentId::new([1u8; 32]),
            modell_version: MerkleRoot::new([2u8; 32]),
            charge: Hash([charge; 32]),
            startschritt: 0,
            schrittzahl: 30,
            folgen: 1,
            lr_zaehler: 1,
            lr_nenner: 4096,
            delta_commitment: Hash([commitment; 32]),
            bewegte_gewichte: 1,
            pod_pfad: vec![MinerId::new([3u8; 32])],
            signaturen: vec![myl_types::bls::BlsSignature([0u8; 96])],
        }
    }

    #[test]
    fn gleiche_arbeit_und_gleiches_ergebnis_ist_einig() {
        assert_eq!(segmente_vergleichen(&segment(1, 9), &segment(1, 9)), Trainingsbefund::Einig);
    }

    #[test]
    fn gleiche_arbeit_und_verschiedenes_ergebnis_ist_uneinig() {
        assert_eq!(segmente_vergleichen(&segment(1, 9), &segment(1, 8)), Trainingsbefund::Uneinig);
    }

    /// ⚑ **Verschiedene Aufträge sind kein Streitfall.**
    #[test]
    fn verschiedene_auftraege_werden_als_solche_gemeldet() {
        assert_eq!(
            segmente_vergleichen(&segment(1, 9), &segment(2, 8)),
            Trainingsbefund::VerschiedeneAuftraege { feld: "charge" }
        );
        let mut a = segment(1, 9);
        let mut b = segment(1, 9);
        b.lr_nenner = 8192;
        assert_eq!(
            segmente_vergleichen(&a, &b),
            Trainingsbefund::VerschiedeneAuftraege { feld: "lernrate" }
        );
        a.schrittzahl = 31;
        b.lr_nenner = 4096;
        assert_eq!(
            segmente_vergleichen(&a, &b),
            Trainingsbefund::VerschiedeneAuftraege { feld: "schritte" }
        );
    }

    #[test]
    fn die_spur_zeigt_auf_den_ersten_abweichenden_shard() {
        let a = [Hash([1; 32]), Hash([2; 32]), Hash([3; 32]), Hash([4; 32])];
        let mut b = a;
        assert_eq!(spuren_vergleichen(&a, &b), Spurbefund::Gleich);
        b[2] = Hash([9; 32]);
        assert_eq!(spuren_vergleichen(&a, &b), Spurbefund::Abweichung { shard: 2 });
        // Auch wenn danach noch mehr abweicht: der **erste** zaehlt.
        b[3] = Hash([8; 32]);
        assert_eq!(spuren_vergleichen(&a, &b), Spurbefund::Abweichung { shard: 2 });
    }

    /// ⚑ **Verschiedene Länge ist ein Zuschnittsfehler, keine
    /// Abweichung.**
    #[test]
    fn verschiedene_spurlaengen_sind_ein_eigener_fall() {
        let a = [Hash([1; 32]), Hash([2; 32])];
        let b = [Hash([1; 32])];
        assert_eq!(spuren_vergleichen(&a, &b), Spurbefund::VerschiedeneLaenge { a: 2, b: 1 });
    }

    /// Eine Spur, aus der das Commitment nicht folgt, wird abgewiesen.
    #[test]
    fn eine_nachtraeglich_geaenderte_spur_faellt_auf() {
        let spur = vec![Hash([1; 32]), Hash([2; 32])];
        let mut seg = segment(1, 0);
        seg.delta_commitment = Trainingssegment::commitment_aus_spur(&spur);
        spur_gehoert_zum_segment(&seg, &spur).expect("die eigene Spur muss passen");

        let gefaelscht = vec![Hash([1; 32]), Hash([9; 32])];
        assert_eq!(
            spur_gehoert_zum_segment(&seg, &gefaelscht),
            Err(Pruefungsfehler::SpurPasstNichtZumCommitment)
        );
        assert_eq!(spur_gehoert_zum_segment(&seg, &[]), Err(Pruefungsfehler::SpurLeer));
    }

    struct Stummel(Vec<Hash>);
    impl Trainingsauditor for Stummel {
        fn nachrechnen(&self, _a: &[Shardauftrag]) -> Result<Vec<Hash>, Pruefungsfehler> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn das_nachrechnen_urteilt_je_shard() {
        let behauptet = vec![Hash([1; 32]), Hash([2; 32]), Hash([3; 32])];
        let auditor = Stummel(vec![Hash([1; 32]), Hash([9; 32]), Hash([3; 32])]);
        let urteil = spur_nachrechnen(&auditor, &behauptet, &[]).expect("Urteil");
        assert_eq!(
            urteil,
            vec![Shardurteil::Bestaetigt, Shardurteil::Widerlegt, Shardurteil::Bestaetigt]
        );
    }

    /// ⚑ **Ein Nachrechner mit falscher Shardzahl urteilt nicht.**
    #[test]
    fn eine_falsche_shardzahl_ist_ein_fehler_und_kein_urteil() {
        let behauptet = vec![Hash([1; 32]), Hash([2; 32])];
        let auditor = Stummel(vec![Hash([1; 32])]);
        assert!(matches!(
            spur_nachrechnen(&auditor, &behauptet, &[]),
            Err(Pruefungsfehler::Nachrechnen(_))
        ));
    }
}
