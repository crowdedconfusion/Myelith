//! **Der Zustandsabzug über das Netz** (2026-10-22): Ein Neuling beginnt bei
//! einer Epochengrenze statt bei Block 0, und die Hintergrundprüfung rechnet
//! trotzdem bis dorthin nach.
//!
//! Alpha erzeugt Blöcke mit Transaktionen über die erste Epochengrenze
//! hinaus und legt dort einen Abzug ab. Beta kennt den Anker (Höhe und
//! Blockhash, wie ihn Alphas Protokoll meldet), holt den Abzug, holt die
//! Spitze ein, und seine Hintergrundprüfung bestätigt die Basis. Nach einem
//! Neustart steht Beta ohne Netz auf derselben Höhe und Wurzel. Gamma hat
//! einen Anker mit falschem Hash: Er übernimmt nichts.

use std::path::{Path, PathBuf};
use std::time::Duration;

use myl_consensus::block::{Anweisung, Transaktion, BLOECKE_JE_EPOCHE};
use myl_node::abzug::Anker;
use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::Knoten;

fn ordner(marke: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("myl-abzug-{marke}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("Ordner");
    p
}

fn konfig(o: &Path, name: &str, bootstrap: Vec<String>, abzug: Option<Anker>) -> KnotenKonfig {
    let d = o.join(name);
    std::fs::create_dir_all(&d).expect("Ordner");
    KnotenKonfig {
        name: name.to_string(),
        schluesseldatei: d.join("knoten.key"),
        protokollverzeichnis: d.clone(),
        protokoll_mb: myl_node::protokoll::GESAMT_HOECHSTENS / (1024 * 1024),
        kette_mb: 2048,
        archiv_ausfuhr: None,
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        bootstrap,
        rolle: Rolle::Teilnehmer,
        kettendatei: Some(d.join("kette.dat")),
        aufnahme_sekunden: 1,
        beobachtung: None,
        tuer: None,
        abzug,
        ..KnotenKonfig::default()
    }
}

/// So viele Zeilen einer Art im Protokoll eines Knotens.
fn im_protokoll(knoten: &Knoten, art: &str) -> Vec<String> {
    std::fs::read_to_string(knoten.protokollpfad())
        .unwrap_or_default()
        .lines()
        .filter(|z| z.contains(&format!("\"art\":\"{art}\"")))
        .map(str::to_string)
        .collect()
}

/// Treibt alle Knoten reihum, bis die Bedingung gilt oder die Frist um ist.
/// Der erste baut dabei weiter, wie ein Erzeuger im Betrieb: Ein Knoten
/// erfährt von einer höheren Kette über neue Blöcke.
async fn bis(knoten: &mut [&mut Knoten], frist: Duration, mut fertig: impl FnMut(&[&mut Knoten]) -> bool) -> bool {
    let ende = tokio::time::Instant::now() + frist;
    while tokio::time::Instant::now() < ende {
        if fertig(knoten) {
            return true;
        }
        knoten[0].erzeuge_block().await;
        for k in knoten.iter_mut() {
            k.laufe_fuer(Duration::from_millis(30)).await;
        }
    }
    fertig(knoten)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ein_neuling_beginnt_beim_abzug_und_prueft_im_hintergrund() {
    let o = ordner("weg");
    let mut alpha = Knoten::starten(KnotenKonfig { erzeugt_bloecke: true, ..konfig(&o, "alpha", vec![], None) }, false)
        .await
        .expect("Alpha");
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let einstieg = format!("{}/p2p/{}", adresse, alpha.peer_id());

    // Über die erste Epochengrenze, mit Transaktionen, damit die Wurzel sich
    // bewegt und ein falscher Zustand auffiele.
    let ziel = BLOECKE_JE_EPOCHE + 40;
    for i in 0..ziel {
        if i % 9 == 0 {
            let n = (i % 8) as u8;
            let konto = myl_node::kette::probekonto(n);
            let nonce = alpha.kette().zustand().account(&konto).nonce;
            let tx = Transaktion::signiere(
                &alpha.kette().netzwert(),
                &myl_node::kette::probeschluessel(n),
                nonce,
                Anweisung::Burn { betrag: 1_000 + i },
            )
            .expect("signieren");
            alpha.kette_mut().aufnehmen(tx);
        }
        alpha.erzeuge_block().await;
    }
    alpha.laufe_fuer(Duration::from_millis(1_100)).await;
    let angelegt = im_protokoll(&alpha, "abzug_angelegt");
    assert_eq!(angelegt.len(), 1, "genau ein Abzug an der Grenze: {angelegt:?}");
    let anker_text = angelegt[0].split("\"anker\":\"").nth(1).and_then(|r| r.split('"').next()).expect("Anker");
    let anker = Anker::aus_text(anker_text).expect("Anker lesbar");
    assert_eq!(anker.hoehe, BLOECKE_JE_EPOCHE);

    // ---- Beta: vom Abzug, dann die Spitze, dann bestätigt ---------------
    let mut beta = Knoten::starten(konfig(&o, "beta", vec![einstieg.clone()], Some(anker)), false).await.expect("Beta");
    assert!(beta.holt_abzug(), "ohne eigene Kette und mit Anker holt Beta den Abzug");
    let erreicht = bis(&mut [&mut alpha, &mut beta], Duration::from_secs(60), |k| {
        k[1].kette().hoehe() == k[0].kette().hoehe() && !k[1].prueft_im_hintergrund() && !k[1].holt_abzug()
    })
    .await;
    assert!(erreicht, "Beta: Höhe {} von {}, Hintergrund läuft: {}", beta.kette().hoehe(), alpha.kette().hoehe(), beta.prueft_im_hintergrund());
    assert_eq!(beta.kette().zustandswurzel(), alpha.kette().zustandswurzel(), "dieselbe Wurzel an der Spitze");
    assert_eq!(beta.kette().basis(), BLOECKE_JE_EPOCHE, "Beta lebt aus dem Abzug");
    assert_eq!(im_protokoll(&beta, "abzug_uebernommen").len(), 1);
    assert_eq!(im_protokoll(&beta, "abzug_bestaetigt").len(), 1, "die Hintergrundprüfung bestätigt");
    assert!(beta.angehalten().is_none());
    assert_eq!(
        beta.kette().gespeicherte_bloecke(),
        Some(beta.kette().hoehe() - BLOECKE_JE_EPOCHE),
        "die Kettendatei hält nur die Blöcke nach dem Abzug"
    );

    // ---- Neustart: aus Basis und Kettendatei, ohne Netz ------------------
    let (hoehe, wurzel) = (beta.kette().hoehe(), beta.kette().zustandswurzel());
    drop(beta);
    let beta = Knoten::starten(konfig(&o, "beta", vec![], Some(anker)), false).await.expect("Neustart");
    assert_eq!((beta.kette().hoehe(), beta.kette().zustandswurzel()), (hoehe, wurzel), "Neustart aus Basis und Kettendatei");
    assert!(!beta.holt_abzug() && !beta.prueft_im_hintergrund(), "bestätigt und vollständig: nichts mehr zu holen");

    // ---- Gegenprobe: Gamma mit falschem Anker übernimmt nichts ------------
    let falsch = Anker { blockhash: myl_types::hash::Hash::sha256(b"kein Block"), ..anker };
    let mut gamma = Knoten::starten(konfig(&o, "gamma", vec![einstieg], Some(falsch)), false).await.expect("Gamma");
    let abgelehnt = bis(&mut [&mut alpha, &mut gamma], Duration::from_secs(20), |k| {
        !im_protokoll(k[1], "abzug_abgelehnt").is_empty()
    })
    .await;
    assert!(abgelehnt, "Gamma meldet die Ablehnung nicht");
    assert_eq!(gamma.kette().hoehe(), 0, "mit falschem Anker übernimmt Gamma nichts und holt auch nicht von Genesis");
    assert!(im_protokoll(&gamma, "abzug_uebernommen").is_empty());
    let _ = std::fs::remove_dir_all(&o);
}

/// Baut in einem Erzeuger `n` Blöcke, mit Transaktionen nach `muster`.
async fn baue(k: &mut Knoten, n: u64, muster: u64) {
    for i in 0..n {
        if i % muster == 0 {
            let s = (i % 8) as u8;
            let nonce = k.kette().zustand().account(&myl_node::kette::probekonto(s)).nonce;
            let tx = Transaktion::signiere(
                &k.kette().netzwert(),
                &myl_node::kette::probeschluessel(s),
                nonce,
                Anweisung::Burn { betrag: 1_000 + i * muster },
            )
            .expect("signieren");
            k.kette_mut().aufnehmen(tx);
        }
        k.erzeuge_block().await;
    }
}

fn anker_aus(k: &Knoten) -> Anker {
    let z = im_protokoll(k, "abzug_angelegt").pop().expect("ein Abzug");
    Anker::aus_text(z.split("\"anker\":\"").nth(1).and_then(|r| r.split('"').next()).expect("Anker")).expect("lesbar")
}

/// ⛔️ **Die Gegenprobe der Hintergrundprüfung**: Führen die Blöcke von
/// Genesis nicht zur Basis, hält der Knoten an.
///
/// Zwei Erzeuger bauen zwei Ketten im selben Netz (A und B, verschiedene
/// Transaktionen). Bravo lebt aus einem Abzug von B und kann deshalb keine
/// Blöcke vor der Grenze liefern. Delta hat den Anker von B: Den Abzug liefert
/// Bravo (Alpha nicht, sein Block an der Grenze hat einen anderen Hash), die
/// Blöcke bis dorthin aber nur Alpha, und die führen zu A.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn eine_widerlegte_basis_haelt_den_knoten_an() {
    let o = ordner("widerlegt");
    let erzeuger = |name: &str| KnotenKonfig { erzeugt_bloecke: true, ..konfig(&o, name, vec![], None) };
    let mut a = Knoten::starten(erzeuger("kette-a"), false).await.expect("A");
    let mut b = Knoten::starten(erzeuger("kette-b"), false).await.expect("B");
    baue(&mut a, BLOECKE_JE_EPOCHE + 5, 7).await;
    baue(&mut b, BLOECKE_JE_EPOCHE + 5, 5).await;
    a.laufe_fuer(Duration::from_millis(1_100)).await;
    b.laufe_fuer(Duration::from_millis(1_100)).await;
    let (anker_a, anker_b) = (anker_aus(&a), anker_aus(&b));
    assert_ne!(anker_a.blockhash, anker_b.blockhash, "zwei verschiedene Ketten");

    // Bravo: aus dem Abzug von B, ohne eigene Prüfung.
    let b_adresse = b.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let mut bravo = Knoten::starten(
        KnotenKonfig {
            hintergrundpruefung: false,
            ..konfig(&o, "bravo", vec![format!("{}/p2p/{}", b_adresse, b.peer_id())], Some(anker_b))
        },
        false,
    )
    .await
    .expect("Bravo");
    assert!(bis(&mut [&mut b, &mut bravo], Duration::from_secs(30), |k| k[1].kette().basis() == BLOECKE_JE_EPOCHE).await, "Bravo ohne Abzug");
    drop(b);

    // Delta: Anker von B, Gegenstellen Bravo (hat den Abzug, keine frühen
    // Blöcke) und A (hat frühe Blöcke, aber die von Kette A).
    let bravo_adresse = bravo.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let a_adresse = a.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let mut delta = Knoten::starten(
        konfig(
            &o,
            "delta",
            vec![format!("{}/p2p/{}", bravo_adresse, bravo.peer_id()), format!("{}/p2p/{}", a_adresse, a.peer_id())],
            Some(anker_b),
        ),
        false,
    )
    .await
    .expect("Delta");
    let angehalten = bis(&mut [&mut a, &mut bravo, &mut delta], Duration::from_secs(60), |k| k[2].angehalten().is_some()).await;
    assert!(angehalten, "Delta hält nicht an; Protokoll: {:?}", im_protokoll(&delta, "abzug_widerlegt"));
    assert_eq!(im_protokoll(&delta, "abzug_uebernommen").len(), 1, "den Abzug selbst nahm Delta an");
    assert_eq!(im_protokoll(&delta, "abzug_widerlegt").len(), 1);
    assert!(im_protokoll(&delta, "abzug_bestaetigt").is_empty());
    let _ = std::fs::remove_dir_all(&o);
}
