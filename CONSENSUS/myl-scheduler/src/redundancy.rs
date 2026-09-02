//! Redundanz-Zuteilung: zonendiverse Pods (Anhang A.2, Schritt 5; Kap. 4.4).
//!
//! Für jedes Segment werden 2 disjunkte Pods zugewiesen, nach
//! Möglichkeit aus verschiedenen Zonen, damit ein regionaler Ausfall
//! nicht beide Seiten desselben Vergleichs trifft.
//!
//! **Konsens-Feld:** Die Redundanz-Regeln sind Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).
//!
//! # ⚑ Fund 110: Die Paarung las eine Quelle, die nicht im Konsens steht
//!
//! Bis zum 2026-09-01 nahm diese Datei die Zone eines Pods aus der
//! **gegossipten** `NodeMetadata` seiner Mitglieder. Seit der
//! Entscheidung 3b steht die Zone in der **Registrierung**, also im
//! Konsenszustand, und der Pod trägt die Registrierung jedes Mitglieds
//! ohnehin bei sich. Die alte Quelle blieb stehen, und sie hatte drei
//! Löcher:
//!
//! - ⚑ **Zwei Knoten mit verschiedener Gossip-Sicht paarten
//!   verschieden.** Wer wessen Ergebnis nachrechnet, ist eine
//!   Konsensentscheidung. Sie aus einer Quelle zu treffen, die nicht
//!   Teil des Konsens ist, bricht die Gleichheit, auf der alles ruht.
//! - ⚑ **Ein einzelnes Mitglied konnte seinen Pod aus jeder Paarung
//!   nehmen**, indem es eine abweichende Region gossipte: Dann war die
//!   Zone des Pods unbestimmt, und unbestimmt schloss ihn überall aus.
//!   Genug davon, und eine ganze Epoche bekam keine Redundanz. **Genau
//!   diesen Verweigerungshebel wollte die Entscheidung 3b vermeiden**,
//!   und er saß die ganze Zeit eine Ebene tiefer.
//! - **Fehlende Metadaten wirkten wie Widerspruch.** Ein frisch
//!   gestarteter Knoten, dessen Gossip noch nicht durch war, fiel aus
//!   der Paarung, ohne etwas falsch gemacht zu haben.
//!
//! Die Zone kommt jetzt aus [`myl_types::miner::MinerRegistration`]. Sie
//! ist damit für jeden Leser dieselbe, und **kein Mitglied kann sie für
//! seinen Pod unbestimmt machen**: Pods entstehen je Zone, ihre
//! Mitglieder teilen die Zone also durch Konstruktion.
//!
//! # ⚑ Fund 108: Was diese Zonendiversität wert ist, und was nicht
//!
//! Die Zone ist **erklärt**, nicht gemessen. Wer beide Pods eines Paars
//! im selben Rechenzentrum betreibt, trägt zwei Zonen ein und besteht
//! diese Prüfung.
//!
//! ⚑ **Deshalb trägt sie die Ausfalldiversität und nicht die
//! Sicherheit.** Hier stand bis zum 2026-09-01, die Prüfung schütze eine
//! Ebene gröber vor derselben Selbstbestätigung wie
//! [`pods_are_disjoint`]. **Das kann sie nicht**, und der Anspruch ist
//! zurückgezogen:
//!
//! - Für die **Ausfalldiversität** trägt eine Erklärung: Der ehrliche
//!   Betreiber sagt die Wahrheit, und wer lügt, verliert seine eigene
//!   Absicherung und nicht die des Netzes.
//! - Gegen einen **kolludierenden Betreiber** trägt sie nichts, und sie
//!   kann es nicht, denn zwei Zonen anzugeben kostet ihn nichts. Was
//!   dort trägt, ist sein Anteil an den Pods, die Unvorhersehbarkeit der
//!   Paarung und die Stichprobe der Checker darüber. **Das sind Zahlen**,
//!   und sie gehören in eine Simulation, nicht in eine Bedingung hier.
//!
//! # ⚑ Die Bedingung hat einen eigenen Preis: Diversität verengt
//!
//! Gibt es drei zonendiverse Paare und hundert Segmente, so rotiert die
//! Zuteilung über diese drei, und **wer eines davon hält, rechnet ein
//! Drittel der Arbeit nach**. Ohne die Bedingung wären es fünfzig Paare
//! und ein Fünfzigstel. Die Diversität kauft Ausfallsicherheit und
//! bezahlt mit Streuung.
//!
//! ⚑ **Gerechnet am 2026-09-02** (`security_sim.py`, Abschnitt 9), und
//! das Ergebnis ist keine Schwelle in Segmenten, sondern eine Bedingung
//! an die Verteilung:
//!
//! **Die Verengung beträgt `(km−1)/((k−1)m)` bei `k` gleich besetzten
//! Zonen mit je `m` Pods, also rund `k/(k−1)`.**
//!
//! | Zonen | Verengung |
//! |---|---|
//! | 2 | **1,95** |
//! | 3 | 1,48 |
//! | 4 | 1,32 |
//! | 10 | 1,11 |
//!
//! Ungleichverteilung kommt oben drauf: Bei zwei Zonen und 100 Pods
//! kostet der halbe Schnitt Faktor 1,98, ein Schnitt von 90 zu 10 aber
//! **5,50**.
//!
//! ⚑ **Die Größe, an der es hängt, ist die Zahl der Zonen und nicht der
//! Anteil der größten.** Wer die Verengung unter zwei bringen will,
//! braucht eine **dritte** Zone; eine gleichmäßigere Verteilung auf zwei
//! reicht nicht, denn der günstigste Fall bei zwei Zonen kostet bereits
//! Faktor zwei.
//!
//! **Was die Rechnung nicht beantwortet:** ob Ausfallsicherheit diesen
//! Preis wert ist. Das hängt an der Wahrscheinlichkeit eines regionalen
//! Ausfalls, und dafür gibt es keine Messung. **Die Abwägung bleibt
//! deshalb eine Entscheidung**, nur jetzt eine mit Zahlen dahinter.

