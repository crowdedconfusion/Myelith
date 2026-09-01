//! Von der Zuteilung zur Pod-Besetzung (Punkt 3.3).
//!
//! # Was hier zusammenkommt
//!
//! `myl-scheduler` rechnet aus, **wer** in welchen Pod gehört;
//! [`crate::standby::PodBesetzung`] führt, **wer gerade wo sitzt**. Bis
//! zum 2026-08-26 gab es keine Stelle, die das eine ins andere
//! überführte: Die Schnittstelle stand auf beiden Seiten, die
//! Verdrahtung dazwischen fehlte.
//!
//! [`epochenwechsel_aus_zuteilung`] speist eine fertige Zuteilung in
//! eine laufende Besetzung. **Gerechnet wird sie nicht hier**, sondern
//! einmal in `myl_scheduler::zuteilung_der_epoche`; warum, steht in der
//! Notiz weiter unten.
//!
//! # ⚑ Was beim Zusammenstecken auffiel (Entscheidung D3)
//!
//! Die beiden Seiten passten **nicht** zusammen. `assign_shards` legte
//! mehrere Miner in jeden Shard, während Anhang A.2 („Pod-Größe k+2"),
//! Kap. 6.8 und das Glossar von **einem** Miner je Position plus zwei in
//! Reserve sprechen. Jede Seite war für sich stimmig und vollständig
//! getestet; genau deshalb konnte der Widerspruch bestehen.
//!
//! **Entschieden am 2026-08-26:** Der Scheduler richtet sich nach dem
//! Papier. Ein Pod hat `k` Positionen mit je einem Miner und zwei in
//! Reserve, und ein Cluster liefert so viele vollständige Pods, wie
//! hineinpassen. Dieses Modul ist seither eine **Übersetzung** und keine
//! Brücke über einen Widerspruch: Die Reihenfolge, die
//! [`PodBesetzung::neu`] erwartet, ist genau
//! `myl_scheduler::Pod::mitglieder`.
//!
//! # ⚑ Die Zuteilung wird hier nicht mehr gerechnet (Punkt 43)
//!
//! Bis zum 2026-09-01 stand hier `plane_epoche`, eine **zweite**
//! Herleitung derselben Zuteilung. Fund 111 hat gezeigt, wohin das
//! führt: Sie stimmte mit dem Weg der Kette in keinem Schritt überein,
//! und zwei Knoten bekamen verschiedene Pods.
//!
//! Die Regel steht jetzt einmal, im Scheduler. **Was hier bleibt, ist
//! die Übersetzung ins Laufende**, und das ist die Arbeit, die dieses
//! Crate wirklich besitzt.

use myl_scheduler::shard_assignment::{Pod, Zuteilung};
use myl_types::ids::{EpochId, MinerId};

use crate::standby::{BesetzungFehler, PodBesetzung, RebuildAuftrag};

/// Was beim Planen oder Übersetzen einer Zuteilung schiefgehen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZuteilungFehler {
    /// Der gesuchte Pod steht nicht in der Zuteilung.
    KeinSolcherPod { gesucht: u32, vorhanden: usize },
    /// Die Zuteilung hat eine andere Shardzahl als der Pod Positionen.
    ///
    /// Entweder bliebe eine Position unbesetzt oder ein Shard hätte
    /// keine.
    ShardzahlPasstNicht { erwartet: usize, bekommen: usize },
    /// Die Besetzung ließ sich nicht bilden.
    Besetzung(BesetzungFehler),
}

impl std::fmt::Display for ZuteilungFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeinSolcherPod { gesucht, vorhanden } => write!(
                f,
                "Pod {gesucht} steht nicht in der Zuteilung; sie führt {vorhanden}"
            ),
            Self::ShardzahlPasstNicht {
                erwartet,
                bekommen,
            } => write!(
                f,
                "die Zuteilung hat {bekommen} Shards, der Pod aber {erwartet} Positionen. \
                 Entweder bliebe eine Position unbesetzt oder ein Shard ohne Position"
            ),
            Self::Besetzung(e) => write!(f, "Besetzung: {e}"),
        }
    }
}

