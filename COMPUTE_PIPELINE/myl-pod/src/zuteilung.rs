//! Vom Epochenseed zur Pod-Besetzung (Punkt 3.3).
//!
//! # Was hier zusammenkommt
//!
//! `myl-scheduler` rechnet aus, **wer** in welchen Pod gehört;
//! [`crate::standby::PodBesetzung`] führt, **wer gerade wo sitzt**. Bis
//! zum 2026-08-26 gab es keine Stelle, die das eine ins andere
//! überführte: Die Schnittstelle stand auf beiden Seiten, die
//! Verdrahtung dazwischen fehlte.
//!
//! [`plane_epoche`] geht vom **geprüften** Epochenseed über Filter,
//! Clusterbildung und Pod-Zuteilung; [`epochenwechsel_aus_zuteilung`]
//! speist das Ergebnis in eine laufende Besetzung.
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
//! # ⚑ Der Seed wird geprüft, nicht geglaubt
//!
//! [`plane_epoche`] nimmt Beweis und öffentlichen Schlüssel entgegen und
//! verifiziert selbst, statt einen `EpochSeed` als gegeben zu nehmen.
//! **Ein Aufrufer, der die Prüfung vergisst, fällt sonst nicht auf**,
//! und wer den Seed frei wählen kann, wählt seine eigenen Pod-Nachbarn.
//!
//! Zusätzlich muss die Epoche des Seeds passen: Ein gültiger Seed der
//! **vorigen** Epoche ist immer noch ein gültiger VRF-Beweis und hielte
//! sonst die alte Zuteilung fest.

use myl_scheduler::miner_filter::{HardwareClass, MinerRegistration};
use myl_scheduler::shard_assignment::{Pod, Zuteilung};
use myl_scheduler::zonenzuteilung::zuteilung_aus_saat;
use myl_scheduler::vrf_seed::{verify_epoch_seed, EpochSeed};
use myl_types::ids::{EpochId, MinerId};
use myl_types::vrf::{VrfProof, VrfPublicKey};

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
    /// Der Epochenseed hielt der Prüfung nicht stand.
    ///
    /// ⚑ **Der wichtigste Fehler dieses Moduls.** Wer den Seed frei
    /// wählen kann, wählt die Pods, in denen er sitzt, und damit seine
    /// eigenen Nachbarn. Das ist Grinding, und es ist der Angriff, gegen
    /// den der VRF überhaupt gebaut wurde.
    SeedNichtBelegt,
    /// Der Seed gehört zu einer anderen Epoche.
    SeedAndereEpoche { erwartet: u64, bekommen: u64 },
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
            Self::SeedNichtBelegt => write!(
                f,
                "der VRF-Beweis zum Epochenseed gilt nicht. Ein frei gewählter Seed \
                 wäre die Wahl der eigenen Pod-Nachbarn"
            ),
            Self::SeedAndereEpoche {
                erwartet,
                bekommen,
            } => write!(
                f,
                "der Seed gehört zu Epoche {bekommen}, geplant wird {erwartet}. Ein \
                 gültiger Seed der vorigen Epoche hielte die alte Zuteilung fest"
            ),
        }
    }
}

impl std::error::Error for ZuteilungFehler {}

impl From<BesetzungFehler> for ZuteilungFehler {
    fn from(e: BesetzungFehler) -> Self {
        Self::Besetzung(e)
    }
}

/// Die Größen, mit denen eine Epoche geplant wird.
///
/// **Alles davon sind Governance-Parameter** (Kap. 10.3). Sie stehen
/// hier zusammen, damit ein Knoten sie an einer Stelle setzt und nicht
/// über drei Aufrufe verteilt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planparameter {
    /// Shards je Pod, also `k`.
    pub shards_je_pod: u32,
    /// Zugelassene Hardwareklassen.
    pub zugelassene_klassen: Vec<HardwareClass>,
}

