//! Die Zuteilung einer Epoche, abgeleitet statt gespeichert (Punkt 40, Glied 3c).
//!
//! # ⚑ Zonen statt Latenzgraph (Entscheidung 3b, 2026-09-01)
//!
//! Bis zum 2026-09-01 bildete `geo_clustering::form_clusters` Cluster
//! aus einer **gemessenen** Latenzmatrix. Die gehört nicht in den
//! Konsens: Wer
//! wählt, mit wem er attestiert, formt mit, in welchem Topf er gemischt
//! wird, und erhöht damit seine Chance, **beide Seiten eines
//! Redundanzpaars** zu besetzen. Dann verglände Stufe 1 der Verifikation
//! zwei Ergebnisse desselben Betreibers.
//!
//! Hier bilden **Zonen** die Cluster. Die Zone steht seit dem
//! 2026-09-01 in der Registrierung und damit im Konsenszustand; sie ist
//! **eine Angabe je Miner statt einer Matrix über alle Paare**, also
//! O(1) statt O(n²), und niemand muss mitzeichnen, also kann auch
//! niemand jemanden isolieren.
//!
//! # ⚑ Abgeleitet und nicht gespeichert
//!
//! Die Zuteilung ist eine **reine Funktion** aus Register, Epoche und
//! Blockhash. Sie in den Zustand zu schreiben wäre eine zweite Quelle
//! für dieselbe Aussage, und zwei Quellen laufen auseinander; dieselbe
//! Lehre wie bei Fund 34. Wer sie braucht, rechnet sie aus.
//!
//! # ⚑ Der Seed, und was an ihm schwach ist
//!
//! Er folgt aus [`crate::vrf_seed::seed_alpha`], also aus Blockhash und
//! Epoche mit eigenem Trennstring. **Damit kann der Erzeuger des letzten
//! Blocks einer Epoche den Seed mahlen**: Er sieht für jeden möglichen
//! Block die entstehende Zuteilung und wählt die ihm liebste.
//!
//! **Ein VRF-Seed behebt das nicht**, auch wenn es so aussieht: Der
//! Erzeuger hält den Schlüssel, kennt also seine eigene Ausgabe und kann
//! ebenso wählen. Was der VRF bringt, ist Unvorhersehbarkeit **für alle
//! anderen** und Nachprüfbarkeit, nicht Mahlfestigkeit.
//!
//! ⚑ **Wogegen es hilft, steht schon im Entwurf:** Der
//! Registrierungsschluss bei `e-2` friert die **Menge** der Teilnehmer
//! zwei Epochen im Voraus ein. Ein Mahlender kann umschichten, wer wo
//! landet, aber nicht, **wer überhaupt dabei ist**. Das begrenzt den
//! Angriff, hebt ihn nicht auf, und das gehört so gesagt.

use std::collections::BTreeMap;

use myl_types::hash::Hash;
use myl_types::miner::{HardwareClass, MinerRegistration};
use myl_types::node_metadata::GeoRegion;

use crate::miner_filter::filter_miners;
use crate::shard_assignment::{assign_pods, pod_groesse, MinerCluster, Zuteilung};
use crate::vrf_seed::seed_alpha;
use myl_types::seed_rng::deterministic_shuffle;
use sha2::{Digest, Sha256};

