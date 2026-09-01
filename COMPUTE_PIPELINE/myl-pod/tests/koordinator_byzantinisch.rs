//! Was ein Pod-Koordinator anrichten kann (Punkt 4.3).
//!
//! Der Koordinator ist die einzige Stelle im Pod, die **für alle
//! spricht**: Er sammelt die Segmente, bildet die Merkle-Wurzel, beziffert
//! die beanspruchte Arbeit und reicht das PoI-Bündel ein. Kap. 5.5 führt
//! ihn deshalb mit eigenem Zusatz-Stake und 100 % Slash bei falscher
//! Aggregation.
//!
//! Geprüft wird hier die Kette **Pod → Konsens**: Was der Koordinator
//! baut, muss der Konsens prüfen können, und was er fälscht, muss dort
//! auffallen.

use myl_consensus::poi::{bundle_message, verify_bundle_signature, PodMembership, PoIError};
use myl_types::bls::{aggregate_signatures, BlsSecretKey, BlsSignature};
use myl_types::ids::{EpochId, MinerId, PodId, SegmentId};
use myl_types::{segments_root, PoIBundle};

mod artefakte;

/// Segment-Ids zu Zeugnissen, mit einer aus der Id abgeleiteten
/// Spurwurzel.
///
/// ⚑ Seit Fund 100 bezeugt die Bündelwurzel `Id ‖ Spurwurzel`, nicht
/// mehr die bloße Id. Für diese Tests ist der Inhalt der Spur
/// gleichgültig, ihre Anwesenheit nicht.
fn zeugnisse(ids: &[SegmentId]) -> Vec<myl_types::Segmentzeugnis> {
    ids.iter()
        .map(|id| myl_types::Segmentzeugnis {
            id: *id,
            spurwurzel: myl_types::spurwurzel(&[*id.as_bytes()]).expect("Wurzel"),
        })
        .collect()
}


fn miner(b: u8) -> MinerId {
    MinerId::new([b; 32])
}
fn sk(b: u8) -> BlsSecretKey {
    BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("key_gen")
}
fn segment(b: u8) -> SegmentId {
    SegmentId::new([b; 32])
}

/// Drei Mitglieder mit Besitznachweis, wie `PodMembership` sie verlangt.
fn membership(epoch: EpochId, pod: PodId) -> PodMembership {
    let mitglieder: Vec<_> = (1..=3u8)
        .map(|i| {
            let k = sk(i);
            let pk = k.public_key().expect("pk");
            let pop = k.prove_possession().expect("pop");
            (miner(i), pk, pop)
        })
        .collect();
    PodMembership::new(epoch, pod, miner(1), mitglieder).expect("Mitgliedschaft")
}

/// Ein Bündel, das die Mitglieder **korrekt** unterschrieben haben.
fn ehrliches_buendel(epoch: EpochId, pod: PodId, vtfe: u64) -> PoIBundle {
    let ids = [segment(9), segment(10)];
    let root = segments_root(&zeugnisse(&ids)).expect("Wurzel");
    let vorlage = PoIBundle {
        epoch,
        pod,
        segments_root: root,
        vtfe_claimed: vtfe,
        aggregate_sig: BlsSignature([0u8; 96]),
        segmente: 1,
    };
    let msg = bundle_message(&vorlage);
    let sigs: Vec<BlsSignature> = (1..=3u8).map(|i| sk(i).sign(&msg).expect("sign")).collect();
    let agg = aggregate_signatures(&sigs).expect("agg");
    PoIBundle {
        aggregate_sig: BlsSignature(agg.0),
        ..vorlage
    }
}

// ---------------------------------------------------------------------
// Gegenprobe
// ---------------------------------------------------------------------

/// **Das ehrliche Bündel wird angenommen.**
///
/// Ohne diese Probe wäre jeder Test darunter wertlos: Eine Prüfung, die
/// alles ablehnt, lehnt auch jede Fälschung ab.
#[test]
fn das_ehrliche_buendel_wird_angenommen() {
    let (e, p) = (EpochId(4), PodId::new([7u8; 32]));
    let m = membership(e, p);
    assert!(verify_bundle_signature(&ehrliches_buendel(e, p, 1_000), &m).is_ok());
}

// ---------------------------------------------------------------------
// Angriffe des Koordinators
// ---------------------------------------------------------------------