/// Plant eine Epoche: vom geprüften Seed zur Pod-Zuteilung.
///
/// **Die Stelle, die bis zum 2026-08-26 fehlte.** Siehe Modulkopf, auch
/// dazu, warum der Seed hier geprüft und nicht geglaubt wird.
///
/// # ⚑ Fund 111: Hier stand eine zweite Herleitung derselben Zuteilung
///
/// Bis zum 2026-09-01 rechnete diese Funktion die Zuteilung **selbst**
/// aus, und sie stimmte mit
/// [`myl_scheduler::zonenzuteilung::zuteilung_der_epoche`], dem Weg der
/// Kette, in **keinem** der drei Schritte überein:
///
/// - Sie clusterte nach **gemessener Latenz**. Die Entscheidung 3b hat
///   das am 2026-09-01 verworfen, weil wer wählt, mit wem er attestiert,
///   mitformt, in welchem Topf er gemischt wird. **Der Aufruf blieb
///   trotzdem stehen.**
/// - Sie nahm die **VRF-Saat**, die Kette die Saat aus dem Blockhash.
/// - Sie ließ nur die Klassen aus [`Planparameter`] zu, die Kette alle.
///
/// **Zwei Knoten, die denselben Pod auf verschiedenen Wegen ausrechnen,
/// bekamen verschiedene Pods.** Die Regel steht jetzt einmal, in
/// `zuteilung_aus_saat`; diese Funktion prüft die Saat und ruft sie.
///
/// ⚑ **Offen bleibt allein, welche Saat gilt.** Diese hier verlangt
/// einen VRF-Beweis, die Kette nimmt den Blockhash. Das ist eine
/// Entscheidung und kein Algorithmus, und sie ist jetzt eine Zeile groß
/// statt einer Datei.
///
/// # Determinismus
///
/// Alle Schritte hängen am selben Seed und an Angaben aus dem
/// Konsenszustand. **Es geht keine gemessene Größe mehr ein.**
pub fn plane_epoche(
    seed: &EpochSeed,
    beweis: &VrfProof,
    vrf_pk: &VrfPublicKey,
    epoche: u64,
    registrierungen: &[MinerRegistration],
    p: &Planparameter,
) -> Result<Zuteilung, ZuteilungFehler> {
    if seed.epoch != epoche {
        return Err(ZuteilungFehler::SeedAndereEpoche {
            erwartet: epoche,
            bekommen: seed.epoch,
        });
    }
    if !verify_epoch_seed(seed, beweis, vrf_pk) {
        return Err(ZuteilungFehler::SeedNichtBelegt);
    }
    Ok(zuteilung_aus_saat(
        registrierungen,
        epoche,
        &seed.as_random_bytes(),
        p.shards_je_pod,
        &p.zugelassene_klassen,
    ))
}

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
    use myl_scheduler::shard_assignment::{assign_shards, pod_groesse, RESERVE_JE_POD};
    use myl_scheduler::vrf_seed::{derive_epoch_seed, seed_alpha};
    use myl_types::hash::Hash;
    use myl_types::vrf::VrfSecretKey;

    fn miner(b: u8) -> MinerRegistration {
        MinerRegistration {
            miner_id: MinerId::new([b; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: 1,
            zone: myl_types::node_metadata::GeoRegion::Europe,
            schluessel: myl_types::bls::BlsPublicKey([0; 48]),
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

    fn vrf() -> VrfSecretKey {
        VrfSecretKey::from_seed([3u8; 32])
    }

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

    fn parameter() -> Planparameter {
        Planparameter {
            shards_je_pod: 4,
            zugelassene_klassen: vec![HardwareClass::MediumGpu, HardwareClass::LargeGpu],
        }
    }

    fn planen(marke: &[u8], epoche: u64, n: u8) -> Result<Zuteilung, ZuteilungFehler> {
        let sk = vrf();
        let v = Hash::sha256(marke);
        let seed = derive_epoch_seed(v, &sk, epoche).expect("Seed");
        let (beweis, _) = sk.prove(&seed_alpha(&v, epoche)).expect("Beweis");
        plane_epoche(
            &seed,
            &beweis,
            &sk.public_key(),
            epoche,
            &registrierungen(n),
            &parameter(),
        )
    }

    /// ⚑ **Fund 111: eine Regel, zwei Eingänge.**
    ///
    /// Bis zum 2026-09-01 rechnete [`plane_epoche`] die Zuteilung selbst
    /// aus, und zwar anders als die Kette: nach gemessener Latenz statt
    /// nach Zone. **Zwei Knoten, die denselben Pod auf verschiedenen
    /// Wegen ausrechnen, bekamen verschiedene Pods.**
    ///
    /// Dieser Test hält die Zusage fest, die den Fund behebt: Bei
    /// gleicher Saat, gleichem Register und gleichen Klassen kommt über
    /// beide Eingänge **dieselbe** Zuteilung heraus. Bliebe hier eine
    /// zweite Herleitung stehen, fiele es auf.
    #[test]
    fn beide_eingaenge_ergeben_dieselbe_zuteilung() {
        let sk = vrf();
        let v = Hash::sha256(b"vorgaengerblock");
        let epoche = 7u64;
        let seed = derive_epoch_seed(v, &sk, epoche).expect("Seed");
        let (beweis, _) = sk.prove(&seed_alpha(&v, epoche)).expect("Beweis");
        let reg = registrierungen(12);
        let p = parameter();

        let ueber_den_pod =
            plane_epoche(&seed, &beweis, &sk.public_key(), epoche, &reg, &p).expect("Planung");
        let ueber_den_scheduler = zuteilung_aus_saat(
            &reg,
            epoche,
            &seed.as_random_bytes(),
            p.shards_je_pod,
            &p.zugelassene_klassen,
        );
        assert_eq!(ueber_den_pod, ueber_den_scheduler);
        assert!(
            !ueber_den_pod.pods.is_empty(),
            "zwölf Miner müssen mindestens einen Pod ergeben"
        );
    }

    /// **Der Punkt, der 3.3 offen hielt:** eine Stelle, die den
    /// Scheduler befragt und das Ergebnis in den Pod einspeist.
    #[test]
    fn eine_epoche_laesst_sich_von_der_kette_bis_zum_pod_planen() {
        let z = planen(b"vorgaengerblock", 7, 6).expect("Planung");
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
        let z = planen(b"vorgaengerblock", 7, 12).expect("Planung");
        assert_eq!(z.pods.len(), 2);
        assert!(z.ohne_pod.is_empty());
        for p in &z.pods {
            assert_eq!(p.shards.len(), 4);
            assert_eq!(p.reserve.len(), RESERVE_JE_POD);
        }
    }

    /// ⚑ **Ein frei gewählter Seed kommt nicht durch.**
    #[test]
    fn ein_seed_ohne_gueltigen_beweis_wird_abgewiesen() {
        let sk = vrf();
        let v = Hash::sha256(b"vorgaengerblock");
        let echt = derive_epoch_seed(v, &sk, 7).expect("Seed");
        let (beweis, _) = sk.prove(&seed_alpha(&v, 7)).expect("Beweis");
        let mut gefaelscht = echt;
        gefaelscht.beta[0] ^= 0xFF;

        let ruf = |s: &EpochSeed| {
            plane_epoche(
                s,
                &beweis,
                &sk.public_key(),
                7,
                &registrierungen(6),
                &parameter(),
            )
        };
        assert_eq!(ruf(&gefaelscht), Err(ZuteilungFehler::SeedNichtBelegt));
        // Gegenprobe: Ohne sie wäre auch eine Prüfung grün, die jeden
        // Seed ablehnt.
        assert!(ruf(&echt).is_ok());
    }

    /// ⚑ **Ein gültiger Seed der vorigen Epoche hält die Rotation nicht auf.**
    #[test]
    fn ein_seed_der_vorigen_epoche_wird_abgewiesen() {
        let sk = vrf();
        let v = Hash::sha256(b"vorgaengerblock");
        let alt = derive_epoch_seed(v, &sk, 6).expect("Seed");
        let (beweis, _) = sk.prove(&seed_alpha(&v, 6)).expect("Beweis");
        assert_eq!(
            plane_epoche(
                &alt,
                &beweis,
                &sk.public_key(),
                7,
                &registrierungen(6),
                &parameter(),
            ),
            Err(ZuteilungFehler::SeedAndereEpoche {
                erwartet: 7,
                bekommen: 6
            })
        );
    }

    #[test]
    fn zwei_knoten_planen_dieselbe_epoche() {
        assert_eq!(
            planen(b"vorgaengerblock", 7, 12).expect("a"),
            planen(b"vorgaengerblock", 7, 12).expect("b")
        );
    }

    #[test]
    fn ein_anderer_vorgaengerblock_ergibt_eine_andere_zuteilung() {
        // Ohne diese Gegenprobe wäre auch eine Planung grün, die den
        // Seed nie benutzt, und dann rotierte nichts.
        assert_ne!(
            planen(b"block-a", 7, 12).expect("a"),
            planen(b"block-b", 7, 12).expect("b")
        );
    }

    #[test]
    fn ein_cluster_ohne_vollstaendigen_pod_meldet_seine_miner() {
        // Fünf Miner bei k=4: einer zu wenig. Sie dürfen nicht
        // verschwinden, sonst warten sie auf eine Zuweisung, die nie
        // kommt.
        let z = planen(b"vorgaengerblock", 7, 5).expect("Planung");
        assert!(z.pods.is_empty());
        assert_eq!(z.ohne_pod.len(), 5);
    }

    #[test]
    fn ein_cluster_von_vierzehn_traegt_zwei_pods_und_meldet_den_rest() {
        let z = planen(b"vorgaengerblock", 7, 14).expect("Planung");
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
        let z = planen(b"vorgaengerblock", 7, 12).expect("Planung");
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
