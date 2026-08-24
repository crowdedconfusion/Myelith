//! Die Sicherheitsargumente des Papiers gegen die Implementierung
//! (Punkte 4.1 und 4.2, Whitepaper Anhang B.2 und Kap. 6.8).
//!
//! **Was diese Datei nicht ist:** eine Nachrechnung der Formeln. `β^{2k}`
//! in einem Test noch einmal auszurechnen belegt nichts außer der
//! Rechenfähigkeit des Testrahmens.
//!
//! **Was sie ist:** eine Messung an den **echten Zuteilungsfunktionen**.
//! Die Formeln des Papiers unterstellen unabhängige, gleichverteilte
//! Ziehungen. Die Implementierung zieht nicht so: Pods entstehen aus
//! Geo-Clustern, und die Redundanzpaarung verlangt disjunkte und
//! zonendiverse Pods. Ob unter diesen Bedingungen dieselbe
//! Wahrscheinlichkeit herauskommt, ist eine offene Frage des Papiers
//! selbst — Anhang B.2 nennt sie und verschiebt sie auf Meilenstein M1.
//!
//! Hier wird sie gemessen.

use myl_scheduler::redundancy::assign_redundant_pods;
use myl_scheduler::sampling::sample_segments;
use myl_scheduler::shard_assignment::{assign_shards, Pod};
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, MinerId, SegmentId};
use myl_verifier::{einschleusungsplan, KontrollsegmentVorrat, Kontrollsegment};

/// SplitMix64, reproduzierbar.
struct Rng(u64);
impl Rng {
    fn neu(k: u64) -> Self {
        Self(k)
    }
    fn u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn seed(n: u64) -> [u8; 32] {
    let mut s = [0u8; 32];
    s[..8].copy_from_slice(&n.to_le_bytes());
    s
}

fn segment(n: u64) -> SegmentId {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&n.to_le_bytes());
    SegmentId::new(b)
}

// ---------------------------------------------------------------------
// 4.2 (Teil 1): Die Kontrollsegment-Rate
// ---------------------------------------------------------------------

/// **Die Einschleusung hält γ ein**, über viele Ströme gemittelt.
///
/// Das ist die Voraussetzung dafür, dass „Entdeckungsrisiko γ" aus
/// Kap. 6.7 überhaupt eine Aussage ist. Hält der Plan die Rate nicht ein,
/// ist die Zahl im Papier eine Behauptung über etwas, das nicht
/// stattfindet.
#[test]
fn die_einschleusung_haelt_die_rate_ein() {
    for (gz, gn) in [(2u64, 100u64), (1, 100), (3, 100), (10, 100)] {
        let mut gesamt = 0usize;
        let mut auftraege = 0usize;
        for lauf in 0..200u64 {
            let n = 500;
            let plan = einschleusungsplan(n, gz, gn, &seed(lauf)).expect("Plan");
            gesamt += plan.len();
            auftraege += n;
            // Aufsteigend und ohne Doppelung.
            let mut sortiert = plan.clone();
            sortiert.sort_unstable();
            sortiert.dedup();
            assert_eq!(sortiert, plan, "Plan muss aufsteigend und doppelungsfrei sein");
            assert!(plan.iter().all(|&i| i < n));
        }
        let ist_bp = (gesamt as f64 / auftraege as f64 * 10_000.0).round() as u64;
        let soll_bp = gz * 10_000 / gn;
        assert!(
            ist_bp.abs_diff(soll_bp) <= 20,
            "γ = {gz}/{gn}: gemessen {ist_bp} bp statt {soll_bp} bp"
        );
    }
}

/// **Die Einschleusung ist ohne den Seed nicht vorhersehbar.**
///
/// Zwei Seeds müssen zu deutlich verschiedenen Plänen führen. Wäre der
/// Plan seedunabhängig, kennte ihn jeder Miner, und der ganze Mechanismus
/// wäre wirkungslos.
#[test]
fn ohne_den_seed_ist_der_plan_nicht_vorhersehbar() {
    let n = 1_000;
    let a = einschleusungsplan(n, 2, 100, &seed(1)).unwrap();
    let b = einschleusungsplan(n, 2, 100, &seed(2)).unwrap();
    let gemeinsam = a.iter().filter(|i| b.contains(i)).count();
    // Bei 20 aus 1000 wären zufällig ~0,4 gemeinsam; alles unter der
    // Hälfte ist deutlich verschieden.
    assert!(
        gemeinsam * 2 < a.len(),
        "zwei Seeds ergaben {gemeinsam} von {} gemeinsamen Positionen",
        a.len()
    );
    // Und derselbe Seed ergibt denselben Plan — sonst wäre nichts prüfbar.
    assert_eq!(a, einschleusungsplan(n, 2, 100, &seed(1)).unwrap());
}

