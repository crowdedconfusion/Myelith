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
pub fn zonen_cluster(miner: &[MinerRegistration], mindestbesetzung: usize) -> Vec<MinerCluster> {
    let mut nach_zone: BTreeMap<GeoRegion, Vec<MinerRegistration>> = BTreeMap::new();
    for m in miner {
        nach_zone.entry(m.zone).or_default().push(*m);
    }

    let mut cluster = Vec::new();
    let mut sammel: Vec<MinerRegistration> = Vec::new();
    for (_, miners) in nach_zone {
        if miners.len() >= mindestbesetzung {
            cluster.push(MinerCluster {
                miners,
                max_internal_latency: 0,
            });
        } else {
            sammel.extend(miners);
        }
    }
    if !sammel.is_empty() {
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
    let cluster = zonen_cluster(&geeignet, pod_groesse(shards_je_pod));
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
        let cluster = zonen_cluster(&register, 1);
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
        );
        let b = zonen_cluster(
            &[miner(2, GeoRegion::Europe, 0), miner(1, GeoRegion::Asia, 0)],
            1,
        );
        let zonen_a: Vec<GeoRegion> = a.iter().map(|c| c.miners[0].zone).collect();
        let zonen_b: Vec<GeoRegion> = b.iter().map(|c| c.miners[0].zone).collect();
        assert_eq!(zonen_a, zonen_b, "die Reihenfolge haengt an der Eingabe");
    }

    /// ⚑ **Es wird nichts gemessen, also steht dort auch keine Zahl.**
    #[test]
    fn keine_erfundene_latenz() {
        let cluster = zonen_cluster(&[miner(1, GeoRegion::Europe, 0)], 1);
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
        let cluster = zonen_cluster(&register, 4);
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
        let a = zonen_cluster(&vorwaerts, 4);
        let b = zonen_cluster(&rueckwaerts, 4);
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