impl std::error::Error for ZuteilungFehler {}

impl From<BesetzungFehler> for ZuteilungFehler {
    fn from(e: BesetzungFehler) -> Self {
        Self::Besetzung(e)
    }
}

// ⚑ **Notiz zu `plane_epoche` und `Planparameter`, entfernt am 2026-09-01
// (Entscheidung zu Punkt 43).**
//
// Hier stand eine zweite Herleitung der Epochenzuteilung: aus einem
// **VRF-Seed** mit Beweis, mit eigenen zugelassenen Hardwareklassen und,
// bis Fund 111, mit eigener Clusterbildung. Sie ist ersatzlos
// weggefallen; wer die Zuteilung braucht, ruft
// [`myl_scheduler::zuteilung_der_epoche`].
//
// **Die Saat ist der vorherige Blockhash, nicht der VRF**, und zwar aus
// vier Gründen:
//
// - **Den Blockhash gibt es immer.** Ein VRF-Seed muss erzeugt,
//   veröffentlicht und geprüft werden; schweigt der Halter, gibt es
//   keine Zuteilung. Das ist eine Liveness-Abhängigkeit für nichts.
// - **Gemahlen werden können beide.** Wer den letzten Block einer Epoche
//   erzeugt, sieht bei beiden Verfahren die entstehende Zuteilung und
//   wählt die ihm liebste. Begrenzt wird das vom Registrierungsschluss
//   bei `e-2`, und der bleibt.
// - ⚑ **Der Vorteil des VRF trägt hier nicht.** Er bringt
//   Unvorhersehbarkeit **für alle anderen**. Die zahlt sich aus, wo
//   jemand von frühem Wissen profitiert; die Zuteilung rechnet jeder im
//   selben Augenblick aus demselben Kettenzustand.
// - ⚑ **„Verifiziert statt geglaubt" (D3) bleibt, und zwar stärker.**
//   An einem Blockhash ist nichts zu glauben, er **ist** die Kette. Wer
//   einen falschen einsetzt, bekommt eine Zuteilung, die niemand teilt.
//   Auch die Epochenbindung bleibt: Sie steckt in `epochenseed(hash,
//   epoche)` und ist damit Bau statt Prüfung.
//
// ⚑ **Wo der VRF hingehört, ist die Stichprobenlotterie.** Dort hilft
// frühes Wissen sehr wohl: Wer weiß, welche Segmente geprüft werden,
// weiß auch, bei welchen er sich nicht anstrengen muss. Das ist ein
// eigener Punkt und keine Sache dieses Moduls.

/// Die Mitglieder eines Pods in der Reihenfolge, die
/// [`PodBesetzung::neu`] erwartet: erst die Positionen, dann die
/// Reserve.
///
/// Seit der Entscheidung D3 ist das genau `Pod::mitglieder`; die
/// Funktion bleibt, weil sie die **Zusage** benennt. Änderte der
/// Scheduler seine Reihenfolge, fiele es hier auf und nicht erst in
/// einer laufenden Sitzung.
pub fn besetzungsreihenfolge(pod: &Pod) -> Vec<MinerId> {
    pod.mitglieder().map(|m| m.miner_id).collect()
}

/// Prüft **vor** dem Epochenwechsel, ob eine Zuteilung besetzbar ist.
///
/// Ein Pod, der erst beim Wechsel scheitert, bleibt mitten in einer
/// Sitzung stehen. Diese Prüfung kostet nichts und beantwortet dieselbe
/// Frage vorher.
pub fn ist_besetzbar(pod: &Pod, positionen: usize) -> Result<Vec<MinerId>, ZuteilungFehler> {
    if pod.shards.len() != positionen {
        return Err(ZuteilungFehler::ShardzahlPasstNicht {
            erwartet: positionen,
            bekommen: pod.shards.len(),
        });
    }
    let reihenfolge = besetzungsreihenfolge(pod);
    let gebraucht = positionen + crate::standby::RESERVE_PLAETZE;
    if reihenfolge.len() < gebraucht {
        return Err(ZuteilungFehler::Besetzung(BesetzungFehler::ZuWenigMiner {
            gebraucht,
            bekommen: reihenfolge.len(),
        }));
    }
    Ok(reihenfolge)
}

