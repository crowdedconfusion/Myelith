//! Die Segmentkette: Whitepaper Kap. 8.4.
//!
//! Ein Agent arbeitet in Schritten, und jeder Schritt ist ein eigenes
//! Inferenz-Segment mit eigener Verifikation. **Damit auch der Ablauf
//! nachprüfbar bleibt, hängt jeder Schritt am Ausgabe-Commitment seines
//! Vorgängers.** Es entsteht dieselbe Struktur wie die Berechnungsspur
//! innerhalb eines Segments (Kap. 6.1), nur eine Ebene höher.
//!
//! ## ⚑ Eine Kette allein belegt zu wenig
//!
//! Eine Folge von Gliedern, die sauber aneinanderhängen, ist **in sich**
//! stimmig, und das heißt nicht, dass sie richtig ist. Wer einen Schritt
//! auslässt und die Kette danach neu knüpft, bekommt wieder eine in sich
//! stimmige Kette, nur eine kürzere.
//!
//! **Geprüft wird deshalb gegen den Plan**, nicht gegen die Kette
//! selbst. Der Plan sagt, wie viele Schritte es sind und welches
//! Werkzeug an welcher Stelle läuft; beides geht in den Kettenwert ein.
//! Damit fallen die drei Fälle aus Kap. 8.4 auseinander:
//!
//! - **ausgelassen**: die Länge stimmt nicht mehr mit dem Plan überein;
//! - **eingefügt**: ebenso, und zusätzlich sitzt an der Stelle ein
//!   Werkzeug, das der Plan dort nicht vorsieht;
//! - **vertauscht**: die Länge stimmt, aber die Stellen tragen die
//!   falschen Werkzeuge, und der Wert läuft auseinander.
//!
//! ## Der Anker, und warum es ihn braucht
//!
//! ⚑ **Ohne Startwert ließe sich eine ganze Kette aus einer Session in
//! eine andere heben.** Sie hinge weiter sauber zusammen, gehörte aber
//! zu einem anderen Auftrag mit anderen Grenzen. Der Anker bindet sie an
//! **Session und Plan**: dieselbe Arbeit unter einem anderen Kontrakt
//! ergibt einen anderen Anker und damit eine andere Kette.
//!
//! ## Was hier nicht steht
//!
//! **Wo eine Kette bricht, sagt diese Datei nicht.** Der Kettenwert
//! stimmt oder nicht; welcher Schritt schuld ist, findet die Bisektion
//! in VERIFICATION, die es dafür schon gibt. Eine zweite Suche daneben
//! wäre eine zweite Quelle für dieselbe Aussage.
//!
//! Wer zwei Ketten **hat**, kommt billiger davon: [`erster_unterschied`]
//! nennt die Stelle sofort. Das ist der Redundanzvergleich aus Kap. 6.4,
//! eine Ebene höher, und er kostet nichts, weil beide Ketten ohnehin
//! vorliegen.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::hash::Hash;
use myl_types::ids::{SegmentId, SitzungId};
use myl_types::sitzung::{Sitzungskontrakt, Sitzungszustand, Waehrung};

use crate::plan::Plan;

/// Trennzeichen des Kettenankers.
pub const DST_KETTENANKER: &[u8] = b"MYELITH_KETTENANKER_v1";

/// Trennzeichen eines Kettenglieds.
pub const DST_KETTENGLIED: &[u8] = b"MYELITH_KETTENGLIED_v1";

/// Der Startwert einer Kette: bindet sie an Session und Plan.
///
/// ⚑ **Beides, nicht eines von beidem.** Ohne die Session ließe sich
/// eine Kette unter einen anderen Kontrakt mit anderen Grenzen legen;
/// ohne den Plan ließe sich der Plan nachträglich zu dem umschreiben,
/// was geschehen ist.
pub fn anker(sitzung: &SitzungId, plan: &Hash) -> Hash {
    let mut daten = Vec::with_capacity(DST_KETTENANKER.len() + 64);
    daten.extend_from_slice(DST_KETTENANKER);
    daten.extend_from_slice(sitzung.as_bytes());
    daten.extend_from_slice(plan.as_bytes());
    Hash::sha256(&daten)
}

