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
use myl_scheduler::shard_assignment::{assign_shards, Pod};
use myl_types::ids::{EpochId, MinerId};

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

// ---------------------------------------------------------------------
// 4.1: Kollusionswahrscheinlichkeit gegen Anhang B.2
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// ⚑ Hier standen sechs Tests zu den Kontrollsegmenten
// ---------------------------------------------------------------------
//
// Sie prueften die Einschleusungsrate gegen gamma, die
// Unvorhersehbarkeit des Plans ohne Seed, die **Unabhaengigkeit** von
// Stichprobe und Einschleusung (Kap. 6.8 rechnet multiplikativ, und das
// gilt nur, wenn beide nicht aus demselben Seed stammen), die
// Gegenprobe dazu, den einmaligen Eingriff und die Vorratserneuerung.
//
// **Sie sind mit ihrem Gegenstand entfallen** (Entscheidung A1,
// 2026-09-02): Die Kontrollsegmente wurden abgeschafft, gamma ist in die
// Stichprobenrate aufgegangen.
//
// ⚑ **Was damit auch entfaellt, ist eine Annahme, die nie leicht war:**
// Kap. 6.8 multipliziert `(1-p)` und `(1-gamma)` und setzt dafuer
// Unabhaengigkeit voraus. Mit nur noch einer Ziehung gibt es nichts
// mehr, was korreliert sein koennte. **Eine Voraussetzung, die
// wegfaellt, ist besser als eine, die geprueft wird.**
//
// Die Rechnung, die an ihre Stelle tritt, steht in `security_sim.py`,
// Abschnitte 7 und 8: Wie hoch die Stichprobenrate sein muss, damit sie
// beide Stufen gleichwertig ersetzt, und warum der naive Ansatz
// `p + gamma - p*gamma` dabei 20 % zu wenig prueft (Fund 138).

/// Baut `anzahl` Pods aus je `k` Shards mit je einem Miner.
fn pods_bauen(anzahl: u32, k: usize, s: &[u8; 32]) -> Vec<Pod> {
    use myl_scheduler::shard_assignment::MinerCluster;
    use myl_scheduler::miner_filter::{HardwareClass, MinerRegistration};
    use myl_types::node_metadata::GeoRegion;
    let zonen = [GeoRegion::Europe, GeoRegion::NorthAmerica, GeoRegion::Asia];
    (0..anzahl)
        .map(|p| {
            // ⚑ **k+2 Mitglieder, nicht k** (Entscheidung D3, 2026-08-26).
            // Vorher entstanden hier Pods ohne Reserve; das fiel nicht
            // auf, weil dieser Test die Reserve nie ansah.
            let mitglieder = myl_scheduler::shard_assignment::pod_groesse(k as u32);
            let miners: Vec<MinerRegistration> = (0..mitglieder)
                .map(|i| {
                    let id = (p as u64) * 1000 + i as u64;
                    let mut b = [0u8; 32];
                    b[..8].copy_from_slice(&id.to_le_bytes());
                    MinerRegistration {
                        miner_id: MinerId::new(b),
                        hardware_class: HardwareClass::MediumGpu,
                        registration_epoch: 0,
                        // ⚑ **Die Zone kommt aus der Registrierung**
                        // (Fund 110). Sie stand hier bis zum
                        // 2026-09-01 für alle auf `Europe`, während
                        // eine zweite Hilfsfunktion den Pods über
                        // gegossipte Metadaten rotierende Regionen
                        // gab. **Zwei Quellen, die sich widersprachen**,
                        // und die Paarung las die falsche.
                        zone: zonen[(p as usize) % zonen.len()],
                        schluessel: myl_types::bls::BlsPublicKey([0; 48]),
                        netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
                    }
                })
                .collect();
            let cluster = MinerCluster {
                miners,
                max_internal_latency: 10,
            };
            assign_shards(&cluster.miners, k as u32, p, s)
                .expect("k+2 Mitglieder ergeben einen vollständigen Pod")
        })
        .collect()
}