/// **Angriff: die beanspruchte Arbeit nachträglich erhöhen.**
///
/// Der wichtigste Einzelangriff des Koordinators: Er sammelt die
/// Unterschriften über ein ehrliches Bündel und schreibt danach eine
/// größere Zahl in `vtfe_claimed`.
///
/// Abgewehrt, weil `vtfe_claimed` **mitsigniert** ist. Stünde es nicht in
/// der Signierbotschaft, wäre es die einzige Zahl des Protokolls, die
/// jemand ohne Zustimmung ändern kann, und sie ist zugleich die, an der
/// die Vergütung hängt.
#[test]
fn eine_nachtraeglich_erhoehte_arbeitsmenge_wird_abgelehnt() {
    let (e, p) = (EpochId(4), PodId::new([7u8; 32]));
    let m = membership(e, p);
    let ehrlich = ehrliches_buendel(e, p, 1_000);

    for gefaelscht in [1_001u64, 10_000, u64::MAX, 999] {
        let manipuliert = PoIBundle {
            vtfe_claimed: gefaelscht,
            ..ehrlich.clone()
        };
        assert_eq!(
            verify_bundle_signature(&manipuliert, &m),
            Err(PoIError::InvalidAggregateSignature),
            "vtfe_claimed = {gefaelscht} muss auffallen"
        );
    }
}

/// **Angriff: Segmente hinzudichten oder weglassen.**
///
/// Die Merkle-Wurzel ist mitsigniert; jede andere Segmentmenge ergibt
/// eine andere Wurzel und damit eine andere Botschaft.
#[test]
fn eine_veraenderte_segmentmenge_wird_abgelehnt() {
    let (e, p) = (EpochId(4), PodId::new([7u8; 32]));
    let m = membership(e, p);
    let ehrlich = ehrliches_buendel(e, p, 1_000);

    for ids in [
        vec![segment(9), segment(10), segment(11)], // hinzugedichtet
        vec![segment(9)],                            // weggelassen
        vec![segment(10), segment(9)],               // umsortiert
    ] {
        let manipuliert = PoIBundle {
            segments_root: segments_root(&zeugnisse(&ids)).expect("Wurzel"),
            ..ehrlich.clone()
        };
        assert_eq!(
            verify_bundle_signature(&manipuliert, &m),
            Err(PoIError::InvalidAggregateSignature)
        );
    }
}

/// **Angriff: ein Bündel aus einer anderen Epoche oder für einen anderen
/// Pod einreichen.**
#[test]
fn ein_fremdes_buendel_wird_abgelehnt() {
    let (e, p) = (EpochId(4), PodId::new([7u8; 32]));
    let m = membership(e, p);
    let ehrlich = ehrliches_buendel(e, p, 1_000);

    let andere_epoche = PoIBundle { epoch: EpochId(5), ..ehrlich.clone() };
    assert!(matches!(
        verify_bundle_signature(&andere_epoche, &m),
        Err(PoIError::EpochMismatch { .. })
    ));

    let anderer_pod = PoIBundle { pod: PodId::new([8u8; 32]), ..ehrlich };
    assert_eq!(verify_bundle_signature(&anderer_pod, &m), Err(PoIError::PodMismatch));
}

/// **Angriff: das Bündel allein unterschreiben.**
///
/// Der Koordinator ist selbst Mitglied. Unterschreibt nur er, fehlen die
/// übrigen im Aggregat, und `fast_aggregate_verify` gegen die volle
/// Mitgliedermenge scheitert.
#[test]
fn ein_allein_unterschriebenes_buendel_wird_abgelehnt() {
    let (e, p) = (EpochId(4), PodId::new([7u8; 32]));
    let m = membership(e, p);
    let ids = [segment(9), segment(10)];
    let vorlage = PoIBundle {
        epoch: e,
        pod: p,
        segments_root: segments_root(&zeugnisse(&ids)).expect("Wurzel"),
        vtfe_claimed: 1_000,
        aggregate_sig: BlsSignature([0u8; 96]),
        segmente: 1,
    };
    let msg = bundle_message(&vorlage);

    // Nur der Koordinator.
    let allein = sk(1).sign(&msg).expect("sign");
    let nur_einer = PoIBundle {
        aggregate_sig: BlsSignature(aggregate_signatures(&[allein]).expect("agg").0),
        ..vorlage.clone()
    };
    assert_eq!(
        verify_bundle_signature(&nur_einer, &m),
        Err(PoIError::InvalidAggregateSignature)
    );

    // Und zwei von drei genügen ebenfalls nicht: Das Aggregat gilt gegen
    // **alle** Mitglieder, nicht gegen eine Mehrheit.
    let zwei: Vec<BlsSignature> = (1..=2u8).map(|i| sk(i).sign(&msg).expect("sign")).collect();
    let nur_zwei = PoIBundle {
        aggregate_sig: BlsSignature(aggregate_signatures(&zwei).expect("agg").0),
        ..vorlage
    };
    assert_eq!(
        verify_bundle_signature(&nur_zwei, &m),
        Err(PoIError::InvalidAggregateSignature)
    );
}