/// Bildet je Zone ein Cluster.
///
/// Die Zonen kommen in kanonischer Reihenfolge, weil `BTreeMap` sortiert
/// und `GeoRegion` eine feste Ordnung hat; innerhalb einer Zone bleibt
/// die Reihenfolge der Eingabe erhalten. **Beides ist nötig**, denn zwei
/// Knoten mit verschiedener Reihenfolge kämen zu verschiedenen Pods.
///
/// `max_internal_latency` bleibt null: Es wird nichts gemessen, und eine
/// erfundene Zahl wäre schlimmer als keine.
///
/// # ⚑ Fund 112: Eine dünne Zone schloss ihre Miner aus, und das lud zum Lügen ein
///
/// Ein Pod braucht `k + 2` Mitglieder. Bildete man je Zone genau ein
/// Cluster, so trug eine Zone mit weniger Minern **keinen einzigen Pod**,
/// und ihre Miner landeten in `ohne_pod`. Bei sieben Zonen und `k = 8`
/// hieße das: **siebzig Miner, bevor jede Zone einen Pod trägt.**
///
/// ⚑ **Und der Schaden ist nicht nur der Ausschluss, sondern der
/// Anreiz.** Wer allein in seiner Zone steht, verdient nichts, solange er
/// die Wahrheit sagt, und alles, sobald er eine volle Zone angibt. **Das
/// Verfahren drängte die Angabe zur Unwahrheit**, und zwar genau dort, wo
/// sie die Vielfalt am meisten wert gewesen wäre.
///
/// Zonen unter `mindestbesetzung` kommen deshalb in **ein gemeinsames
/// Sammelcluster**, in kanonischer Zonenreihenfolge. Niemand wird
/// ausgeschlossen, und eine dünne Zone anzugeben kostet nichts mehr.
///
/// **Was das Sammelcluster nicht vortäuscht:** Seine Pods haben
/// Mitglieder aus mehreren Zonen, also **keine** bestimmte Ausfallzone.
/// Die Redundanzpaarung sieht das und behandelt sie entsprechend, statt
/// ihnen ein Etikett zu geben, das nicht stimmt.
///
/// # ⚑ Fund 142: Die Reihenfolge war die des Registers, und das war ein Hebel
///
/// Bis zum 2026-09-02 blieb innerhalb einer Zone die **Eingabereihenfolge**
/// erhalten, und die Eingabe ist `state.miner.values()`, also nach
/// `MinerId` sortiert, und `MinerId` ist `SHA-256` über den BLS-Schlüssel.
/// Wer eine Kennung an einer bestimmten Stelle haben wollte, erzeugte
/// Schlüssel, bis eine dort landete: **gemessen 0,06 Sekunden für einen
/// ganzen Pod bei tausend ehrlichen Minern.**
///
/// ⚑ **Damit fiel die Annahme, auf der Stufe 1 steht.** Zwei Pods
/// rechnen dieselbe Arbeit doppelt, und das trägt nur, solange ein
/// Angreifer nicht bestimmen kann, mit wem er in einem Pod sitzt.
///
/// **Der Kommentar in [`crate::shard_assignment::assign_pods`] behauptete
/// das Mischen bereits** („aus dem seed-gesteuerten Shuffle der
/// Clusterbildung"). Er stammte aus `geo_clustering.rs`, die am
/// 2026-09-01 entfernt wurde; der Shuffle ging mit ihr, der Satz blieb.
/// **Eine Zusicherung, deren Code verschwunden ist, ist gefährlicher als
/// gar keine**, denn sie hält den nächsten Leser vom Nachsehen ab.
///
/// # ⚑ Was das Mischen **nicht** schließt, und das gehört hierher
///
/// **Die Zone ist eine Erklärung** (Fund 108). Wer eine Zone angibt, in
/// der sonst niemand steht, bekommt daraus ein eigenes Cluster und
/// damit ganze Pods, gemischt oder nicht: **Zwölf Anmeldungen in zwei
/// leeren Zonen ergeben zwei ganze Pods und ein zonendiverses, also
/// bevorzugtes Redundanzpaar**, und eine Anmeldung kostet nichts.
///
/// Das Mischen nimmt den **Rechenangriff** auf die Kennung weg, nicht
/// den **Zonenhebel**. Der bleibt offen, ist gemessen und benannt; ihn
/// zu schließen hieße, die Zone aus der Besetzung zu nehmen, und das
/// kostet Latenz in der Pipeline. Es ist eine Entscheidung des
/// Projektinhabers und keine Ableitung.
pub fn zonen_cluster(
    miner: &[MinerRegistration],
    mindestbesetzung: usize,
    saat: &[u8; 32],
) -> Vec<MinerCluster> {
    let mut nach_zone: BTreeMap<GeoRegion, Vec<MinerRegistration>> = BTreeMap::new();
    for m in miner {
        nach_zone.entry(m.zone).or_default().push(*m);
    }

    let mut cluster = Vec::new();
    let mut sammel: Vec<MinerRegistration> = Vec::new();
    for (zone, mut miners) in nach_zone {
        if miners.len() >= mindestbesetzung {
            // ⚑ **Hier wird gemischt, und zwar hier** (Fund 142).
            deterministic_shuffle(&mut miners, &zonensaat(saat, zone));
            cluster.push(MinerCluster {
                miners,
                max_internal_latency: 0,
            });
        } else {
            sammel.extend(miners);
        }
    }
    if !sammel.is_empty() {
        // Das Sammelcluster ebenso: Es ist der Topf der dünnen Zonen
        // und wäre ungemischt genauso vorhersagbar wie eine Zone.
        deterministic_shuffle(&mut sammel, &sammelsaat(saat));
        cluster.push(MinerCluster {
            miners: sammel,
            max_internal_latency: 0,
        });
    }
    cluster
}