/// **Anhang B.2 gegen die echte Zuteilung.**
///
/// Das Papier sagt `P_koll ≈ β^{2k}` und unterstellt damit, dass die
/// `2k` Positionen eines Segments **unabhängig** aus der Miner-Menge
/// gezogen werden. Die Implementierung zieht anders: `assign_shards`
/// verteilt die Miner eines Clusters per Fisher-Yates auf Shards, und
/// `assign_redundant_pods` wählt aus den **gültigen Paaren** (disjunkt,
/// und zonendivers, solange es solche gibt).
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
            // Die Reserve zählt mit: Ein kolludierender Reservemiter
            // übernimmt bei einem Ausfall und rechnet dann mit.
            let boese: std::collections::BTreeSet<MinerId> = alle_pods
                .iter()
                .flat_map(|p| p.mitglieder())
                .filter(|_| rng.u64() % 100 < beta_prozent)
                .map(|m| m.miner_id)
                .collect();

            let zuteilung = assign_redundant_pods(segmente, &alle_pods, &s)
                .expect("acht Pods über drei Zonen bilden Paare");
            assert!(
                zuteilung.zonendivers,
                "acht Pods über drei Zonen müssen zonendiverse Paare hergeben"
            );
            for z in &zuteilung.zuweisungen {
                let beide_boese = [z.primary_pod_index, z.redundant_pod_index].iter().all(|pi| {
                    alle_pods
                        .iter()
                        .find(|p| p.pod_index == *pi)
                        .map(|p| {
                            // Ein Pod kolludiert, wenn **alle** seine
                            // Shard-Positionen kolludieren. Die Reserve
                            // sitzt nicht in der Pipeline, solange
                            // niemand ausfällt.
                            p.shards
                                .iter()
                                .all(|sh| boese.contains(&sh.miner.miner_id))
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



// ---------------------------------------------------------------------
// Der Vorrat
// ---------------------------------------------------------------------



// ---------------------------------------------------------------------
// 4.3: Liveness der Standby-Übernahme gegen Kap. 6.8
// ---------------------------------------------------------------------

/// **Kap. 6.8 macht eine quantitative Liveness-Zusage**, und hier wird
/// sie gemessen statt geglaubt:
///
/// > „Fällt ein Shard-Miner aus, übernimmt der Standby-Miner des Pods
/// > (k+2 Mitglieder, 2 in Reserve); Session-Verlust **nur bei mehr als
/// > zwei gleichzeitigen Ausfällen** im selben Pod."
///
/// Zwei Aussagen stecken darin, und beide müssen geprüft werden:
/// **bis zu zwei überstehen die Session** und **drei nicht**. Eine
/// Implementierung, die bei drei Ausfällen stillschweigend weiterliefe,
/// verspräche eine Redundanz, die es nicht gibt; eine, die schon bei
/// einem aufgäbe, hielte die Zusage ebenfalls nicht.
///
/// Gemessen über alle Ausfallmuster bis zur Podgröße, nicht über eine
/// ausgesuchte Folge.
#[test]
fn liveness_kap_6_8_zwei_ausfaelle_ja_drei_nein() {
    use myl_pod::standby::{PodBesetzung, Uebernahme, RESERVE_PLAETZE};

    fn miner(b: u64) -> MinerId {
        let mut x = [0u8; 32];
        x[..8].copy_from_slice(&b.to_le_bytes());
        MinerId::new(x)
    }

    for k in 2..=8usize {
        let liste: Vec<MinerId> = (1..=(k + RESERVE_PLAETZE) as u64).map(miner).collect();

        for anzahl_ausfaelle in 0..=(RESERVE_PLAETZE + 2) {
            let mut pod = PodBesetzung::neu(k, &liste, EpochId(1)).expect("Besetzung");
            let mut verloren = false;
            for i in 0..anzahl_ausfaelle {
                let position = i % k;
                match pod.ausfall(position, 100, 0, 6) {
                    Uebernahme::Uebernommen { rebuild, .. } => {
                        // Jede Übernahme zieht einen Rebuild nach sich,
                        // und der ist nie leer, wenn die Session schon
                        // läuft.
                        assert!(!rebuild.ist_leer(), "k = {k}: Rebuild ohne Arbeit");
                    }
                    Uebernahme::SessionVerloren { .. } => verloren = true,
                    Uebernahme::BereitsAusgefallen { .. } => {}
                }
                if verloren {
                    break;
                }
            }

            if anzahl_ausfaelle <= RESERVE_PLAETZE {
                assert!(
                    !verloren && pod.fahrbar(),
                    "k = {k}, {anzahl_ausfaelle} Ausfälle: die Session muss überstehen"
                );
            } else {
                assert!(
                    verloren,
                    "k = {k}, {anzahl_ausfaelle} Ausfälle: die Session muss verloren sein"
                );
            }
        }
    }
}

/// **Die Kosten der Übernahme**, damit die Zusage nicht auf dem Papier
/// gilt und im Betrieb unbezahlbar ist.
///
/// Ein Rebuild kostet einen Prefill über alle bisherigen Positionen,
/// also `Position · Layer`. An Position 10 000 einer langen Sitzung ist
/// das die Arbeit von 10 000 Token für diesen Shard — mehr, als der
/// Shard seit Sitzungsbeginn geleistet hat, wäre es nicht.
///
/// Der Test hält die Größenordnung fest, damit sichtbar bleibt, dass
/// „Standby übernimmt" nicht kostenlos ist.
#[test]
fn die_uebernahme_kostet_einen_prefill() {
    use myl_pod::standby::{PodBesetzung, Uebernahme};

    fn miner(b: u64) -> MinerId {
        let mut x = [0u8; 32];
        x[..8].copy_from_slice(&b.to_le_bytes());
        MinerId::new(x)
    }
    let liste: Vec<MinerId> = (1..=6u64).map(miner).collect();

    for position in [1u64, 100, 10_000] {
        let mut pod = PodBesetzung::neu(4, &liste, EpochId(1)).unwrap();
        match pod.ausfall(0, position, 0, 6) {
            Uebernahme::Uebernommen { rebuild, .. } => {
                assert_eq!(rebuild.arbeit(), position as u128 * 6);
            }
            andere => panic!("musste übernommen werden: {andere:?}"),
        }
    }
}