use myl_types::seed_rng::deterministic_shuffle;
use std::collections::HashSet;

use myl_types::ids::MinerId;
use myl_types::node_metadata::GeoRegion;

use crate::shard_assignment::Pod;

/// Zuweisung eines Segments zu 2 redundanten Pods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentAssignment {
    /// Segment-Index (0-basiert).
    pub segment_index: u32,
    /// Primärer Pod (Index in der Pod-Liste).
    pub primary_pod_index: u32,
    /// Redundanter Pod (Index in der Pod-Liste).
    pub redundant_pod_index: u32,
}

/// Die Zone eines Pods, aus dem Konsenszustand.
///
/// # ⚑ Fund 110: die Quelle war die falsche
///
/// Hier stand bis zum 2026-09-01 die Region aus der **gegossipten**
/// [`myl_types::node_metadata::NodeMetadata`] der Mitglieder. Sie kommt
/// jetzt aus deren **Registrierung**, also aus der Anmeldung in der
/// Kette. Der Unterschied ist keiner des Feldes, sondern einer der
/// Quelle: **Die Registrierung ist bei jedem Leser dieselbe, das Gossip
/// nicht.**
///
/// **Uneinigkeit ist unter der heutigen Pod-Bildung unmöglich**, denn
/// [`crate::zonenzuteilung::zonen_cluster`] bildet je Zone ein Cluster
/// und [`crate::shard_assignment::assign_pods`] teilt ein Cluster in
/// Pods. `None` bleibt für zwei Fälle stehen, die beide beim Aufrufer
/// liegen und nicht beim Miner: ein **leerer** Pod und ein von Hand
/// zusammengesetzter.
///
/// ⛑ **Und hier ist genau zu sein, statt sich besser zu machen:** Ein
/// unbestimmter Pod gilt in keinem Paar als divers, wird also verwendet,
/// **sobald es überhaupt kein diverses Paar gibt**, und sonst nicht. In
/// einem Netz mit Diversität ist er damit faktisch draußen, so wie
/// vorher. **Tragbar ist das allein deshalb, weil kein Miner
/// „unbestimmt" mehr herstellen kann**; vorher genügte dafür eine
/// abweichende Zeile im Gossip. Der Hebel ist an seiner Quelle
/// verschwunden, nicht an seiner Wirkung.
fn pod_zone(pod: &Pod) -> Option<GeoRegion> {
    let mut bekannt: Option<GeoRegion> = None;
    for m in pod.mitglieder() {
        match bekannt {
            None => bekannt = Some(m.zone),
            Some(z) if z == m.zone => {}
            // Uneinig: Die Ausfallzone ist unbestimmt.
            Some(_) => return None,
        }
    }
    bekannt
}