// ---------------------------------------------------------------------
// Die Naht zwischen Pod und Konsens
// ---------------------------------------------------------------------

/// **⚑ Fund 52: Der Pod baut ein Bündel, das der Konsens nicht prüfen
/// kann.**
///
/// `Coordinator::build_poi_bundle` aggregiert die **Übergangs-Signaturen**
/// der Segmente (`CompletedSegment::signatures`), also Unterschriften
/// über `DST_SHARD_TRANSITION ‖ Rolle ‖ Borsh(TransitionSig)`.
///
/// `myl_consensus::verify_bundle_signature` prüft dagegen gegen
/// `bundle_message(bundle)`, also über
/// `DST_POI_BUNDLE ‖ epoch ‖ pod ‖ segments_root ‖ vtfe_claimed`.
///
/// **Zwei verschiedene Botschaften.** Ein Bündel, das der Koordinator
/// baut, kann damit niemals verifizieren — nicht weil es falsch wäre,
/// sondern weil die beiden Seiten über verschiedene Dinge reden.
///
/// **Die Richtung ist die gute:** Es wird abgelehnt, nicht angenommen.
/// Ein PoI-Bündel würde nie durchgehen, und niemand bekäme Vergütung,
/// die ihm nicht zusteht. Aber es hieße auch, dass **überhaupt niemand**
/// Vergütung bekommt: Der Pfad ist nicht bloß ungeprüft, er ist
/// unbenutzbar.
///
/// **Warum es niemandem aufgefallen ist:** `myl-pod` hing bis heute nicht
/// an `myl-consensus`, und `myl-consensus` nicht an `myl-pod`. Beide
/// Seiten sind für sich getestet, die Naht dazwischen hat nie jemand
/// zusammengesteckt. Genau der Fall, für den die Härtungsschleife
/// geschrieben wurde: „fast jeder schwere Fund saß nicht *in* einer
/// Komponente, sondern **zwischen zweien**."
///
/// Dieser Test hält den Zustand als Tatsache fest. **Schlägt er eines
/// Tages fehl, hat jemand die Naht geschlossen**, und dann gehört diese
/// Doku nachgezogen.
#[test]
fn fund_52_die_uebergangssignaturen_verifizieren_das_buendel_nicht() {
    let (e, p) = (EpochId(4), PodId::new([7u8; 32]));
    let m = membership(e, p);
    let ids = [segment(9), segment(10)];
    let vorlage = PoIBundle {
        epoch: e,
        pod: p,
        segments_root: segments_root(&zeugnisse(&ids)).expect("Wurzel"),
        vtfe_claimed: 1_000,
        aggregate_sig: BlsSignature([0u8; 96]),
        segmente: 1,
    };

    // So baut `Coordinator::build_poi_bundle` das Aggregat: über die
    // Übergangs-Signaturen der Segmente, nicht über die Bündelbotschaft.
    let uebergang = myl_pod::trace::TransitionSig {
        segment_id: segment(9),
        shard_index: 0,
        position: 0,
        prev_hash: myl_pod::trace::ZERO_HASH,
        next_hash: [1u8; 32],
    };
    let transitions: Vec<BlsSignature> = (1..=3u8)
        .map(|i| uebergang.sign(&sk(i)).expect("sign"))
        .collect();
    let wie_der_pod = PoIBundle {
        aggregate_sig: BlsSignature(aggregate_signatures(&transitions).expect("agg").0),
        ..vorlage.clone()
    };

    assert_eq!(
        verify_bundle_signature(&wie_der_pod, &m),
        Err(PoIError::InvalidAggregateSignature),
        "wenn das durchgeht, ist die Naht geschlossen und die Doku dieses Tests veraltet"
    );

    // Zum Vergleich: über die richtige Botschaft signiert, geht es durch.
    // Der Unterschied liegt allein in der Botschaft, nicht in den
    // Schlüsseln oder der Aggregation.
    let msg = bundle_message(&vorlage);
    let richtig: Vec<BlsSignature> = (1..=3u8).map(|i| sk(i).sign(&msg).expect("sign")).collect();
    let wie_es_sein_muesste = PoIBundle {
        aggregate_sig: BlsSignature(aggregate_signatures(&richtig).expect("agg").0),
        ..vorlage
    };
    assert!(
        verify_bundle_signature(&wie_es_sein_muesste, &m).is_ok(),
        "dieselben Schlüssel, dieselbe Aggregation, andere Botschaft"
    );
}