/// Ein Schritt in der Kette.
///
/// **Die Stelle steht nicht darin**, und das Werkzeug auch nicht: Beides
/// kommt aus dem Plan, gegen den geprüft wird. Stünde es hier, wäre es
/// eine zweite Quelle für dieselbe Aussage, und zwei Quellen laufen
/// auseinander.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kettenglied {
    /// Welches Inferenz-Segment diesen Schritt gerechnet hat.
    pub segment: SegmentId,
    /// Das Ausgabe-Commitment dieses Schritts.
    pub ausgabe: Hash,
}

/// Warum eine Kette nicht gilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kettenfehler {
    /// Die Kette hat nicht so viele Glieder, wie der Plan Schritte hat.
    ///
    /// **Der Fall „ausgelassen" und der Fall „eingefügt" landen beide
    /// hier**, und das ist richtig so: Von außen sind sie dasselbe,
    /// nämlich eine Kette, die nicht zu ihrem Plan gehört.
    LaengeStimmtNicht {
        /// Schritte im Plan.
        plan: usize,
        /// Glieder in der Kette.
        kette: usize,
    },
}

impl std::fmt::Display for Kettenfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LaengeStimmtNicht { plan, kette } => {
                write!(f, "Plan hat {plan} Schritte, Kette {kette} Glieder")
            }
        }
    }
}

impl std::error::Error for Kettenfehler {}

/// Rechnet den Kettenwert aus.
///
/// Gefaltet über die Glieder, und in **jeden** Faltungsschritt gehen
/// vier Dinge ein: der Wert bis hierher, die Stelle, das **vom Plan
/// vorgesehene** Werkzeug und die Ausgabe des Schritts.
///
/// **Die Stelle geht mit ein**, sonst wären zwei vertauschte Schritte
/// mit denselben Ausgaben nicht zu unterscheiden. **Das Werkzeug geht
/// mit ein**, sonst wäre ein Plan mit derselben Länge und anderen
/// Werkzeugen derselbe Wert.
pub fn kettenwert(
    anker: &Hash,
    plan: &Plan,
    glieder: &[Kettenglied],
) -> Result<Hash, Kettenfehler> {
    if glieder.len() != plan.len() {
        return Err(Kettenfehler::LaengeStimmtNicht {
            plan: plan.len(),
            kette: glieder.len(),
        });
    }
    let mut wert = *anker;
    for (i, (glied, schritt)) in glieder.iter().zip(plan.schritte()).enumerate() {
        let mut daten = Vec::with_capacity(DST_KETTENGLIED.len() + 8 + 32 * 4);
        daten.extend_from_slice(DST_KETTENGLIED);
        daten.extend_from_slice(wert.as_bytes());
        daten.extend_from_slice(&(i as u64).to_le_bytes());
        daten.extend_from_slice(schritt.werkzeug.as_bytes());
        daten.extend_from_slice(glied.segment.as_bytes());
        daten.extend_from_slice(glied.ausgabe.as_bytes());
        wert = Hash::sha256(&daten);
    }
    Ok(wert)
}

/// Das Ergebnis einer Kettenprüfung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kettenbefund {
    /// Die Kette gehört zu diesem Plan und dieser Session.
    Heil,
    /// Die Länge passt nicht zum Plan.
    LaengeStimmtNicht {
        /// Schritte im Plan.
        plan: usize,
        /// Glieder in der Kette.
        kette: usize,
    },
    /// Die Länge passt, der Wert nicht.
    WertStimmtNicht {
        /// Was vorgelegt wurde.
        behauptet: Hash,
        /// Was nachgerechnet herauskommt.
        gerechnet: Hash,
    },
}