/// Prüft, ob zwei Pods disjunkt sind (keine gemeinsamen Mitglieder).
///
/// ⚑ **Die Reserve zählt mit, seit es sie gibt (2026-08-26).** Bis zur
/// Entscheidung D3 hatte ein Pod keine getrennte Reserve, und diese
/// Prüfung sah zwangsläufig alle Mitglieder. Seither wäre eine Prüfung,
/// die nur die Shard-Positionen vergleicht, unvollständig:
///
/// **Stünde dieselbe Maschine in der Reserve beider Pods eines
/// Redundanzpaars**, übernähme sie bei einem Ausfall auf **beiden**
/// Seiten. Der Redundanzvergleich verglände dann zwei Ergebnisse
/// derselben Maschine, und Stufe 1 der Verifikation wäre eine
/// Selbstbestätigung. Genau davor soll die Disjunktheit schützen.
fn pods_are_disjoint(pod_a: &Pod, pod_b: &Pod) -> bool {
    let miners_a: HashSet<MinerId> = pod_a.mitglieder().map(|m| m.miner_id).collect();
    let miners_b: HashSet<MinerId> = pod_b.mitglieder().map(|m| m.miner_id).collect();
    miners_a.is_disjoint(&miners_b)
}

/// Warum eine Redundanzzuteilung nicht zustande kam.
///
/// **Vor dieser Aufteilung bekam der Aufrufer eine leere Liste** und
/// konnte nicht unterscheiden, woran es lag. Die beiden Fälle sind aber
/// verschiedene Befunde, und die Gegenmaßnahmen sind es auch. Ein
/// Ergebnis, das seinen Grund verschweigt, zwingt jeden Aufrufer, ihn
/// selbst zu raten.
///
/// ⚑ **Fehlende Zonendiversität steht seit dem 2026-09-01 nicht mehr
/// hier**, sondern in [`Redundanzzuteilung::zonendivers`]. Sie war ein
/// Hindernis, solange sie eine Bedingung war; als Bedingung konnte ein
/// einzelnes Mitglied eine ganze Epoche ohne Redundanz lassen (Fund
/// 110), und ein Netz, das in einer Zone anfängt, hätte nie eine
/// bekommen. **Keine Redundanz ist schlechter als Redundanz in einer
/// Zone**, denn ohne Paar entfällt Stufe 1 der Verifikation ganz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZuweisungsHindernis {
    /// Weniger als zwei Pods vorhanden: Es gibt nichts, woraus ein Paar
    /// werden könnte.
    ZuWenigPods { pods: usize },
    /// Pods sind vorhanden, aber **kein einziges Paar ist disjunkt**:
    /// Je zwei von ihnen teilen mindestens eine Maschine. Das ist eine
    /// Aussage über den Aufbau der Pods und keine über Angaben ihrer
    /// Mitglieder.
    KeinGueltigesPaar { pods: usize },
}

/// Das Ergebnis einer Redundanzzuteilung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redundanzzuteilung {
    /// Eine Zuweisung je Segment, in Segmentreihenfolge.
    pub zuweisungen: Vec<SegmentAssignment>,
    /// ⚑ **Ob die Paare aus verschiedenen Zonen kommen.**
    ///
    /// `false` heißt: Es gab kein einziges zonendiverses Paar, und die
    /// Zuteilung ist auf disjunkte Paare **derselben** Zone ausgewichen.
    /// Der Vergleich läuft dann weiter, aber ein regionaler Ausfall
    /// trifft beide Seiten.
    ///
    /// **Das steht im Ergebnis und nicht in einem Protokoll**, aus
    /// demselben Grund, aus dem die Speicherzuteilung ihre
    /// Unterbesetzung mitgibt: Eine Zuteilung, die etwas nicht hält und
    /// trotzdem aussieht wie eine, die es hält, ist eine Falle für jeden
    /// Aufrufer.
    pub zonendivers: bool,
}