/// Der Seed einer Epoche, aus Blockhash und Epoche.
///
/// Benutzt denselben Trennstring und dieselbe kanonische Bytefolge wie
/// der VRF-Seed ([`seed_alpha`]), damit es **eine** Kodierung gibt und
/// nicht zwei. Der Wechsel auf den VRF-Seed ändert dann, woher `beta`
/// kommt, nicht, worüber gerechnet wird.
pub fn epochenseed(vorheriger_blockhash: &Hash, epoche: u64) -> [u8; 32] {
    Hash::sha256(&seed_alpha(vorheriger_blockhash, epoche)).0
}

/// Die Zuteilung einer Epoche aus einer fertigen Saat.
///
/// Filtert nach Registrierungsschluss und Hardware-Klasse, bildet je
/// Zone ein Cluster und teilt daraus Pods zu.
///
/// # ⚑ Fund 111: Es gab diese Regel zweimal
///
/// Bis zum 2026-09-01 stand in `myl_pod::zuteilung::plane_epoche` eine
/// **zweite, eigenständige** Herleitung derselben Zuteilung, und sie
/// stimmte in **keinem** der drei Schritte mit dieser überein: Sie
/// clusterte nach **gemessener Latenz** statt nach Zone, nahm die
/// **VRF-Saat** statt der Saat aus dem Blockhash und ließ nur die
/// Klassen aus ihren Parametern zu statt aller.
///
/// **Zwei Herleitungen derselben Konsensgröße laufen auseinander**,
/// dieselbe Lehre wie bei Fund 34. Die Regel steht deshalb nur noch
/// hier, und `plane_epoche` ruft sie. **Was die beiden Wege
/// unterscheidet, ist jetzt die Saat und sonst nichts**, und das ist die
/// offene Entscheidung, nicht ein zweiter Algorithmus.
pub fn zuteilung_aus_saat(
    register: &[MinerRegistration],
    epoche: u64,
    saat: &[u8; 32],
    shards_je_pod: u32,
    zugelassen: &[HardwareClass],
) -> Zuteilung {
    let geeignet = filter_miners(register, epoche, zugelassen);
    let cluster = zonen_cluster(&geeignet, pod_groesse(shards_je_pod), saat);
    assign_pods(&cluster, shards_je_pod, saat)
}

/// Die Zuteilung einer Epoche, abgeleitet aus dem Register, mit der Saat
/// aus dem Blockhash. Das ist der Weg, den die Kette geht.
///
/// **Alle Hardware-Klassen sind zugelassen**, solange es keine Regel
/// gibt, die eine ausschließt. Eine leere Liste zugelassener Klassen
/// ergäbe eine leere Zuteilung, und das sähe aus wie „keine Miner".
pub fn zuteilung_der_epoche(
    register: &[MinerRegistration],
    epoche: u64,
    vorheriger_blockhash: &Hash,
    shards_je_pod: u32,
) -> Zuteilung {
    zuteilung_aus_saat(
        register,
        epoche,
        &epochenseed(vorheriger_blockhash, epoche),
        shards_je_pod,
        &HardwareClass::all(),
    )
}