/// Sucht den eigenen Pod in der Zuteilung.
pub fn pod_aus_zuteilung(pods: &[Pod], pod_index: u32) -> Result<&Pod, ZuteilungFehler> {
    pods.iter()
        .find(|p| p.pod_index == pod_index)
        .ok_or(ZuteilungFehler::KeinSolcherPod {
            gesucht: pod_index,
            vorhanden: pods.len(),
        })
}

/// Baut die Netzreserve der Epoche aus dem, was die Zuteilung übrig
/// ließ (Punkt 3.4).
///
/// `Zuteilung::ohne_pod` führt jeden registrierten Miner, der in keinen
/// vollständigen Pod passte. **Bisher wartete er auf eine Zuweisung, die
/// nie kam**; jetzt ist er die Reserve, aus der ein Pod nachbesetzt,
/// wenn seine eigenen zwei Plätze verbraucht sind.
///
/// Der Seed ist derselbe wie für die Pod-Bildung, die Mischung aber
/// nicht: [`crate::netzreserve::DST_NETZRESERVE`] trennt die beiden
/// Ableitungen.
pub fn netzreserve_aus_zuteilung(
    zuteilung: &Zuteilung,
    seed: &[u8; 32],
    epoche: EpochId,
) -> crate::netzreserve::Netzreserve {
    let frei: Vec<MinerId> = zuteilung.ohne_pod.iter().map(|r| r.miner_id).collect();
    crate::netzreserve::Netzreserve::verteilen(&frei, zuteilung.pods.len(), seed, epoche)
}

/// Wie [`netzreserve_aus_zuteilung`], ordnet aber Auffällige nach hinten
/// (Punkt 3.6).
///
/// `verstoesse` bildet einen Miner auf seine Zahl im
/// Beobachtungsfenster ab. Sie stammt aus
/// `myl_ledger::LedgerState::verstoesse_im_fenster`, das nach **Adresse**
/// fragt; Kennung und Adresse sind verschiedene Typen über denselben
/// Bytes, die Abbildung macht der Aufrufer.
///
/// ⚑ **Der Ledger wird hier nicht gelesen**, und das ist Absicht:
/// `myl-pod` hängt nicht an ihm. Wer die Zahlen liefert, hat sie an
/// **einer** Blockhöhe gelesen, und nur so sind sie über zwei Knoten
/// hinweg dieselben.
pub fn netzreserve_aus_zuteilung_gestaffelt(
    zuteilung: &Zuteilung,
    seed: &[u8; 32],
    epoche: EpochId,
    verstoesse: &std::collections::BTreeMap<MinerId, u64>,
) -> crate::netzreserve::Netzreserve {
    let frei: Vec<MinerId> = zuteilung.ohne_pod.iter().map(|r| r.miner_id).collect();
    crate::netzreserve::Netzreserve::verteilen_gestaffelt(
        &frei,
        zuteilung.pods.len(),
        seed,
        epoche,
        verstoesse,
    )
}