/// Weist Segmente redundanten Pods zu.
///
/// **Algorithmus (Anhang A.2, Schritt 5):**
/// 1. Alle **disjunkten** Paare bilden, getrennt nach zonendivers und
///    nicht zonendivers
/// 2. Gibt es zonendiverse Paare, wird **nur** aus ihnen gewählt; sonst
///    aus den übrigen, und [`Redundanzzuteilung::zonendivers`] sagt es
/// 3. Innerhalb der gewählten Menge entscheidet der Seed (Fisher-Yates)
///
/// # ⚑ Ganz oder gar nicht, und warum nicht je Segment
///
/// Die Wahl zwischen den beiden Mengen fällt **einmal für die ganze
/// Zuteilung**, nicht Segment für Segment. Mischte man sie, käme jedes
/// Segment zuerst an die diversen Paare, und ein Angreifer, der zwei
/// Zonen angibt, säße damit **bevorzugt** in jedem Vergleich. So gibt es
/// die Bevorzugung nur, wenn das Netz überhaupt Diversität hat, und
/// dann ist sie die Regel und keine Auszeichnung.
///
/// **Determinismus:** Gleicher Seed + gleiche Pods → gleiche Zuteilung.
/// Die Zone kommt aus der Registrierung, also aus dem Konsenszustand;
/// **es gibt keine Eingabe mehr, die zwischen zwei Knoten verschieden
/// sein kann** (Fund 110).
///
/// **Parameter:**
/// - `num_segments`: Anzahl der Segmente, die zugewiesen werden sollen
/// - `pods`: Liste aller verfügbaren Pods (aus Phase 2.4)
/// - `seed`: Epochenseed (aus Phase 2.1) für die deterministische Auswahl
///
/// **Returns:** `Ok` mit einer Zuweisung pro Segment; bei null Segmenten
/// eine leere Liste, das ist kein Fehler. `Err` nur, wenn es gar kein
/// disjunktes Paar gibt; ein teilweises Ergebnis gibt es nicht, denn ein
/// Segment ohne Redundanzpartner wäre ein Versprechen, das der Vertrag
/// nicht hält.
pub fn assign_redundant_pods(
    num_segments: u32,
    pods: &[Pod],
    seed: &[u8; 32],
) -> Result<Redundanzzuteilung, ZuweisungsHindernis> {
    // Null Segmente brauchen keine Paare: Wer nichts verlangt, bekommt
    // nichts, und das ist kein Scheitern. Vor dieser Prüfung stünde hier
    // ein Hindernis über fehlende Paare, das niemanden interessiert.
    if num_segments == 0 {
        return Ok(Redundanzzuteilung { zuweisungen: vec![], zonendivers: true });
    }
    if pods.len() < 2 {
        return Err(ZuweisungsHindernis::ZuWenigPods { pods: pods.len() });
    }

    let mut divers: Vec<(u32, u32)> = vec![];
    let mut gleiche_zone: Vec<(u32, u32)> = vec![];

    for i in 0..pods.len() {
        for j in (i + 1)..pods.len() {
            if !pods_are_disjoint(&pods[i], &pods[j]) {
                continue;
            }
            // Ein Paar gilt als divers, wenn **beide** Zonen bestimmt
            // sind und sich unterscheiden. Ein unbestimmter Pod landet
            // damit in der zweiten Menge und nicht im Aus.
            match (pod_zone(&pods[i]), pod_zone(&pods[j])) {
                (Some(a), Some(b)) if a != b => divers.push((i as u32, j as u32)),
                _ => gleiche_zone.push((i as u32, j as u32)),
            }
        }
    }

    let (mut paare, zonendivers) = if !divers.is_empty() {
        (divers, true)
    } else if !gleiche_zone.is_empty() {
        (gleiche_zone, false)
    } else {
        return Err(ZuweisungsHindernis::KeinGueltigesPaar { pods: pods.len() });
    };

    deterministic_shuffle(&mut paare, seed);

    // Weise Segmente zu (rotierend über die Paare)
    let mut zuweisungen = Vec::with_capacity(num_segments as usize);
    for seg_idx in 0..num_segments {
        let pair_idx = (seg_idx as usize) % paare.len();
        let (primary, redundant) = paare[pair_idx];

        zuweisungen.push(SegmentAssignment {
            segment_index: seg_idx,
            primary_pod_index: primary,
            redundant_pod_index: redundant,
        });
    }

    Ok(Redundanzzuteilung { zuweisungen, zonendivers })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::miner_filter::{HardwareClass, MinerRegistration};
    use crate::shard_assignment::Shard;

    fn registrierung(miner_byte: u8, zone: GeoRegion) -> MinerRegistration {
        MinerRegistration {
            miner_id: MinerId::new([miner_byte; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: 5,
            zone,
            schluessel: myl_types::bls::BlsPublicKey([0; 48]),
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        }
    }

    /// Ein Pod mit je einem Miner auf jeder Position, ohne Reserve, alle
    /// in derselben Zone. So entstehen Pods auch im Netz: je Zone ein
    /// Cluster.
    fn test_pod(pod_index: u32, miner_bytes: &[u8], zone: GeoRegion) -> Pod {
        Pod {
            pod_index,
            shards: miner_bytes
                .iter()
                .enumerate()
                .map(|(i, &b)| Shard {
                    shard_index: i as u32,
                    miner: registrierung(b, zone),
                })
                .collect(),
            reserve: vec![],
        }
    }

    /// Ein Pod mit Positionen **und** Reserve.
    fn test_pod_mit_reserve(
        pod_index: u32,
        positionen: &[u8],
        reserve: &[u8],
        zone: GeoRegion,
    ) -> Pod {
        let mut pod = test_pod(pod_index, positionen, zone);
        pod.reserve = reserve.iter().map(|&b| registrierung(b, zone)).collect();
        pod
    }

    // -----------------------------------------------------------------
    // Disjunktheit
    // -----------------------------------------------------------------

    #[test]
    fn pods_are_disjoint_true() {
        let pod_a = test_pod(0, &[1, 2], GeoRegion::Europe);
        let pod_b = test_pod(1, &[3, 4], GeoRegion::Europe);
        assert!(pods_are_disjoint(&pod_a, &pod_b));
    }

    #[test]
    fn pods_are_disjoint_false() {
        let pod_a = test_pod(0, &[1, 2], GeoRegion::Europe);
        let pod_b = test_pod(1, &[2, 3], GeoRegion::Europe); // Miner 2 ist in beiden
        assert!(!pods_are_disjoint(&pod_a, &pod_b));
    }

    /// ⚑ **Eine geteilte Reserve macht zwei Pods nicht disjunkt.**
    ///
    /// Stünde dieselbe Maschine in der Reserve beider Pods eines
    /// Redundanzpaars, übernähme sie bei einem Ausfall auf **beiden**
    /// Seiten. Der Redundanzvergleich verglände dann zwei Ergebnisse
    /// derselben Maschine, und Stufe 1 der Verifikation wäre eine
    /// Selbstbestätigung.
    ///
    /// Diese Prüfung konnte es vor der Entscheidung D3 nicht geben: Ein
    /// Pod hatte keine getrennte Reserve.
    #[test]
    fn eine_geteilte_reserve_bricht_die_disjunktheit() {
        let pod_a = test_pod_mit_reserve(0, &[1, 2], &[9, 10], GeoRegion::Europe);
        let pod_b = test_pod_mit_reserve(1, &[3, 4], &[9, 11], GeoRegion::Asia);
        assert!(
            !pods_are_disjoint(&pod_a, &pod_b),
            "zwei Pods mit gemeinsamem Reservemitglied galten als disjunkt"
        );
        // Gegenprobe: getrennte Reserven sind disjunkt.
        let pod_c = test_pod_mit_reserve(2, &[5, 6], &[12, 13], GeoRegion::Asia);
        assert!(pods_are_disjoint(&pod_a, &pod_c));
    }

    /// Und die andere Richtung: ein Positionsminer des einen darf nicht
    /// in der Reserve des anderen stehen.
    #[test]
    fn ein_positionsminer_in_fremder_reserve_bricht_die_disjunktheit() {
        let pod_a = test_pod_mit_reserve(0, &[1, 2], &[9, 10], GeoRegion::Europe);
        let pod_b = test_pod_mit_reserve(1, &[3, 4], &[1, 11], GeoRegion::Asia);
        assert!(!pods_are_disjoint(&pod_a, &pod_b));
    }

    // -----------------------------------------------------------------
    // Die Zone eines Pods
    // -----------------------------------------------------------------

    /// Ein Pod, dessen Mitglieder dieselbe Zone angemeldet haben, hat
    /// sie. Das ist der Normalfall, denn Cluster entstehen je Zone.
    #[test]
    fn ein_einiger_pod_hat_seine_zone() {
        let pod = test_pod_mit_reserve(0, &[1, 2], &[3], GeoRegion::Asia);
        assert_eq!(pod_zone(&pod), Some(GeoRegion::Asia));
    }

    /// ⚑ **Ein von Hand zusammengesetzter Pod kann uneinig sein**, und
    /// dann ist seine Ausfallzone unbestimmt. Im Netz kann das nicht
    /// entstehen; die Prüfung steht für den Aufrufer, nicht gegen einen
    /// Miner.
    #[test]
    fn ein_uneiniger_pod_hat_keine_zone() {
        let mut pod = test_pod(0, &[1, 2], GeoRegion::Europe);
        pod.shards[1].miner.zone = GeoRegion::Asia;
        assert_eq!(pod_zone(&pod), None);
    }

    /// Auch die Reserve zählt: Sie rechnet mit, sobald jemand ausfällt.
    #[test]
    fn eine_reserve_aus_fremder_zone_macht_den_pod_uneinig() {
        let mut pod = test_pod_mit_reserve(0, &[1, 2], &[3], GeoRegion::Europe);
        pod.reserve[0].zone = GeoRegion::SouthAmerica;
        assert_eq!(pod_zone(&pod), None);
    }

    /// Ein leerer Pod hat keine Zone, und das ist keine Uneinigkeit,
    /// sondern das Fehlen jeder Angabe.
    #[test]
    fn ein_leerer_pod_hat_keine_zone() {
        let pod = test_pod(0, &[], GeoRegion::Europe);
        assert_eq!(pod_zone(&pod), None);
    }

    // -----------------------------------------------------------------
    // Die Zuteilung
    // -----------------------------------------------------------------

    #[test]
    fn zwei_zonendiverse_pods_bilden_ein_paar() {
        let pods = vec![
            test_pod(0, &[1, 2], GeoRegion::Europe),
            test_pod(1, &[3, 4], GeoRegion::NorthAmerica),
        ];
        let z = assign_redundant_pods(2, &pods, &[0u8; 32])
            .expect("zwei zonendiverse Pods bilden ein Paar");

        assert!(z.zonendivers);
        assert_eq!(z.zuweisungen.len(), 2);
        // Nur ein Paar ist möglich, also bekommen beide Segmente dasselbe.
        assert_eq!(
            z.zuweisungen[0].primary_pod_index,
            z.zuweisungen[1].primary_pod_index
        );
        assert_eq!(
            z.zuweisungen[0].redundant_pod_index,
            z.zuweisungen[1].redundant_pod_index
        );
    }

    /// ⚑ **Fund 110, die Hälfte über den Anfang: Ein Netz in einer Zone
    /// bekommt Redundanz, und das Ergebnis sagt, dass sie nicht divers
    /// ist.**
    ///
    /// Vorher gab es hier ein Hindernis und damit **gar kein Paar**.
    /// Keine Redundanz ist schlechter als Redundanz in einer Zone: Ohne
    /// Paar entfällt Stufe 1 der Verifikation ganz, und ein Netz, das in
    /// einer Zone anfängt, käme nie in Gang.
    #[test]
    fn ohne_diverse_paare_wird_ausgewichen_und_es_steht_im_ergebnis() {
        let pods = vec![
            test_pod(0, &[1, 2], GeoRegion::Europe),
            test_pod(1, &[3, 4], GeoRegion::Europe),
        ];
        let z = assign_redundant_pods(2, &pods, &[0u8; 32])
            .expect("zwei disjunkte Pods bilden ein Paar, auch in einer Zone");

        assert!(
            !z.zonendivers,
            "die fehlende Zonendiversität muss im Ergebnis stehen"
        );
        assert_eq!(z.zuweisungen.len(), 2);
    }

    /// ⚑ **Der Kern von Fund 110: Kein Mitglied kann die Zone seines
    /// Pods unbestimmt machen.**
    ///
    /// Solange die Zone aus gegossipten Metadaten kam, machte ein
    /// einzelnes abweichendes Mitglied die Zone seines Pods unbestimmt,
    /// und unbestimmt hieß: aus jedem Paar heraus. Genug davon, und eine
    /// ganze Epoche bekam keine Redundanz.
    ///
    /// Der Schutz sitzt **nicht** in dieser Datei, sondern in der
    /// Pod-Bildung: Cluster entstehen je Zone, ein Pod entsteht in einem
    /// Cluster, also teilen seine Mitglieder die Zone durch
    /// Konstruktion. Deshalb prüft dieser Test die **echte Zuteilung**
    /// und nicht von Hand gebaute Pods.
    ///
    /// ⛑ **Hier stand zuerst ein Test, der prüfte, dass die einigen
    /// Pods weiter gepaart werden.** Der blieb grün, als der Ausschluss
    /// unbestimmter Pods versuchsweise wiederhergestellt wurde: Er
    /// prüfte eine Aussage, die unter beiden Fassungen gilt.
    #[test]
    fn kein_mitglied_kann_die_zone_seines_pods_unbestimmt_machen() {
        use crate::zonenzuteilung::zuteilung_der_epoche;

        // Miner über drei Zonen, genug für mehrere volle Pods je Zone.
        let zonen = [GeoRegion::Europe, GeoRegion::NorthAmerica, GeoRegion::Asia];
        let register: Vec<MinerRegistration> = (0u16..60)
            .map(|i| {
                let mut b = [0u8; 32];
                b[..2].copy_from_slice(&i.to_le_bytes());
                let mut r = registrierung(0, zonen[(i as usize) % zonen.len()]);
                r.miner_id = MinerId::new(b);
                r.registration_epoch = 0;
                r
            })
            .collect();

        let zuteilung =
            zuteilung_der_epoche(&register, 5, &myl_types::hash::Hash([3u8; 32]), 4);
        assert!(
            !zuteilung.pods.is_empty(),
            "sechzig Miner über drei Zonen müssen Pods ergeben"
        );
        for pod in &zuteilung.pods {
            assert!(
                pod_zone(pod).is_some(),
                "Pod {} hatte keine bestimmte Zone",
                pod.pod_index
            );
        }

        // Und die Paarung findet Diversität, weil die Pods über drei
        // Zonen entstehen.
        let z = assign_redundant_pods(12, &zuteilung.pods, &[7u8; 32])
            .expect("Pods aus drei Zonen bilden Paare");
        assert!(z.zonendivers);
    }

    /// ⚑ **Und die Gegenprobe zur Gegenprobe:** Wenn *alle* Pods uneinig
    /// sind, gibt es keine Diversität, aber immer noch Redundanz.
    #[test]
    fn lauter_uneinige_pods_bekommen_trotzdem_paare() {
        let mut pods = vec![
            test_pod(0, &[1, 2], GeoRegion::Europe),
            test_pod(1, &[3, 4], GeoRegion::NorthAmerica),
        ];
        pods[0].shards[1].miner.zone = GeoRegion::Asia;
        pods[1].shards[1].miner.zone = GeoRegion::Africa;

        let z = assign_redundant_pods(3, &pods, &[1u8; 32])
            .expect("unbestimmte Zonen schließen niemanden aus");
        assert!(!z.zonendivers);
        assert_eq!(z.zuweisungen.len(), 3);
    }

    /// Zwei Pods mit gemeinsamem Miner bilden kein Paar, auch nicht als
    /// Rückfall: Die Disjunktheit ist keine Vorliebe, sondern die
    /// Bedingung, ohne die der Vergleich nichts vergleicht.
    #[test]
    fn ueberlappende_pods_bilden_kein_paar() {
        let pods = vec![
            test_pod(0, &[1, 2], GeoRegion::Europe),
            test_pod(1, &[2, 3], GeoRegion::NorthAmerica), // Miner 2 in beiden
        ];
        let hindernis = assign_redundant_pods(2, &pods, &[0u8; 32]).unwrap_err();
        assert_eq!(hindernis, ZuweisungsHindernis::KeinGueltigesPaar { pods: 2 });
    }

    /// Die beiden Hindernisse sind unterscheidbar, und beide sind
    /// Aussagen über den **Aufbau** der Pods.
    #[test]
    fn die_beiden_hindernisse_sind_unterscheidbar() {
        let einer = vec![test_pod(0, &[1, 2], GeoRegion::Europe)];
        assert_eq!(
            assign_redundant_pods(1, &einer, &[0u8; 32]).unwrap_err(),
            ZuweisungsHindernis::ZuWenigPods { pods: 1 }
        );

        let ueberlappend = vec![
            test_pod(0, &[1, 2], GeoRegion::Europe),
            test_pod(1, &[1, 3], GeoRegion::Asia),
        ];
        assert_eq!(
            assign_redundant_pods(1, &ueberlappend, &[0u8; 32]).unwrap_err(),
            ZuweisungsHindernis::KeinGueltigesPaar { pods: 2 }
        );
    }

    #[test]
    fn dieselbe_eingabe_ergibt_dieselbe_zuteilung() {
        let pods = vec![
            test_pod(0, &[1, 2], GeoRegion::Europe),
            test_pod(1, &[3, 4], GeoRegion::NorthAmerica),
            test_pod(2, &[5, 6], GeoRegion::Asia),
            test_pod(3, &[7, 8], GeoRegion::SouthAmerica),
        ];
        let a = assign_redundant_pods(10, &pods, &[42u8; 32]).unwrap();
        let b = assign_redundant_pods(10, &pods, &[42u8; 32]).unwrap();
        assert_eq!(a, b);

        // Und ein anderer Seed ergibt eine andere Zuteilung, sonst wäre
        // der Seed wirkungslos.
        let c = assign_redundant_pods(10, &pods, &[43u8; 32]).unwrap();
        assert_ne!(a, c);
    }

    /// ⚑ **Die Zone kommt aus der Registrierung, und sie wirkt.**
    ///
    /// Ohne diesen Test bliebe unbemerkt, wenn die Paarung die Zone
    /// wieder aus einer anderen Quelle nähme: Dieselben Pods, nur mit
    /// geänderter Zone in der Anmeldung, müssen zu einer anderen
    /// Diversitätsaussage führen.
    #[test]
    fn die_zone_der_registrierung_entscheidet() {
        let mut pods = vec![
            test_pod(0, &[1, 2], GeoRegion::Europe),
            test_pod(1, &[3, 4], GeoRegion::NorthAmerica),
        ];
        assert!(assign_redundant_pods(1, &pods, &[0u8; 32]).unwrap().zonendivers);

        for s in &mut pods[1].shards {
            s.miner.zone = GeoRegion::Europe;
        }
        assert!(!assign_redundant_pods(1, &pods, &[0u8; 32]).unwrap().zonendivers);
    }

    #[test]
    fn zu_wenige_pods() {
        let pods = vec![test_pod(0, &[1, 2], GeoRegion::Europe)];
        assert_eq!(
            assign_redundant_pods(1, &pods, &[0u8; 32]).unwrap_err(),
            ZuweisungsHindernis::ZuWenigPods { pods: 1 }
        );
    }

    /// Null Segmente sind kein Scheitern: Wer nichts verlangt, bekommt
    /// nichts, und zwar auch dann, wenn gar keine Pods da sind.
    #[test]
    fn null_segmente_sind_kein_hindernis() {
        let z = assign_redundant_pods(0, &[], &[0u8; 32]).unwrap();
        assert!(z.zuweisungen.is_empty());
        assert!(z.zonendivers);
    }

    /// Die Segmentindizes laufen lückenlos von null aufwärts, und jedes
    /// Paar ist disjunkt.
    #[test]
    fn segmentindizes_sind_lueckenlos_und_paare_disjunkt() {
        let pods = vec![
            test_pod(0, &[1, 2], GeoRegion::Europe),
            test_pod(1, &[3, 4], GeoRegion::NorthAmerica),
            test_pod(2, &[5, 6], GeoRegion::Asia),
        ];
        let z = assign_redundant_pods(7, &pods, &[9u8; 32]).unwrap();
        for (i, a) in z.zuweisungen.iter().enumerate() {
            assert_eq!(a.segment_index, i as u32);
            assert_ne!(a.primary_pod_index, a.redundant_pod_index);
        }
    }
}