/// **Die Botschaft des Pods ist bitgleich mit der des Konsenses.**
///
/// `Coordinator::signierbotschaft` und
/// `myl_consensus::poi::bundle_message` kodieren dasselbe. Sie stehen an
/// zwei Orten, weil `myl-pod` nicht an `myl-consensus` hängen soll, und
/// **genau deshalb braucht es diesen Test**: Eine Dublette der Kodierung
/// läuft irgendwann auseinander, und dann ist der Streit nicht mehr
/// entscheidbar.
#[test]
fn die_signierbotschaft_des_pods_ist_die_des_konsenses() {
    let mut w: u64 = 0x243F_6A88;
    for _ in 0..1_000 {
        w ^= w << 13;
        w ^= w >> 7;
        w ^= w << 17;
        let ids = [segment((w & 0xff) as u8), segment(((w >> 8) & 0xff) as u8)];
        let b = PoIBundle {
            epoch: EpochId(w % 10_000),
            pod: PodId::new([((w >> 16) & 0xff) as u8; 32]),
            segments_root: segments_root(&zeugnisse(&ids)).expect("Wurzel"),
            vtfe_claimed: w,
            aggregate_sig: BlsSignature([0u8; 96]),
            segmente: 1,
        };
        assert_eq!(
            myl_pod::coordinator::Coordinator::signierbotschaft(&b),
            bundle_message(&b),
            "die beiden Kodierungen sind auseinandergelaufen"
        );
    }
}