/// Führt den Epochenwechsel aus einer Scheduler-Zuteilung aus.
///
/// Gibt die Rebuild-Aufträge der gewechselten Positionen zurück.
pub fn epochenwechsel_aus_zuteilung(
    besetzung: &mut PodBesetzung,
    pods: &[Pod],
    pod_index: u32,
    neue_epoche: EpochId,
    aktuelle_position: u64,
    layer_grenzen: &[(u64, u64)],
) -> Result<Vec<(usize, RebuildAuftrag)>, ZuteilungFehler> {
    let pod = pod_aus_zuteilung(pods, pod_index)?;
    let reihenfolge = ist_besetzbar(pod, besetzung.positionen())?;
    Ok(besetzung.epochenwechsel(
        neue_epoche,
        &reihenfolge,
        aktuelle_position,
        layer_grenzen,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_scheduler::miner_filter::{HardwareClass, MinerRegistration};
    use myl_scheduler::zonenzuteilung::zuteilung_der_epoche;
    use myl_scheduler::shard_assignment::{assign_shards, pod_groesse, RESERVE_JE_POD};
    use myl_types::hash::Hash;

    fn miner(b: u8) -> MinerRegistration {
        MinerRegistration {
            miner_id: MinerId::new([b; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: 1,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            schluessel: myl_types::bls::BlsPublicKey([0; 48]),
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        }
    }

    fn saat(b: u8) -> [u8; 32] {
        [b; 32]
    }

    fn pod(k: u32, n: u8, index: u32, s: u8) -> Pod {
        let mitglieder: Vec<MinerRegistration> = (0..n).map(miner).collect();
        assign_shards(&mitglieder, k, index, &saat(s)).expect("Pod")
    }

    /// ⚑ **Die beiden Reservekonstanten müssen übereinstimmen.**
    ///
    /// Sie stehen in zwei Crates, `myl_scheduler::RESERVE_JE_POD` und
    /// `myl_pod::standby::RESERVE_PLAETZE`. Liefen sie auseinander,
    /// erzeugte der Scheduler Pods, die kein Pod besetzen kann, **und
    /// zwar still**: `ist_besetzbar` meldete `ZuWenigMiner` für jede
    /// Zuteilung, und die Ursache stünde in einer anderen Komponente.
    ///
    /// Zwei Quellen für eine Zahl, also ein Test, der sie
    /// gegeneinanderhält.
    #[test]
    fn die_reservekonstanten_beider_crates_stimmen_ueberein() {
        assert_eq!(RESERVE_JE_POD, crate::standby::RESERVE_PLAETZE);
    }

    // ── Die Übersetzung ─────────────────────────────────────────────

    #[test]
    fn die_reihenfolge_ist_erst_positionen_dann_reserve() {
        let p = pod(4, 6, 0, 7);
        let r = besetzungsreihenfolge(&p);
        assert_eq!(r.len(), pod_groesse(4));
        for (i, shard) in p.shards.iter().enumerate() {
            assert_eq!(r[i], shard.miner.miner_id, "Position {i}");
        }
        for (i, res) in p.reserve.iter().enumerate() {
            assert_eq!(r[4 + i], res.miner_id, "Reserve {i}");
        }
    }

    #[test]
    fn eine_zuteilung_laesst_sich_unmittelbar_besetzen() {
        let p = pod(4, 6, 0, 7);
        let r = ist_besetzbar(&p, 4).expect("besetzbar");
        let b = PodBesetzung::neu(4, &r, EpochId(7)).expect("Besetzung");
        assert!(b.fahrbar());
        assert_eq!(b.reserve_uebrig(), RESERVE_JE_POD);
        // Und die Positionen stimmen mit den Shards überein.
        for (i, shard) in p.shards.iter().enumerate() {
            assert_eq!(b.miner_an(i), Some(shard.miner.miner_id));
        }
    }

    #[test]
    fn eine_falsche_shardzahl_wird_abgewiesen() {
        let p = pod(3, 5, 0, 7);
        assert_eq!(
            ist_besetzbar(&p, 4),
            Err(ZuteilungFehler::ShardzahlPasstNicht {
                erwartet: 4,
                bekommen: 3
            })
        );
    }

    #[test]
    fn ein_unbekannter_pod_wird_abgewiesen() {
        let pods = vec![pod(4, 6, 0, 7)];
        assert_eq!(
            pod_aus_zuteilung(&pods, 3).unwrap_err(),
            ZuteilungFehler::KeinSolcherPod {
                gesucht: 3,
                vorhanden: 1
            }
        );
    }

    // ── Der Epochenwechsel ──────────────────────────────────────────

    #[test]
    fn ein_epochenwechsel_aus_der_zuteilung_liefert_die_auftraege() {
        let alt = pod(4, 6, 0, 7);
        let mut besetzung =
            PodBesetzung::neu(4, &besetzungsreihenfolge(&alt), EpochId(5)).expect("Besetzung");

        // Andere Epoche, anderer Seed, andere Zuteilung.
        let neu = pod(4, 6, 0, 9);
        let grenzen = [(0u64, 7u64), (7, 14), (14, 21), (21, 28)];
        let auftraege = epochenwechsel_aus_zuteilung(
            &mut besetzung,
            std::slice::from_ref(&neu),
            0,
            EpochId(6),
            42,
            &grenzen,
        )
        .expect("Wechsel");

        for (pos, auftrag) in &auftraege {
            assert_eq!(
                (auftrag.layer_start, auftrag.layer_end),
                grenzen[*pos],
                "Position {pos} bekam fremde Layergrenzen"
            );
            assert_eq!(auftrag.bis_position, 42);
        }
        assert_eq!(besetzung.epoche(), EpochId(6));
    }

    #[test]
    fn eine_unveraenderte_zuteilung_erzeugt_keine_auftraege() {
        // Ein Rebuild ist teuer, und einer ohne Anlass ist verschenkte
        // Rechenzeit.
        let p = pod(4, 6, 0, 7);
        let mut besetzung =
            PodBesetzung::neu(4, &besetzungsreihenfolge(&p), EpochId(5)).expect("Besetzung");
        let grenzen = [(0u64, 7u64), (7, 14), (14, 21), (21, 28)];
        let auftraege = epochenwechsel_aus_zuteilung(
            &mut besetzung,
            std::slice::from_ref(&p),
            0,
            EpochId(6),
            42,
            &grenzen,
        )
        .expect("Wechsel");
        assert!(auftraege.is_empty(), "{} Aufträge ohne Anlass", auftraege.len());
    }

    #[test]
    fn ein_wechsel_auf_eine_falsche_zuteilung_scheitert_ohne_schaden() {
        // ⚑ Der wichtige Teil ist „ohne Schaden": Ein Pod, der beim
        // gescheiterten Wechsel seine Besetzung verliert, ist schlimmer
        // dran als vorher.
        let gut = pod(4, 6, 0, 7);
        let mut besetzung =
            PodBesetzung::neu(4, &besetzungsreihenfolge(&gut), EpochId(5)).expect("Besetzung");
        let vorher = besetzung.belegung().to_vec();

        let falsch = pod(3, 5, 0, 9);
        let grenzen = [(0u64, 7u64), (7, 14), (14, 21), (21, 28)];
        assert!(epochenwechsel_aus_zuteilung(
            &mut besetzung,
            std::slice::from_ref(&falsch),
            0,
            EpochId(6),
            42,
            &grenzen,
        )
        .is_err());
        assert_eq!(besetzung.belegung(), &vorher[..], "die Besetzung wurde beschädigt");
        assert_eq!(besetzung.epoche(), EpochId(5), "die Epoche sprang trotzdem");
    }

    // ── Der volle Bogen: von der Kette zum Pod ──────────────────────


    /// ⚑ **Über mehrere Zonen**, sonst könnte dieser Aufbau eine
    /// Zonenbildung nicht von einem einzigen großen Cluster
    /// unterscheiden. Die Gegenprobe hat das gezeigt: Mit lauter
    /// `Europe` blieb der Test grün, als `plane_epoche` versuchsweise
    /// wieder selbst rechnete.
    fn registrierungen(n: u8) -> Vec<MinerRegistration> {
        use myl_types::node_metadata::GeoRegion;
        let zonen = [GeoRegion::Europe, GeoRegion::NorthAmerica, GeoRegion::Asia];
        (0..n)
            .map(|b| {
                let mut m = miner(b);
                m.zone = zonen[(b as usize) % zonen.len()];
                m
            })
            .collect()
    }

    /// ⚑ **Planen heißt jetzt: den Scheduler fragen.**
    ///
    /// Die Saat ist der vorherige Blockhash (Punkt 43). Die
    /// Epochenbindung steckt in der Ableitung und nicht in einer
    /// Prüfung: Wer die Epoche verwechselt, bekommt eine andere Saat und
    /// damit eine Zuteilung, die niemand teilt.
    fn planen(marke: &[u8], epoche: u64, n: u8) -> Zuteilung {
        zuteilung_der_epoche(&registrierungen(n), epoche, &Hash::sha256(marke), 4)
    }

    // ⚑ **Hier stand ein Test, der zweimal dieselbe reine Funktion rief
    // und ihre Ergebnisse verglich.** Er hätte nur scheitern können,
    // wenn die Zuteilung zufällig wäre, und das verbietet der
    // Konsensvertrag ohnehin. **Eine Zusage, die nicht brechen kann,
    // ist keine Zusage**, dieselbe Klasse wie die schwachen Tests vom
    // 2026-09-01.
    //
    // Die Zusage aus Fund 111 lautet „die Regel steht nur einmal da",
    // und die hält kein Test, sondern **die Abwesenheit des Codes**:
    // `plane_epoche` gibt es nicht mehr. Geprüft wird stattdessen, was
    // prüfbar ist, nämlich dass die Zuteilung des Schedulers hier
    // besetzbar ankommt.

    /// **Der Punkt, der 3.3 offen hielt:** eine Stelle, die den
    /// Scheduler befragt und das Ergebnis in den Pod einspeist.
    #[test]
    fn eine_epoche_laesst_sich_von_der_kette_bis_zum_pod_planen() {
        let z = planen(b"vorgaengerblock", 7, 6);
        assert_eq!(z.pods.len(), 1, "sechs Miner tragen genau einen Pod");
        assert!(z.ohne_pod.is_empty());

        let r = ist_besetzbar(&z.pods[0], 4).expect("besetzbar");
        let b = PodBesetzung::neu(4, &r, EpochId(7)).expect("Besetzung");
        assert!(b.fahrbar());
        assert_eq!(b.reserve_uebrig(), RESERVE_JE_POD);
    }

    #[test]
    fn zwoelf_miner_tragen_zwei_pods() {
        // Mehr Miner heißt mehr Kapazität, nicht mehr Belegung je
        // Position. Das ist die Aussage von D3.
        let z = planen(b"vorgaengerblock", 7, 12);
        assert_eq!(z.pods.len(), 2);
        assert!(z.ohne_pod.is_empty());
        for p in &z.pods {
            assert_eq!(p.shards.len(), 4);
            assert_eq!(p.reserve.len(), RESERVE_JE_POD);
        }
    }



    #[test]
    fn zwei_knoten_planen_dieselbe_epoche() {
        assert_eq!(
            planen(b"vorgaengerblock", 7, 12),
            planen(b"vorgaengerblock", 7, 12)
        );
    }

    #[test]
    fn ein_anderer_vorgaengerblock_ergibt_eine_andere_zuteilung() {
        // Ohne diese Gegenprobe wäre auch eine Planung grün, die den
        // Seed nie benutzt, und dann rotierte nichts.
        assert_ne!(
            planen(b"block-a", 7, 12),
            planen(b"block-b", 7, 12)
        );
    }

    #[test]
    fn ein_cluster_ohne_vollstaendigen_pod_meldet_seine_miner() {
        // Fünf Miner bei k=4: einer zu wenig. Sie dürfen nicht
        // verschwinden, sonst warten sie auf eine Zuweisung, die nie
        // kommt.
        let z = planen(b"vorgaengerblock", 7, 5);
        assert!(z.pods.is_empty());
        assert_eq!(z.ohne_pod.len(), 5);
    }

    #[test]
    fn ein_cluster_von_vierzehn_traegt_zwei_pods_und_meldet_den_rest() {
        let z = planen(b"vorgaengerblock", 7, 14);
        assert_eq!(z.pods.len(), 2);
        assert_eq!(z.ohne_pod.len(), 2, "zwei Miner passten nicht mehr hinein");
    }

    /// Zwei Pods derselben Zuteilung teilen sich niemanden.
    ///
    /// Darauf beruht Stufe 1 der Verifikation: Ein Redundanzpaar aus
    /// zwei Pods, die eine Maschine teilen, verglände zwei Ergebnisse
    /// derselben Maschine.
    #[test]
    fn zwei_pods_derselben_zuteilung_sind_disjunkt() {
        let z = planen(b"vorgaengerblock", 7, 12);
        let a: std::collections::BTreeSet<MinerId> =
            z.pods[0].mitglieder().map(|m| m.miner_id).collect();
        let b: std::collections::BTreeSet<MinerId> =
            z.pods[1].mitglieder().map(|m| m.miner_id).collect();
        assert!(a.is_disjoint(&b), "zwei Pods teilten sich einen Miner");
    }

    /// ⚑ **Punkt 3.4: Wer in keinen Pod passte, wartet nicht mehr
    /// vergeblich.** Aus `ohne_pod` wird die Netzreserve, und sie ist
    /// auf die Pods aufgeteilt.
    #[test]
    fn aus_ohne_pod_wird_die_netzreserve() {
        let zuteilung = Zuteilung {
            pods: vec![pod(4, 6, 0, 1), pod(4, 6, 1, 2)],
            ohne_pod: (20..=25).map(miner).collect(),
        };
        let netz = netzreserve_aus_zuteilung(&zuteilung, &[9u8; 32], EpochId(3));
        assert_eq!(netz.pods(), 2);
        assert_eq!(netz.uebrig(), 6);
        assert!(netz.pods_ohne_reserve().is_empty());
    }

    /// Ging die Zuteilung glatt auf, gibt es keine Netzreserve, und
    /// jeder Pod erfährt das.
    #[test]
    fn ohne_uebrige_gibt_es_keine_netzreserve() {
        let zuteilung = Zuteilung {
            pods: vec![pod(4, 6, 0, 1)],
            ohne_pod: vec![],
        };
        let netz = netzreserve_aus_zuteilung(&zuteilung, &[9u8; 32], EpochId(3));
        assert_eq!(netz.uebrig(), 0);
        assert_eq!(netz.pods_ohne_reserve(), vec![0]);
    }

    /// ⚑ **Punkt 3.6 an der Nahtstelle:** Wer auffiel, steht in der
    /// Netzreserve hinten.
    #[test]
    fn die_netzreserve_staffelt_nach_verstoessen() {
        let zuteilung = Zuteilung {
            pods: vec![pod(4, 6, 0, 1)],
            ohne_pod: (20..=25).map(miner).collect(),
        };
        let mut v = std::collections::BTreeMap::new();
        // Die ersten drei sind auffällig.
        for b in 20..=22u8 {
            v.insert(MinerId::new([b; 32]), 4u64);
        }
        let netz = netzreserve_aus_zuteilung_gestaffelt(&zuteilung, &[9u8; 32], EpochId(3), &v);
        let folge = netz.fuer_pod(0);
        assert_eq!(folge.len(), 6);
        for m in &folge[..3] {
            assert!(!v.contains_key(m), "ein Auffälliger stand vorn: {m:?}");
        }
    }

    /// Ohne Verstöße ist die gestaffelte Verteilung die gewöhnliche.
    #[test]
    fn ohne_verstoesse_staffelt_nichts() {
        let zuteilung = Zuteilung {
            pods: vec![pod(4, 6, 0, 1), pod(4, 6, 1, 2)],
            ohne_pod: (20..=25).map(miner).collect(),
        };
        let leer = std::collections::BTreeMap::new();
        assert_eq!(
            netzreserve_aus_zuteilung_gestaffelt(&zuteilung, &[9u8; 32], EpochId(3), &leer),
            netzreserve_aus_zuteilung(&zuteilung, &[9u8; 32], EpochId(3))
        );
    }
}