impl Kettenbefund {
    /// Ist die Kette heil?
    pub fn heil(&self) -> bool {
        matches!(self, Self::Heil)
    }
}

/// Prüft eine Kette gegen Plan, Session und einen behaupteten Wert.
pub fn pruefe_kette(
    sitzung: &SitzungId,
    plan: &Plan,
    glieder: &[Kettenglied],
    behauptet: &Hash,
) -> Kettenbefund {
    let a = anker(sitzung, &plan.adresse());
    match kettenwert(&a, plan, glieder) {
        Err(Kettenfehler::LaengeStimmtNicht { plan, kette }) => {
            Kettenbefund::LaengeStimmtNicht { plan, kette }
        }
        Ok(gerechnet) if gerechnet == *behauptet => Kettenbefund::Heil,
        Ok(gerechnet) => Kettenbefund::WertStimmtNicht {
            behauptet: *behauptet,
            gerechnet,
        },
    }
}

/// Die erste Stelle, an der sich zwei Ketten unterscheiden.
///
/// Der Redundanzvergleich aus Kap. 6.4, eine Ebene höher: Zwei Pods, die
/// dieselbe Session gerechnet haben, legen zwei Ketten vor. **Wo sie
/// zuerst auseinandergehen, ist der Schritt, über den zu streiten ist**,
/// und dorthin gehört die Bisektion.
///
/// Verschiedene Längen gelten ab der kürzeren als Unterschied.
pub fn erster_unterschied(a: &[Kettenglied], b: &[Kettenglied]) -> Option<usize> {
    let gemeinsam = a.len().min(b.len());
    for i in 0..gemeinsam {
        if a[i] != b[i] {
            return Some(i);
        }
    }
    if a.len() == b.len() {
        None
    } else {
        Some(gemeinsam)
    }
}

/// Warum eine Session geendet hat (Kap. 8.4, Abbruchbedingungen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ende {
    /// Der Plan ist abgearbeitet.
    Vollstaendig,
    /// Der Kontrakt lässt keine weiteren Schritte zu.
    Schrittzahl {
        /// Die Grenze.
        grenze: u32,
    },
    /// In beiden Währungen ist nichts mehr übrig.
    Budget,
    /// Das Zeitfenster ist zu.
    Frist,
    /// Der Inhaber hat beendet.
    Widerrufen,
    /// Nichts davon: Es geht weiter.
    Laeuft,
}

/// Warum die Session hier endet, oder dass sie weiterläuft.
///
/// ⚑ **„Zielerreichung" steht in Kap. 8.4 und ist nicht maschinell
/// entscheidbar.** Ob ein Auftrag erfüllt ist, beurteilt ein Mensch;
/// eine Maschine kann nur sehen, dass der **Plan** zu Ende ist. Genau
/// das heißt [`Ende::Vollstaendig`], und es heißt nicht „gelungen".
/// Diese Unterscheidung hier zu verwischen hieße, dem Konsens eine
/// Beurteilung zuzuschreiben, die er nicht leisten kann.
///
/// Die Reihenfolge der Prüfungen ist die der Endgültigkeit: Ein Widerruf
/// gilt, auch wenn die Frist noch läuft.
pub fn ende(
    kontrakt: &Sitzungskontrakt,
    zustand: &Sitzungszustand,
    jetzt: myl_types::ids::EpochId,
    plan: &Plan,
    gelaufen: usize,
) -> Ende {
    if zustand.widerrufen {
        return Ende::Widerrufen;
    }
    if jetzt.0 > kontrakt.gueltig_bis.0 {
        return Ende::Frist;
    }
    if gelaufen >= plan.len() {
        return Ende::Vollstaendig;
    }
    if gelaufen as u64 >= kontrakt.max_schritte as u64 {
        return Ende::Schrittzahl { grenze: kontrakt.max_schritte };
    }
    let nichts_mehr = [Waehrung::Credits, Waehrung::Myl].iter().all(|w| {
        kontrakt.grenzen(*w).budget.saturating_sub(zustand.verbraucht(*w)) == 0
    });
    if nichts_mehr {
        return Ende::Budget;
    }
    Ende::Laeuft
}