// ---------------------------------------------------------------------
// 4.1: Kollusionswahrscheinlichkeit gegen Anhang B.2
// ---------------------------------------------------------------------

/// Baut `anzahl` Pods aus je `k` Shards mit je einem Miner.
fn pods_bauen(anzahl: u32, k: usize, s: &[u8; 32]) -> Vec<Pod> {
    use myl_scheduler::geo_clustering::MinerCluster;
    use myl_scheduler::miner_filter::{HardwareClass, MinerRegistration};
    (0..anzahl)
        .map(|p| {
            let miners: Vec<MinerRegistration> = (0..k)
                .map(|i| {
                    let id = (p as u64) * 1000 + i as u64;
                    let mut b = [0u8; 32];
                    b[..8].copy_from_slice(&id.to_le_bytes());
                    MinerRegistration {
                        miner_id: MinerId::new(b),
                        hardware_class: HardwareClass::MediumGpu,
                        registration_epoch: 0,
                    }
                })
                .collect();
            let cluster = MinerCluster {
                miners,
                max_internal_latency: 10,
            };
            assign_shards(&cluster, k as u32, p, s)
        })
        .collect()
}

/// Metadaten für `anzahl` Pods, Regionen rotierend über drei.
///
/// **Ohne Metadaten weist `assign_redundant_pods` gar nichts zu**, weil
/// die Zonendiversität mangels Region nicht feststellbar ist und das
/// Paar dann übersprungen wird. Das ist fail-closed und damit die
/// richtige Richtung, aber es ist **still**: Der Rückgabewert ist ein
/// leerer Vektor, und der sieht genauso aus wie „keine Segmente
/// angefragt". Vermerkt im Fahrplan.
fn metadaten(
    pods: &[Pod],
) -> std::collections::HashMap<MinerId, myl_types::node_metadata::NodeMetadata> {
    use myl_types::node_metadata::{Asn, GeoRegion, NodeMetadata};
    let regionen = [GeoRegion::Europe, GeoRegion::NorthAmerica, GeoRegion::Asia];
    let mut m = std::collections::HashMap::new();
    for p in pods {
        let region = regionen[(p.pod_index as usize) % regionen.len()];
        for sh in &p.shards {
            for miner in &sh.miners {
                m.insert(
                    miner.miner_id,
                    NodeMetadata {
                        miner: miner.miner_id,
                        region,
                        asn: Asn(1000 + p.pod_index),
                        timestamp_ms: 1,
                    },
                );
            }
        }
    }
    m
}

/// **Anhang B.2 gegen die echte Zuteilung.**
///
/// Das Papier sagt `P_koll ≈ β^{2k}` und unterstellt damit, dass die
/// `2k` Positionen eines Segments **unabhängig** aus der Miner-Menge
/// gezogen werden. Die Implementierung zieht anders: `assign_shards`
/// verteilt die Miner eines Clusters per Fisher-Yates auf Shards, und
/// `assign_redundant_pods` wählt aus den **gültigen Paaren** (disjunkt,
/// zonendivers).
///
/// Gemessen wird die tatsächliche Rate über viele Zuteilungen, mit
/// zufällig als kolludierend markierten Minern.
#[test]
fn kollusionsrate_gegen_anhang_b2() {
    let k = 4usize;
    let pods = 8u32;
    let segmente = 200u32;

    for beta_prozent in [20u64, 50] {
        let mut kolludiert = 0usize;
        let mut gesamt = 0usize;
        let mut rng = Rng::neu(0xB2 + beta_prozent);

        for lauf in 0..50u64 {
            let s = seed(lauf);
            let alle_pods = pods_bauen(pods, k, &s);
            // Miner zufällig als kolludierend markieren.
            let boese: std::collections::BTreeSet<MinerId> = alle_pods
                .iter()
                .flat_map(|p| p.shards.iter())
                .flat_map(|sh| sh.miners.iter())
                .filter(|_| rng.u64() % 100 < beta_prozent)
                .map(|m| m.miner_id)
                .collect();

            let metadata = metadaten(&alle_pods);
            let zuteilungen = assign_redundant_pods(segmente, &alle_pods, &metadata, &s);
            for z in &zuteilungen {
                let beide_boese = [z.primary_pod_index, z.redundant_pod_index].iter().all(|pi| {
                    alle_pods
                        .iter()
                        .find(|p| p.pod_index == *pi)
                        .map(|p| {
                            p.shards
                                .iter()
                                .flat_map(|sh| sh.miners.iter())
                                .all(|m| boese.contains(&m.miner_id))
                        })
                        .unwrap_or(false)
                });
                if beide_boese {
                    kolludiert += 1;
                }
                gesamt += 1;
            }
        }

        assert!(gesamt > 1_000, "zu wenige Zuteilungen gemessen: {gesamt}");
        let gemessen = kolludiert as f64 / gesamt as f64;
        let formel = (beta_prozent as f64 / 100.0).powi(2 * k as i32);
        eprintln!(
            "  β = {beta_prozent} %, k = {k}: gemessen {gemessen:.3e}, Formel β^2k = {formel:.3e} \
             ({kolludiert} von {gesamt})"
        );

        // **Die Aussage, die geprüft wird**, ist nicht Gleichheit mit der
        // Formel — die Stichprobe ist dafür zu klein und die Zuteilung
        // nicht unabhängig. Geprüft wird die Richtung: Die gemessene Rate
        // darf die Formel nicht um Größenordnungen **übersteigen**, denn
        // dann wäre die Schranke des Papiers zu optimistisch.
        assert!(
            gemessen <= (formel * 100.0).max(1.0 / gesamt as f64 * 5.0),
            "gemessene Kollusionsrate {gemessen:.3e} liegt weit über der Schranke {formel:.3e}"
        );
    }
}

