//! **Knoten finden einander über die Bootstrap-Gegenstelle hinaus**
//! (Fund 587, 2026-10-22, gefunden im Netzlauf).
//!
//! Drei Knoten: A und C kennen nur B. Ohne die Behebung blieben beide bei
//! einer Gegenstelle; Gossip lief über B weiter, aber eine direkte Anfrage
//! von A an C (Zeitprobe, Datei, Stichprobe) fand keinen Weg. Mit ihr tragen
//! `identify` die Horchadressen in Kademlia ein und die Suche läuft an,
//! also kennen A und C einander nach kurzer Zeit unmittelbar.

use std::path::PathBuf;
use std::time::Duration;

use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::Knoten;

fn verzeichnis() -> PathBuf {
    let p = std::env::temp_dir().join(format!("myl-entdeckung-{}", std::process::id()));
    std::fs::create_dir_all(&p).expect("Ordner");
    p
}

/// Die Probekette, aber mit Suche im lokalen Netz.
static LOKALNETZ: myl_node::netz::Netz = myl_node::netz::Netz { lokale_suche: true, ..myl_node::netz::PROBEKETTE };

fn konfig(ordner: &std::path::Path, name: &str, bootstrap: Vec<String>) -> KnotenKonfig {
    KnotenKonfig {
        name: name.to_string(),
        schluesseldatei: ordner.join(format!("{name}.key")),
        protokollverzeichnis: ordner.join(name),
        protokoll_mb: myl_node::protokoll::GESAMT_HOECHSTENS / (1024 * 1024),
        kette_mb: 2048,
        archiv_ausfuhr: None,
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        bootstrap,
        rolle: Rolle::Teilnehmer,
        kettendatei: None,
        stimmsatzdatei_pfad: None,
        konsensschluesseldatei: None,
        nat: Default::default(),
        speicherorte: None,
        netz: &myl_node::netz::PROBEKETTE,
        modellzusage: None,
        zone: myl_types::node_metadata::GeoRegion::Europe,
        shard: None,
        aufnahme_sekunden: 1,
        beobachtung: None,
        tuer: None,
        ortsleitung: None,
        ortsausweis: None,
        pod: None,
        modellname: "sparrow-0.6b-int8".to_string(),
        kontoschluesseldatei: None,
        konto: None,
        testverkehr_sekunden: None,
        erzeugt_bloecke: false,
        blocktakt_ms: None,
        abzug: None,
        hintergrundpruefung: true,
        teilnehmer: Vec::new(),
    }
}

#[tokio::test]
async fn zwei_knoten_finden_einander_ueber_einen_dritten() {
    let ordner = verzeichnis();
    let mut b = Knoten::starten(konfig(&ordner, "b", vec![]), false).await.expect("B");
    let adresse = b.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let bootstrap = vec![format!("{}/p2p/{}", adresse, b.peer_id())];
    let mut a = Knoten::starten(konfig(&ordner, "a", bootstrap.clone()), false).await.expect("A");
    let mut c = Knoten::starten(konfig(&ordner, "c", bootstrap), false).await.expect("C");
    let ende = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        for k in [&mut a, &mut b, &mut c] {
            k.laufe_fuer(Duration::from_millis(50)).await;
        }
        if a.peers().await >= 2 && c.peers().await >= 2 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < ende,
            "A und C kennen einander nicht: A {} Gegenstellen, C {}",
            a.peers().await,
            c.peers().await
        );
    }
    let _ = std::fs::remove_dir_all(&ordner);
}

/// ⚑ **Ohne jede Angabe, im lokalen Netz** (mDNS, 2026-10-22): Zwei Knoten
/// ohne Bootstrap finden einander. Ohne die Suche bliebe jeder allein.
#[tokio::test]
async fn zwei_knoten_ohne_angabe_finden_einander_im_lokalen_netz() {
    let ordner = verzeichnis().join("lokal");
    let mut ka = konfig(&ordner, "a", vec![]);
    let mut kb = konfig(&ordner, "b", vec![]);
    ka.netz = &LOKALNETZ;
    kb.netz = &LOKALNETZ;
    // mDNS sucht ueber die Schnittstellen des Rechners, nicht ueber die
    // Rueckschleife; wer nur dort horcht, ist im lokalen Netz nicht da.
    ka.horchadressen = vec!["/ip4/0.0.0.0/tcp/0".to_string()];
    kb.horchadressen = vec!["/ip4/0.0.0.0/tcp/0".to_string()];
    let mut a = Knoten::starten(ka, false).await.expect("A");
    let mut b = Knoten::starten(kb, false).await.expect("B");
    let ende = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        for k in [&mut a, &mut b] {
            k.laufe_fuer(Duration::from_millis(50)).await;
        }
        if a.peers().await >= 1 && b.peers().await >= 1 {
            break;
        }
        assert!(tokio::time::Instant::now() < ende, "ohne Angabe fand keiner den anderen");
    }
    let _ = std::fs::remove_dir_all(&ordner);
}

/// Die Einstiegsliste des Netzes kommt zu den eigenen Angaben, ohne Doppelte.
#[test]
fn die_einstiegsliste_ergaenzt_die_eigenen_angaben() {
    static MIT_TUER: myl_node::netz::Netz = myl_node::netz::Netz {
        einstieg: &["/ip4/192.0.2.1/tcp/4150/p2p/a", "/ip4/192.0.2.2/tcp/4150/p2p/b"],
        ..myl_node::netz::PROBEKETTE
    };
    let mut k = konfig(&verzeichnis(), "e", vec!["/ip4/192.0.2.2/tcp/4150/p2p/b".into(), "/ip4/192.0.2.3/tcp/4150/p2p/c".into()]);
    k.netz = &MIT_TUER;
    assert_eq!(
        k.einstiegsliste(),
        ["/ip4/192.0.2.2/tcp/4150/p2p/b", "/ip4/192.0.2.3/tcp/4150/p2p/c", "/ip4/192.0.2.1/tcp/4150/p2p/a"]
    );
    assert!(!k.netz().lokale_suche, "die Probekette sucht nicht lokal");
}