/// **Fund 52 geschlossen: Das unterschriebene Bündel verifiziert.**
///
/// Der Test fährt einen echten Pod gegen die INTEGER_LLM-Runtime, lässt
/// ihn ein Bündel bauen **und unterschreiben**, und legt es dem Konsens
/// vor. Das ist die Naht, die vorher nicht zusammenpasste.
///
/// **Braucht Artefakte** und überspringt sich ohne sie — wie
/// `pod_e2e.rs`. Ohne Modell gibt es keine Segmente, ohne Segmente kein
/// Bündel, und ein Bündel aus erfundenen Segmenten bewiese nichts über
/// die Naht.
#[test]
fn fund_52_das_unterschriebene_buendel_verifiziert() {
    use std::sync::Arc;

    let dir = {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let mut p = std::path::PathBuf::from(manifest);
        p.push("..");
        p.push("..");
        p.push("INTEGER_LLM");
        p.push("artifacts");
        p.push("qwen2.5-0.5b");
        p
    };
    if !artefakte::vorhanden(&dir) {
        return;
    }

    let model = Arc::new(integer_llm_runtime::loader::load_model(&dir).expect("Modell"));
    let epoch = EpochId(4);
    let pod = PodId::new([7u8; 32]);

    // Vier Shards, je einer je Layer-Viertel; Schlüssel wie in
    // `membership`, damit die Mitgliedermenge zusammenpasst.
    let layers = model.num_layers;
    let k = 4usize;
    let je = layers / k;
    let shards: Vec<Arc<myl_pod::shard::ShardNode>> = (0..k)
        .map(|i| {
            Arc::new(myl_pod::shard::ShardNode::new(
                i,
                i * je,
                if i == k - 1 { layers } else { (i + 1) * je },
                i == 0,
                i == k - 1,
                model.clone(),
                sk((i + 1) as u8),
                8,
            ))
        })
        .collect();

    let mut koordinator = myl_pod::coordinator::Coordinator::new(pod, epoch, shards, 250);
    let _ = koordinator.run_prompt(1, &[9707, 11], 2);
    assert!(
        !koordinator.completed_segments().is_empty(),
        "der Lauf muss Segmente erzeugen"
    );

    // Die Mitgliedschaft aus denselben vier Schlüsseln.
    let mitglieder: Vec<_> = (1..=k as u8)
        .map(|i| {
            let key = sk(i);
            (
                miner(i),
                key.public_key().expect("pk"),
                key.prove_possession().expect("pop"),
            )
        })
        .collect();
    let m = PodMembership::new(epoch, pod, miner(1), mitglieder).expect("Mitgliedschaft");

    // **Vorher:** das unsignierte Bündel wird abgelehnt.
    let unsigniert = koordinator.build_poi_bundle().expect("Bündel");
    assert_eq!(
        verify_bundle_signature(&unsigniert, &m),
        Err(PoIError::InvalidAggregateSignature),
        "das Aggregat über Übergangs-Signaturen darf nicht gelten"
    );

    // **Nachher:** das unterschriebene verifiziert.
    let signiert = koordinator
        .build_signed_poi_bundle()
        .expect("Mitglieder unterschreiben");
    assert!(
        verify_bundle_signature(&signiert, &m).is_ok(),
        "das unterschriebene Bündel muss gelten"
    );

    // Und der Anspruch ist derselbe: Die Runde ändert nur die Signatur.
    assert_eq!(signiert.vtfe_claimed, unsigniert.vtfe_claimed);
    assert_eq!(signiert.segments_root, unsigniert.segments_root);

    // **Die Gegenprobe zur Reihenfolge:** Wer nach dem Sammeln den
    // Anspruch erhöht, verliert die Gültigkeit.
    let nachtraeglich = PoIBundle {
        vtfe_claimed: signiert.vtfe_claimed + 1,
        ..signiert
    };
    assert_eq!(
        verify_bundle_signature(&nachtraeglich, &m),
        Err(PoIError::InvalidAggregateSignature)
    );
}

/// **Ein Mitglied, das den Anspruch nicht nachrechnen kann,
/// unterschreibt nicht.**
///
/// Eine Unterschrift ohne Prüfung ist keine Zustimmung, sondern eine
/// Anwesenheitsnotiz. Kap. 5.5 belegt falsche PoI-Aggregation mit 100 %
/// Slash des Koordinators, und das setzt voraus, dass die Mitglieder
/// etwas anderes bezeugen als „ich war dabei".
#[test]
fn ein_mitglied_unterschreibt_keinen_falschen_anspruch() {
    use std::sync::Arc;

    let dir = {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let mut p = std::path::PathBuf::from(manifest);
        p.push("..");
        p.push("..");
        p.push("INTEGER_LLM");
        p.push("artifacts");
        p.push("qwen2.5-0.5b");
        p
    };
    if !artefakte::vorhanden(&dir) {
        return;
    }
    let model = Arc::new(integer_llm_runtime::loader::load_model(&dir).expect("Modell"));
    let shard = myl_pod::shard::ShardNode::new(
        0,
        0,
        6,
        true,
        false,
        model,
        sk(1),
        8,
    );
    let zuschnitte = vec![shard.zuschnitt()];
    let botschaft = b"egal, es kommt gar nicht so weit".to_vec();

    // Der richtige Anspruch für 10 Segmente.
    let profil = shard.modell_profil();
    let richtig = myl_tokenomics::vtfe_gutschrift(&profil, &shard.zuschnitt(), 10).unwrap();
    assert!(shard
        .signiere_buendel(&botschaft, richtig, 10, &zuschnitte)
        .is_ok());

    // Jeder andere Anspruch wird verweigert, nach oben wie nach unten.
    for falsch in [richtig + 1, richtig.saturating_sub(1), richtig * 2, 0] {
        if falsch == richtig {
            continue;
        }
        assert!(
            shard
                .signiere_buendel(&botschaft, falsch, 10, &zuschnitte)
                .is_err(),
            "Anspruch {falsch} statt {richtig} muss verweigert werden"
        );
    }
}