/// Passt ein Plan überhaupt unter diesen Kontrakt?
///
/// ⚑ **Geprüft wird vor dem ersten Schritt und nicht nach dem letzten.**
/// Ein Plan, der die Schrittzahl überschreitet, würde sonst zur Hälfte
/// laufen und dann abbrechen; bezahlt wäre die Hälfte, erreicht nichts.
///
/// **Was der Konsens davon heute durchsetzt, ist Budget und Frist**
/// (Kap. 8.2, gebaut). Die Schrittzahl setzt hier der Agent Layer durch,
/// weil das Ledger den Plan nicht sieht. Damit sie auch im Konsens
/// gilt, müsste die Planadresse in den Sitzungszustand wandern; das ist
/// ein eigener Punkt und keine Zeile, die hier fehlt.
pub fn plan_passt(kontrakt: &Sitzungskontrakt, plan: &Plan) -> bool {
    plan.len() as u64 <= kontrakt.max_schritte as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Herkunft, Werkzeugart, Werkzeugmanifest};
    use crate::plan::{Quelle, Schritt};
    use crate::registratur::Registratur;
    use myl_types::ids::{Address, EpochId, MerkleRoot};
    use myl_types::sitzung::Grenzen;

    fn werkzeuge() -> (MerkleRoot, MerkleRoot) {
        let mut r = Registratur::neu();
        let mach = |name: &str| Werkzeugmanifest {
            name: name.into(),
            anbieter: "prüfstand".into(),
            revision: "1".into(),
            lizenz: "Apache-2.0".into(),
            art: Werkzeugart::Deterministisch,
            herkunft: Herkunft::Verankert,
        };
        (
            r.nimm_werkzeug(mach("eins")).expect("gültig"),
            r.nimm_werkzeug(mach("zwei")).expect("gültig"),
        )
    }

    fn plan_mit(werkzeuge: &[MerkleRoot]) -> Plan {
        Plan::neu(
            werkzeuge
                .iter()
                .enumerate()
                .map(|(i, w)| Schritt {
                    werkzeug: *w,
                    argumente: if i == 0 { vec![] } else { vec![Quelle::Schritt(i as u16 - 1)] },
                })
                .collect(),
        )
        .expect("gültiger Plan")
    }

    fn glied(n: u8) -> Kettenglied {
        Kettenglied {
            segment: SegmentId::new([n; 32]),
            ausgabe: Hash::sha256(&[n, n, n]),
        }
    }

    fn sitzung() -> SitzungId {
        SitzungId::new([77u8; 32])
    }

    fn kontrakt(max_schritte: u32, budget: u64) -> Sitzungskontrakt {
        Sitzungskontrakt::neu(
            Address::new([1; 32]),
            Address::new([2; 32]),
            Grenzen { budget, einzellimit: budget, schwelle: u64::MAX, zeugenleiter: Vec::new() },
            Grenzen::gesperrt(),
            vec![Address::new([10; 32])],
            EpochId(0),
            EpochId(100),
            max_schritte,
        )
        .expect("gültiger Kontrakt")
    }

    /// Eine heile Kette wird als heil erkannt, und der Wert ist
    /// deterministisch.
    #[test]
    fn eine_heile_kette_ist_heil() {
        let (a, b) = werkzeuge();
        let p = plan_mit(&[a, b, a]);
        let g = vec![glied(1), glied(2), glied(3)];
        let wert = kettenwert(&anker(&sitzung(), &p.adresse()), &p, &g).expect("Länge passt");

        assert_eq!(pruefe_kette(&sitzung(), &p, &g, &wert), Kettenbefund::Heil);
        assert!(pruefe_kette(&sitzung(), &p, &g, &wert).heil());
        // Zweimal gerechnet ergibt dasselbe.
        assert_eq!(
            kettenwert(&anker(&sitzung(), &p.adresse()), &p, &g).expect("gut"),
            wert
        );
    }

    /// ⚑ **Das Akzeptanzkriterium der Phase, als Test:** ausgelassen,
    /// eingefügt und vertauscht fallen alle drei auf.
    #[test]
    fn ausgelassen_eingefuegt_vertauscht_fallen_auf() {
        let (a, b) = werkzeuge();
        let p = plan_mit(&[a, b, a]);
        let echt = vec![glied(1), glied(2), glied(3)];
        let wert = kettenwert(&anker(&sitzung(), &p.adresse()), &p, &echt).expect("gut");

        // Ausgelassen: der mittlere Schritt fehlt.
        let ausgelassen = vec![glied(1), glied(3)];
        assert_eq!(
            pruefe_kette(&sitzung(), &p, &ausgelassen, &wert),
            Kettenbefund::LaengeStimmtNicht { plan: 3, kette: 2 }
        );

        // Eingefügt: einer zu viel.
        let eingefuegt = vec![glied(1), glied(2), glied(9), glied(3)];
        assert_eq!(
            pruefe_kette(&sitzung(), &p, &eingefuegt, &wert),
            Kettenbefund::LaengeStimmtNicht { plan: 3, kette: 4 }
        );

        // Vertauscht: dieselbe Länge, dieselben Glieder, andere
        // Reihenfolge. **Genau hier trägt die Stelle im Kettenwert.**
        let vertauscht = vec![glied(2), glied(1), glied(3)];
        assert!(matches!(
            pruefe_kette(&sitzung(), &p, &vertauscht, &wert),
            Kettenbefund::WertStimmtNicht { .. }
        ));
    }

    /// ⚑ Der Plan geht in den Anker ein: Wer ihn hinterher ändert,
    /// bricht die ganze Kette. Ohne das wäre „der Plan stand vorher
    /// fest" nicht überprüfbar.
    #[test]
    fn ein_nachtraeglich_geaenderter_plan_bricht_die_kette() {
        let (a, b) = werkzeuge();
        let echt = plan_mit(&[a, b]);
        let g = vec![glied(1), glied(2)];
        let wert = kettenwert(&anker(&sitzung(), &echt.adresse()), &echt, &g).expect("gut");

        // Derselbe Ablauf, aber der Plan sieht an Stelle 1 ein anderes
        // Werkzeug vor.
        let umgeschrieben = plan_mit(&[a, a]);
        assert_ne!(echt.adresse(), umgeschrieben.adresse());
        assert!(matches!(
            pruefe_kette(&sitzung(), &umgeschrieben, &g, &wert),
            Kettenbefund::WertStimmtNicht { .. }
        ));
    }

    /// ⚑ Und die Session geht mit ein: Dieselbe Arbeit unter einem
    /// anderen Kontrakt ist eine andere Kette. Ohne das ließe sich eine
    /// Kette in eine Session mit anderen Grenzen heben.
    #[test]
    fn dieselbe_arbeit_unter_fremder_session_gilt_nicht() {
        let (a, b) = werkzeuge();
        let p = plan_mit(&[a, b]);
        let g = vec![glied(1), glied(2)];
        let wert = kettenwert(&anker(&sitzung(), &p.adresse()), &p, &g).expect("gut");

        let fremd = SitzungId::new([78u8; 32]);
        assert_ne!(anker(&fremd, &p.adresse()), anker(&sitzung(), &p.adresse()));
        assert!(matches!(
            pruefe_kette(&fremd, &p, &g, &wert),
            Kettenbefund::WertStimmtNicht { .. }
        ));
    }

    /// Der Redundanzvergleich eine Ebene höher: Wo zwei Ketten zuerst
    /// auseinandergehen, ist der Schritt, über den zu streiten ist.
    #[test]
    fn zwei_ketten_verraten_die_erste_abweichung() {
        let a = vec![glied(1), glied(2), glied(3)];
        assert_eq!(erster_unterschied(&a, &a), None);

        let b = vec![glied(1), glied(2), glied(9)];
        assert_eq!(erster_unterschied(&a, &b), Some(2));

        let c = vec![glied(9), glied(2), glied(3)];
        assert_eq!(erster_unterschied(&a, &c), Some(0));

        // Verschiedene Längen: ab der kürzeren.
        assert_eq!(erster_unterschied(&a, &a[..2]), Some(2));
        assert_eq!(erster_unterschied(&[], &[]), None);
    }

    /// ⚑ Die Abbruchbedingungen aus Kap. 8.4, und ihre Rangfolge: Ein
    /// Widerruf gilt, auch wenn die Frist noch läuft.
    #[test]
    fn die_abbruchbedingungen_greifen_in_ihrer_rangfolge() {
        let (a, b) = werkzeuge();
        let p = plan_mit(&[a, b, a]);
        let k = kontrakt(10, 1_000);
        let frisch = Sitzungszustand::neu();

        assert_eq!(ende(&k, &frisch, EpochId(5), &p, 0), Ende::Laeuft);
        assert_eq!(ende(&k, &frisch, EpochId(5), &p, 2), Ende::Laeuft);
        assert_eq!(ende(&k, &frisch, EpochId(5), &p, 3), Ende::Vollstaendig);

        // Frist vor Vollständigkeit.
        assert_eq!(ende(&k, &frisch, EpochId(101), &p, 3), Ende::Frist);
        // Widerruf vor Frist.
        let widerrufen = Sitzungszustand { widerrufen: true, ..Sitzungszustand::neu() };
        assert_eq!(ende(&k, &widerrufen, EpochId(101), &p, 3), Ende::Widerrufen);

        // Schrittzahl greift vor dem Plan-Ende, wenn sie kleiner ist.
        let eng = kontrakt(2, 1_000);
        assert_eq!(
            ende(&eng, &frisch, EpochId(5), &p, 2),
            Ende::Schrittzahl { grenze: 2 }
        );

        // Budget: beide Währungen leer.
        let leer = Sitzungszustand { verbraucht_credits: 1_000, ..Sitzungszustand::neu() };
        assert_eq!(ende(&k, &leer, EpochId(5), &p, 1), Ende::Budget);
    }

    /// ⚑ Geprüft wird vor dem ersten Schritt: Ein zu langer Plan läuft
    /// sonst zur Hälfte und bricht dann ab. Bezahlt wäre die Hälfte,
    /// erreicht nichts.
    #[test]
    fn ein_zu_langer_plan_wird_vorher_abgewiesen() {
        let (a, b) = werkzeuge();
        let p = plan_mit(&[a, b, a, b]);
        assert!(plan_passt(&kontrakt(4, 1_000), &p));
        assert!(plan_passt(&kontrakt(99, 1_000), &p));
        assert!(!plan_passt(&kontrakt(3, 1_000), &p));
        assert!(!plan_passt(&kontrakt(0, 1_000), &p));
        // Der leere Plan passt auch unter einen sperrenden Kontrakt.
        assert!(plan_passt(&kontrakt(0, 1_000), &Plan::leer()));
    }

    /// Der leere Plan hat eine leere Kette, und die ist heil.
    #[test]
    fn der_leere_plan_hat_eine_heile_leere_kette() {
        let p = Plan::leer();
        let a = anker(&sitzung(), &p.adresse());
        assert_eq!(kettenwert(&a, &p, &[]).expect("gut"), a, "nichts gefaltet, nichts geändert");
        assert_eq!(pruefe_kette(&sitzung(), &p, &[], &a), Kettenbefund::Heil);
        assert_eq!(
            ende(&kontrakt(5, 1_000), &Sitzungszustand::neu(), EpochId(1), &p, 0),
            Ende::Vollstaendig
        );
    }
}