/// Findet den Pod zu einer Bündel-Kennung.
///
/// ⚑ **Der Weg vom Bündel zur Besetzung, und er fehlte** (Fund 109).
/// Ein `PoIBundle` nennt seinen Pod über eine `PodId`, die Zuteilung
/// nummeriert ihre Pods mit `pod_index`, und zwischen beiden gab es
/// keine Verbindung. [`myl_types::pod_kennung`] stellt sie her, indem
/// sie die Kennung aus Epoche und Platznummer **ableitet**.
///
/// `None`, wenn keine Platznummer dieser Epoche auf die Kennung führt.
/// **Das ist der Normalfall bei einem gefälschten Bündel** und kein
/// Fehler dieser Funktion.
pub fn pod_zu_kennung<'a>(
    zuteilung: &'a Zuteilung,
    epoche: u64,
    kennung: &myl_types::ids::PodId,
) -> Option<&'a crate::shard_assignment::Pod> {
    zuteilung
        .pods
        .iter()
        .find(|p| myl_types::pod_kennung(epoche, p.pod_index) == *kennung)
}

/// Der Saat eines Zonenclusters, aus der Epochensaat und der Zone.
///
/// **Je Zone eine eigene Saat.** Dieselbe Saat auf zwei Zonen anzuwenden
/// wäre nicht falsch, aber es verknüpfte zwei Permutationen, die nichts
/// miteinander zu tun haben; eine eigene Ableitung kostet einen Hash
/// und schließt die Frage.
fn zonensaat(saat: &[u8; 32], zone: GeoRegion) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"MYELITH_ZONENCLUSTER_v1");
    h.update(saat);
    // Die Zone als ein Byte ihrer kanonischen Ordnung. Das
    // Sammelcluster bekommt 0xFF, denn es gehört zu keiner Zone.
    h.update([zonenbyte(zone)]);
    let d = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&d);
    out
}

/// Die Saat des Sammelclusters.
fn sammelsaat(saat: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"MYELITH_ZONENCLUSTER_v1");
    h.update(saat);
    h.update([0xFFu8]);
    let d = h.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&d);
    out
}