// ---------------------------------------------------------------------
// 4.2: Soundness gegen Kap. 6.8
// ---------------------------------------------------------------------

/// **Kap. 6.8 behauptet Unabhängigkeit der drei Ereignisse.**
///
/// > „Ein falsches Segment überlebt die Streitfrist nur, wenn beide
/// > redundanten Pods identisch falsch rechnen, es nicht in der
/// > Stichprobe landet (P = 1−p) und kein Kontrollsegment trifft
/// > (P = 1−γ). **Die Ereignisse sind unabhängig, das Gesamtrisiko
/// > multiplikativ.**"
///
/// Unabhängigkeit ist eine Aussage über die Implementierung, nicht über
/// die Formel. Stichprobe und Kontrollsegment werden beide aus einem Seed
/// gezogen; **liefen sie aus demselben Seed, wären sie korreliert**, und
/// das Produkt wäre falsch.
///
/// Gemessen wird die gemeinsame Überlebensrate gegen das Produkt der
/// Einzelraten.
#[test]
fn soundness_die_ereignisse_sind_unabhaengig() {
    let n = 2_000usize;
    let p_bp = 200u32; // 2 % Stichprobe
    let (gz, gn) = (2u64, 100u64); // 2 % Kontrollsegmente

    let mut ueberlebt = 0usize;
    let mut nur_stichprobe = 0usize;
    let mut nur_kontrolle = 0usize;
    let mut gesamt = 0usize;

    for lauf in 0..100u64 {
        // **Verschiedene Seeds**, wie im Betrieb: Die Stichproben-Lotterie
        // läuft über den Epochenseed des Konsenses, die Einschleusung über
        // den Gateway-Seed.
        let stichprobe = sample_segments(n as u32, p_bp, &seed(lauf));
        let gezogen: std::collections::BTreeSet<u32> =
            stichprobe.sampled_segments.iter().copied().collect();
        let plan: std::collections::BTreeSet<usize> =
            einschleusungsplan(n, gz, gn, &seed(lauf + 1_000_000))
                .unwrap()
                .into_iter()
                .collect();

        for i in 0..n {
            let in_stichprobe = gezogen.contains(&(i as u32));
            let ist_kontrolle = plan.contains(&i);
            if !in_stichprobe {
                nur_stichprobe += 1;
            }
            if !ist_kontrolle {
                nur_kontrolle += 1;
            }
            if !in_stichprobe && !ist_kontrolle {
                ueberlebt += 1;
            }
            gesamt += 1;
        }
    }

    let p_ueberleben = ueberlebt as f64 / gesamt as f64;
    let p_produkt =
        (nur_stichprobe as f64 / gesamt as f64) * (nur_kontrolle as f64 / gesamt as f64);
    eprintln!(
        "  gemeinsam {p_ueberleben:.5}, Produkt der Einzelraten {p_produkt:.5}, \
         Abweichung {:.2} %",
        (p_ueberleben - p_produkt).abs() / p_produkt * 100.0
    );

    // Unabhängig heißt: Die gemeinsame Rate ist das Produkt, bis auf die
    // Stichprobenschwankung.
    assert!(
        (p_ueberleben - p_produkt).abs() / p_produkt < 0.02,
        "gemeinsame Überlebensrate {p_ueberleben:.5} weicht mehr als 2 % vom Produkt \
         {p_produkt:.5} ab; die Ereignisse sind dann nicht unabhängig, und das \
         multiplikative Risiko aus Kap. 6.8 gilt nicht"
    );
}

