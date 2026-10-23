//! **Nachholen von mehreren Quellen zugleich** (2026-10-22).
//!
//! Bis dahin lief eine Nachforderung zur Zeit, bei einer Gegenstelle. Über
//! eine Leitung mit 200 ms und 5 % Verlust kamen so rund 75 Blöcke je
//! Sekunde an; mehrere Verbindungen addieren sich. Geprüft wird hier der Weg,
//! nicht der Durchsatz (der hängt an der Leitung und steht im Lauf mit
//! getrennten Prozessen): Delta fragt bei zwei Gegenstellen zugleich, setzt
//! Lieferungen in jeder Reihenfolge zusammen und endet mit derselben Wurzel.

use std::path::{Path, PathBuf};
use std::time::Duration;

use myl_consensus::block::{Anweisung, Transaktion};
use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::Knoten;

fn ordner() -> PathBuf {
    let p = std::env::temp_dir().join(format!("myl-nachholen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("Ordner");
    p
}

fn konfig(o: &Path, name: &str, bootstrap: Vec<String>) -> KnotenKonfig {
    let d = o.join(name);
    std::fs::create_dir_all(&d).expect("Ordner");
    KnotenKonfig {
        name: name.to_string(),
        schluesseldatei: d.join("knoten.key"),
        protokollverzeichnis: d.clone(),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        bootstrap,
        rolle: Rolle::Teilnehmer,
        kettendatei: Some(d.join("kette.dat")),
        aufnahme_sekunden: 1,
        beobachtung: None,
        tuer: None,
        ..KnotenKonfig::default()
    }
}

async fn treiben(knoten: &mut [&mut Knoten], frist: Duration, fertig: impl Fn(&[&mut Knoten]) -> bool) -> bool {
    let ende = tokio::time::Instant::now() + frist;
    while tokio::time::Instant::now() < ende {
        if fertig(knoten) {
            return true;
        }
        // Wie im Betrieb baut der Erzeuger weiter; daran hört ein Knoten,
        // dass es eine höhere Kette gibt.
        knoten[0].erzeuge_block().await;
        for k in knoten.iter_mut() {
            k.laufe_fuer(Duration::from_millis(30)).await;
        }
    }
    fertig(knoten)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ein_neuling_holt_bei_mehreren_zugleich_nach() {
    let o = ordner();
    let mut alpha = Knoten::starten(KnotenKonfig { erzeugt_bloecke: true, ..konfig(&o, "alpha", vec![]) }, false)
        .await
        .expect("Alpha");
    for i in 0..2_500u64 {
        if i % 11 == 0 {
            let n = (i % 8) as u8;
            let nonce = alpha.kette().zustand().account(&myl_node::kette::probekonto(n)).nonce;
            let tx = Transaktion::signiere(&alpha.kette().netzwert(), &myl_node::kette::probeschluessel(n), nonce, Anweisung::Burn {
                betrag: 1_000 + i,
            })
            .expect("signieren");
            alpha.kette_mut().aufnehmen(tx);
        }
        alpha.erzeuge_block().await;
    }
    let a = alpha.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let einstieg_a = format!("{}/p2p/{}", a, alpha.peer_id());
    let mut beta = Knoten::starten(konfig(&o, "beta", vec![einstieg_a.clone()]), false).await.expect("Beta");
    assert!(
        treiben(&mut [&mut alpha, &mut beta], Duration::from_secs(60), |k| k[1].kette().hoehe() == k[0].kette().hoehe()).await,
        "Beta holte nicht auf"
    );
    let b = beta.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let einstieg_b = format!("{}/p2p/{}", b, beta.peer_id());

    let mut delta = Knoten::starten(konfig(&o, "delta", vec![einstieg_a, einstieg_b]), false).await.expect("Delta");
    assert!(
        treiben(&mut [&mut alpha, &mut beta, &mut delta], Duration::from_secs(60), |k| {
            k[2].kette().hoehe() == k[0].kette().hoehe() && k[1].kette().hoehe() == k[0].kette().hoehe()
        })
        .await,
        "Delta holte nicht auf: {} von {}",
        delta.kette().hoehe(),
        alpha.kette().hoehe()
    );
    assert_eq!(delta.kette().zustandswurzel(), alpha.kette().zustandswurzel());

    // Bei zwei Gegenstellen, und mehr als eine Nachforderung zugleich.
    let zeilen: Vec<String> = std::fs::read_to_string(delta.protokollpfad())
        .unwrap_or_default()
        .lines()
        .filter(|z| z.contains("\"art\":\"nachschub_angefordert\""))
        .map(str::to_string)
        .collect();
    let bei: std::collections::BTreeSet<&str> =
        zeilen.iter().filter_map(|z| z.split("\"bei\":\"").nth(1)?.split('"').next()).collect();
    assert!(bei.len() >= 2, "nur bei {bei:?} nachgefordert");
    assert!(zeilen.iter().any(|z| !z.contains("\"laufend\":1,") && !z.contains("\"laufend\":1}")), "nie mehr als eine zugleich");
    let _ = std::fs::remove_dir_all(&o);
}