/// Die Platznummer einer Zone in ihrer kanonischen Ordnung.
fn zonenbyte(zone: GeoRegion) -> u8 {
    GeoRegion::all()
        .iter()
        .position(|z| *z == zone)
        .unwrap_or(0xFE) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::ids::MinerId;

    fn miner(b: u8, zone: GeoRegion, ab: u64) -> MinerRegistration {
        MinerRegistration {
            miner_id: MinerId::new([b; 32]),
            hardware_class: HardwareClass::MediumGpu,
            registration_epoch: ab,
            zone,
            schluessel: myl_types::bls::BlsPublicKey([0; 48]),
            netzadresse: myl_types::latency_attest::PeerIdBytes([0; 32]),
        }
    }

    /// ⚑ **Die Saat bestimmt, wer mit wem in einem Pod sitzt**
    /// (Fund 142).
    ///
    /// Bis zum 2026-09-02 folgte die Besetzung der Eingabereihenfolge,
    /// und die Eingabe ist `state.miner.values()`, also nach `MinerId`
    /// sortiert. `MinerId` ist `SHA-256` über einen frei erzeugbaren
    /// Schlüssel: Wer eine Kennung an einer bestimmten Stelle haben
    /// wollte, erzeugte Schlüssel, bis eine dort landete. **Damit fiel
    /// die Annahme, auf der Stufe 1 steht.**
    ///
    /// Der Test hält fest, dass zwei Saaten **verschiedene**
    /// Besetzungen ergeben. Ohne das Mischen wären sie gleich, und
    /// genau daran fällt er.
    #[test]
    fn zwei_saaten_ergeben_zwei_besetzungen() {
        let register: Vec<MinerRegistration> =
            (0..24u8).map(|b| miner(b, GeoRegion::Europe, 0)).collect();

        let besetzung = |s: u8| -> Vec<Vec<u8>> {
            zuteilung_aus_saat(&register, 5, &saat(s), 4, &HardwareClass::all())
                .pods
                .iter()
                .map(|p| {
                    let mut m: Vec<u8> =
                        p.mitglieder().map(|x| x.miner_id.as_bytes()[0]).collect();
                    m.sort();
                    m
                })
                .collect()
        };

        let a = besetzung(1);
        let b = besetzung(2);
        assert_eq!(a.len(), 4, "vierundzwanzig Miner ergeben vier Pods zu sechs");
        assert_ne!(
            a, b,
            "zwei Saaten ergeben dieselbe Besetzung, es wird nicht gemischt"
        );

        // ⚑ **Und dieselbe Saat ergibt dasselbe**, sonst kämen zwei
        // Knoten zu verschiedenen Pods, und das wäre schlimmer als der
        // Fund.
        assert_eq!(a, besetzung(1), "dieselbe Saat ergibt eine andere Besetzung");

        // Verloren geht dabei niemand.
        let mut alle: Vec<u8> = a.iter().flatten().copied().collect();
        alle.sort();
        assert_eq!(alle, (0..24u8).collect::<Vec<u8>>(), "ein Miner ging verloren");
    }

    /// ⚑ **Was das Mischen nicht schließt: die erklärte Zone**
    /// (Fund 142, zweite Hälfte).
    ///
    /// Der Test hält die **bekannte Lücke** fest, damit sie niemand für
    /// geschlossen hält. Wer eine Zone angibt, in der sonst niemand
    /// steht, bekommt daraus ganze Pods, gemischt oder nicht: Mischen
    /// ordnet um, wer schon drin ist, und drin ist hier nur er.
    ///
    /// Sie zu schließen hieße, die Zone aus der Besetzung zu nehmen,
    /// und das kostet Latenz in der Pipeline. Das ist eine
    /// Entscheidung und keine Ableitung.
    #[test]
    fn eine_eigene_zone_bleibt_ein_hebel() {
        let mut register: Vec<MinerRegistration> =
            (0..60u8).map(|b| miner(b, GeoRegion::Europe, 0)).collect();
        for b in 100..106u8 {
            register.push(miner(b, GeoRegion::Oceania, 0));
        }
        let z = zuteilung_aus_saat(&register, 5, &saat(3), 4, &HardwareClass::all());
        let ganz_fremd = z
            .pods
            .iter()
            .filter(|p| p.mitglieder().all(|m| m.miner_id.as_bytes()[0] >= 100))
            .count();
        assert_eq!(
            ganz_fremd, 1,
            "sechs Anmeldungen in einer leeren Zone ergeben keinen ganzen Pod mehr, \
             dann ist die Lücke geschlossen und dieser Test veraltet"
        );
    }

    /// Eine Saat fuer die Tests des Clusterns.
    fn saat(b: u8) -> [u8; 32] {
        [b; 32]
    }

    fn hash(b: u8) -> Hash {
        Hash::sha256(&[b; 8])
    }

    /// Je Zone ein Cluster, und keine Zone geht verloren.
    #[test]
    fn je_zone_ein_cluster() {
        let register = vec![
            miner(1, GeoRegion::Europe, 0),
            miner(2, GeoRegion::Asia, 0),
            miner(3, GeoRegion::Europe, 0),
            miner(4, GeoRegion::NorthAmerica, 0),
        ];
        let cluster = zonen_cluster(&register, 1, &saat(1));
        assert_eq!(cluster.len(), 3);
        let gesamt: usize = cluster.iter().map(|c| c.miners.len()).sum();
        assert_eq!(gesamt, 4, "ein Miner ging verloren");
    }

    /// ⚑ **Die Zonen kommen in kanonischer Reihenfolge**, sonst kämen
    /// zwei Knoten zu verschiedenen Pods.
    #[test]
    fn die_zonen_kommen_kanonisch() {
        let a = zonen_cluster(
            &[miner(1, GeoRegion::Asia, 0), miner(2, GeoRegion::Europe, 0)],
            1,
            &saat(1),
        );
        let b = zonen_cluster(
            &[miner(2, GeoRegion::Europe, 0), miner(1, GeoRegion::Asia, 0)],
            1,
            &saat(1),
        );
        let zonen_a: Vec<GeoRegion> = a.iter().map(|c| c.miners[0].zone).collect();
        let zonen_b: Vec<GeoRegion> = b.iter().map(|c| c.miners[0].zone).collect();
        assert_eq!(zonen_a, zonen_b, "die Reihenfolge haengt an der Eingabe");
    }

    /// ⚑ **Es wird nichts gemessen, also steht dort auch keine Zahl.**
    #[test]
    fn keine_erfundene_latenz() {
        let cluster = zonen_cluster(&[miner(1, GeoRegion::Europe, 0)], 1, &saat(1));
        assert_eq!(cluster[0].max_internal_latency, 0);
    }

    /// ⚑ **Fund 112: Eine dünne Zone schließt ihre Miner nicht mehr
    /// aus.**
    ///
    /// Vier Miner in Europa, je einer in Asien und Nordamerika, und ein
    /// Pod braucht vier Mitglieder: Ohne das Sammelcluster trüge Europa
    /// einen Pod und die beiden anderen nichts, obwohl sie zusammen
    /// reichen.
    #[test]
    fn duenne_zonen_kommen_in_ein_sammelcluster() {
        let register = vec![
            miner(1, GeoRegion::Europe, 0),
            miner(2, GeoRegion::Europe, 0),
            miner(3, GeoRegion::Europe, 0),
            miner(4, GeoRegion::Europe, 0),
            miner(5, GeoRegion::Asia, 0),
            miner(6, GeoRegion::NorthAmerica, 0),
        ];
        let cluster = zonen_cluster(&register, 4, &saat(1));
        assert_eq!(cluster.len(), 2, "Europa und das Sammelcluster");
        assert_eq!(cluster[0].miners.len(), 4);
        assert_eq!(cluster[1].miners.len(), 2, "Asien und Nordamerika zusammen");
        let gesamt: usize = cluster.iter().map(|c| c.miners.len()).sum();
        assert_eq!(gesamt, 6, "ein Miner ging verloren");
    }

    /// Und das Sammelcluster steht am Ende, in kanonischer
    /// Zonenreihenfolge seiner Mitglieder: Sonst hinge die Zuteilung an
    /// der Eingabereihenfolge.
    #[test]
    fn das_sammelcluster_ist_kanonisch() {
        let vorwaerts = vec![
            miner(1, GeoRegion::Asia, 0),
            miner(2, GeoRegion::Europe, 0),
            miner(3, GeoRegion::NorthAmerica, 0),
        ];
        let mut rueckwaerts = vorwaerts.clone();
        rueckwaerts.reverse();
        let a = zonen_cluster(&vorwaerts, 4, &saat(1));
        let b = zonen_cluster(&rueckwaerts, 4, &saat(1));
        assert_eq!(a.len(), 1);
        let ids_a: Vec<_> = a[0].miners.iter().map(|m| m.miner_id).collect();
        let ids_b: Vec<_> = b[0].miners.iter().map(|m| m.miner_id).collect();
        assert_eq!(ids_a, ids_b, "die Reihenfolge haengt an der Eingabe");
    }

    /// ⚑ **Über drei Zonen entstehen Pods, ohne dass eine Zone allein
    /// die Mindestbesetzung trägt.**
    ///
    /// Zwölf Miner zu je vier in drei Zonen, `k = 4`, also sechs
    /// Mitglieder je Pod: Keine Zone trägt einen Pod, das Sammelcluster
    /// trägt zwei.
    #[test]
    fn drei_duenne_zonen_tragen_zusammen_zwei_pods() {
        let zonen = [GeoRegion::Europe, GeoRegion::NorthAmerica, GeoRegion::Asia];
        let register: Vec<MinerRegistration> = (1..=12u8)
            .map(|b| miner(b, zonen[(b as usize) % zonen.len()], 0))
            .collect();
        let z = zuteilung_der_epoche(&register, 5, &hash(1), 4);
        assert_eq!(z.pods.len(), 2, "zwölf Miner tragen zwei Pods zu sechs");
        assert!(z.ohne_pod.is_empty());
    }

    /// Dieselbe Eingabe ergibt dieselbe Zuteilung.
    #[test]
    fn die_zuteilung_ist_wiederholbar() {
        let register: Vec<MinerRegistration> =
            (1..=12).map(|b| miner(b, GeoRegion::Europe, 0)).collect();
        let a = zuteilung_der_epoche(&register, 5, &hash(1), 4);
        let b = zuteilung_der_epoche(&register, 5, &hash(1), 4);
        assert_eq!(a, b);
    }

    /// ⚑ Ein anderer Blockhash ergibt eine andere Zuteilung, sonst hinge
    /// sie nicht am Seed.
    #[test]
    fn ein_anderer_blockhash_teilt_anders_zu() {
        let register: Vec<MinerRegistration> =
            (1..=12).map(|b| miner(b, GeoRegion::Europe, 0)).collect();
        let a = zuteilung_der_epoche(&register, 5, &hash(1), 4);
        let b = zuteilung_der_epoche(&register, 5, &hash(2), 4);
        assert_ne!(a, b);
    }

    /// ⚑ **Ein Pod bleibt in seiner Zone.** Das ist der ganze Zweck:
    /// Nähe ohne gemessene Latenz.
    #[test]
    fn ein_pod_bleibt_in_seiner_zone() {
        // ⛑ **Die Eingabe ist absichtlich verschränkt.** Der erste
        // Entwurf listete erst zwölf Europäer, dann zwölf Asiaten, und
        // damit war der Test wertlos: `assign_pods` schneidet die
        // Cluster der Reihe nach in Pod-Portionen, also wären die Pods
        // auch **ohne** Zonengruppierung sortenrein gewesen. Der Test
        // prüfte seine eigenen Daten.
        //
        // Verschränkt kann nur die Gruppierung selbst sortenreine Pods
        // erzeugen.
        let mut register: Vec<MinerRegistration> = Vec::new();
        for i in 0..12u8 {
            register.push(miner(1 + i, GeoRegion::Europe, 0));
            register.push(miner(20 + i, GeoRegion::Asia, 0));
        }
        let z = zuteilung_der_epoche(&register, 5, &hash(3), 4);
        assert_eq!(z.pods.len(), 4, "es entstanden nicht vier Pods");
        for pod in &z.pods {
            let zonen: std::collections::BTreeSet<GeoRegion> =
                pod.mitglieder().map(|m| m.zone).collect();
            assert_eq!(zonen.len(), 1, "ein Pod streut ueber Zonen");
        }
    }

    /// ⚑ **Der Registrierungsschluss wirkt, und zwar durch
    /// `zuteilung_der_epoche` hindurch.**
    ///
    /// ⛑ **Der erste Entwurf dieses Tests rief `filter_miners`
    /// selbst** und prüfte damit das Werkzeug statt seines Gebrauchs:
    /// Baute man den Filteraufruf aus `zuteilung_der_epoche` aus, blieb
    /// er grün. Dieselbe Falle wie bei
    /// `jeder_gegenstand_bekommt_einen_eigenen_seed` am 2026-08-31.
    ///
    /// Er geht jetzt über die Zuteilung selbst: Sechs pünktliche Miner
    /// ergeben einen Pod, der siebte kommt zu spät und darf **nirgends**
    /// auftauchen, weder auf einer Position noch in der Reserve noch
    /// unter den Übrigen.
    #[test]
    fn der_registrierungsschluss_wirkt() {
        let mut register: Vec<MinerRegistration> =
            (1..=6).map(|b| miner(b, GeoRegion::Europe, 0)).collect();
        let zu_spaet = miner(7, GeoRegion::Europe, 5); // Schluss ist e-2 = 3
        register.push(zu_spaet);

        let z = zuteilung_der_epoche(&register, 5, &hash(4), 4);
        let alle: Vec<MinerId> = z
            .pods
            .iter()
            .flat_map(|p| p.mitglieder().map(|m| m.miner_id))
            .chain(z.ohne_pod.iter().map(|m| m.miner_id))
            .collect();
        assert_eq!(alle.len(), 6, "es sind nicht genau die sechs Puenktlichen");
        assert!(
            !alle.contains(&zu_spaet.miner_id),
            "der zu spaet Angemeldete kam durch"
        );
    }

    /// Ein leeres Register ergibt eine leere Zuteilung und keinen Absturz.
    #[test]
    fn ein_leeres_register_ergibt_nichts() {
        let z = zuteilung_der_epoche(&[], 5, &hash(1), 4);
        assert!(z.pods.is_empty());
        assert!(z.ohne_pod.is_empty());
    }

    /// Wer in keinen vollständigen Pod passt, steht in `ohne_pod` und
    /// wird damit zur Netzreserve.
    #[test]
    fn uebrige_stehen_in_ohne_pod() {
        let register: Vec<MinerRegistration> =
            (1..=8).map(|b| miner(b, GeoRegion::Europe, 0)).collect();
        // Pod-Größe bei vier Shards ist sechs; acht ergeben einen Pod
        // und zwei Übrige.
        let z = zuteilung_der_epoche(&register, 5, &hash(1), 4);
        assert_eq!(z.pods.len(), 1);
        assert_eq!(z.ohne_pod.len(), 2);
    }

    /// ⚑ **Fund 109 verdrahtet:** Die abgeleitete Kennung führt auf
    /// genau den Pod, der sie trägt.
    #[test]
    fn die_kennung_findet_ihren_pod() {
        let register: Vec<MinerRegistration> =
            (1..=12).map(|b| miner(b, GeoRegion::Europe, 0)).collect();
        let z = zuteilung_der_epoche(&register, 5, &hash(1), 4);
        assert_eq!(z.pods.len(), 2, "es entstanden nicht zwei Pods");
        for p in &z.pods {
            let k = myl_types::pod_kennung(5, p.pod_index);
            let gefunden = pod_zu_kennung(&z, 5, &k).expect("gefunden");
            assert_eq!(gefunden.pod_index, p.pod_index);
        }
    }

    /// ⚑ Eine Kennung aus einer anderen Epoche findet nichts. Ohne die
    /// Epochenbindung ließe sich ein altes Bündel unter neuer Besetzung
    /// abrechnen.
    #[test]
    fn eine_kennung_fremder_epoche_findet_nichts() {
        let register: Vec<MinerRegistration> =
            (1..=12).map(|b| miner(b, GeoRegion::Europe, 0)).collect();
        let z = zuteilung_der_epoche(&register, 5, &hash(1), 4);
        let alt = myl_types::pod_kennung(4, z.pods[0].pod_index);
        assert!(pod_zu_kennung(&z, 5, &alt).is_none());
    }

    /// Eine erfundene Kennung findet nichts, und das ist kein Fehler.
    #[test]
    fn eine_erfundene_kennung_findet_nichts() {
        let register: Vec<MinerRegistration> =
            (1..=12).map(|b| miner(b, GeoRegion::Europe, 0)).collect();
        let z = zuteilung_der_epoche(&register, 5, &hash(1), 4);
        let frei = myl_types::ids::PodId::new([0xAB; 32]);
        assert!(pod_zu_kennung(&z, 5, &frei).is_none());
    }
}