/// **Die Gegenprobe: Aus demselben Seed sind sie es nicht.**
///
/// Der Test hält fest, **warum** verschiedene Seeds eine Bedingung sind
/// und nicht eine Bequemlichkeit. Liefen Stichprobe und Einschleusung aus
/// demselben Seed, wären die gezogenen Mengen korreliert, und das Produkt
/// aus Kap. 6.8 wäre eine zu optimistische Schranke.
#[test]
fn aus_demselben_seed_waeren_sie_korreliert() {
    let n = 2_000usize;
    let mut ueberlebt = 0usize;
    let mut nur_a = 0usize;
    let mut nur_b = 0usize;
    let mut gesamt = 0usize;

    for lauf in 0..100u64 {
        let s = seed(lauf);
        // **Derselbe Seed für beide** — genau das, was nicht passieren darf.
        let stichprobe = sample_segments(n as u32, 200, &s);
        let gezogen: std::collections::BTreeSet<u32> =
            stichprobe.sampled_segments.iter().copied().collect();
        let plan: std::collections::BTreeSet<usize> =
            einschleusungsplan(n, 2, 100, &s).unwrap().into_iter().collect();

        for i in 0..n {
            let a = !gezogen.contains(&(i as u32));
            let b = !plan.contains(&i);
            if a {
                nur_a += 1;
            }
            if b {
                nur_b += 1;
            }
            if a && b {
                ueberlebt += 1;
            }
            gesamt += 1;
        }
    }
    let p_gemeinsam = ueberlebt as f64 / gesamt as f64;
    let p_produkt = (nur_a as f64 / gesamt as f64) * (nur_b as f64 / gesamt as f64);
    let abweichung = (p_gemeinsam - p_produkt).abs() / p_produkt * 100.0;
    eprintln!("  gleicher Seed: gemeinsam {p_gemeinsam:.5}, Produkt {p_produkt:.5}, Abweichung {abweichung:.2} %");
    // Kein harter Assert auf Korrelation: Die beiden Verfahren ziehen
    // verschieden (Lotterie gegen Sortierschlüssel), also muss ein
    // gemeinsamer Seed nicht zwangsläufig korrelieren. Der Test hält die
    // Messung fest; die **Betriebsbedingung** verschiedener Seiten steht
    // in der Moduldoku von `kontrollsegmente`.
}

// ---------------------------------------------------------------------
// Der Vorrat
// ---------------------------------------------------------------------

/// **Gegenprobe: Ein Kontrollsegment fängt den einmaligen Eingriff.**
///
/// Das ist die Aussage, für die es den Mechanismus gibt. Ohne diesen Test
/// wäre alles darüber Buchhaltung.
#[test]
fn ein_kontrollsegment_faengt_den_einmaligen_eingriff() {
    let mut vorrat = KontrollsegmentVorrat::neu(100);
    let soll = Hash::sha256(b"das richtige Ergebnis");
    vorrat.aufnehmen(Kontrollsegment {
        segment_id: segment(1),
        soll_commitment: soll,
        aufgenommen_in: EpochId(1),
    });

    // Der ehrliche Pod besteht.
    assert_eq!(
        vorrat.pruefen(&segment(1), &soll),
        myl_verifier::Kontrollergebnis::Bestanden
    );

    // Der manipulierte fällt durch, **beim ersten Versuch** und ohne
    // Bisektion: Das richtige Ergebnis lag bereits vor.
    let gefaelscht = Hash::sha256(b"manipuliert");
    assert!(matches!(
        vorrat.pruefen(&segment(1), &gefaelscht),
        myl_verifier::Kontrollergebnis::Abgewichen { .. }
    ));

    // Und ein Segment, das keine Kontrolle ist, wird **nicht beurteilt**.
    // Ein Vorgabewert „bestanden" wäre Fund 41 an anderer Stelle.
    assert_eq!(
        vorrat.pruefen(&segment(99), &gefaelscht),
        myl_verifier::Kontrollergebnis::KeineKontrolle
    );
}

/// **Die Erneuerung hält den Vorrat begrenzt und verdrängt deterministisch.**
#[test]
fn die_erneuerung_verdraengt_die_aeltesten() {
    let mut vorrat = KontrollsegmentVorrat::neu(10);
    for i in 0..50u64 {
        vorrat.erneuern(segment(i), Hash::sha256(&i.to_le_bytes()), EpochId(i));
    }
    assert_eq!(vorrat.len(), 10);
    // Die zehn jüngsten sind übrig.
    for k in vorrat.segmente() {
        assert!(k.aufgenommen_in.0 >= 40, "zu altes Segment im Vorrat");
    }

    // Deterministisch: derselbe Ablauf ergibt denselben Vorrat.
    let mut zweiter = KontrollsegmentVorrat::neu(10);
    for i in 0..50u64 {
        zweiter.erneuern(segment(i), Hash::sha256(&i.to_le_bytes()), EpochId(i));
    }
    let a: Vec<_> = vorrat.segmente().cloned().collect();
    let b: Vec<_> = zweiter.segmente().cloned().collect();
    assert_eq!(a, b);
}
