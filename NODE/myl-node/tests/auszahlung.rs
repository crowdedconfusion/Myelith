//! **Das Auszahlungskonto über das Netz** (Fund 643, 2026-10-26).
//!
//! Bis dahin las der Knoten `--konto` und tat nichts damit. Jetzt meldet er
//! sich mit seinem Konsensschlüssel als Miner an und trägt das Konto über
//! die Kette ein. Ein Wechsel braucht die Unterschrift des bisher
//! eingetragenen Kontos (`--kontowechsel`); ohne sie bleibt das alte stehen.

use std::path::{Path, PathBuf};
use std::time::Duration;

use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::schluessel::Konsensschluessel;
use myl_node::Knoten;
use myl_types::ids::{Address, MinerId};

/// Ein Probeordner im Zwischenordner, der beim Fallen verschwindet.
struct Probeordner(PathBuf);

impl Drop for Probeordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ordner(name: &str) -> Probeordner {
    let p = std::env::temp_dir().join(format!("myl-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("Ordner");
    Probeordner(p)
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

async fn treiben(alpha: &mut Knoten, beta: &mut Knoten, frist: Duration, fertig: impl Fn(&Knoten) -> bool) -> bool {
    let ende = tokio::time::Instant::now() + frist;
    while tokio::time::Instant::now() < ende {
        if fertig(alpha) {
            return true;
        }
        alpha.erzeuge_block().await;
        alpha.laufe_fuer(Duration::from_millis(30)).await;
        beta.laufe_fuer(Duration::from_millis(30)).await;
    }
    fertig(alpha)
}

fn eingetragen(k: &Knoten, miner: &MinerId) -> Option<Address> {
    k.kette().zustand().auszahlung.get(miner).copied()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn das_konto_geht_ueber_die_kette_und_wechselt_nur_mit_dem_alten_schluessel() {
    let o = ordner("auszahlung");
    let konsens = o.0.join("beta.konsens.key");
    let ich = Konsensschluessel::aus_datei(&konsens).expect("Konsensschluessel").kennung();
    let wallet_a = o.0.join("wallet-a.key");
    let wallet_b = o.0.join("wallet-b.key");
    let a = Konsensschluessel::neu_erzeugen(&wallet_a).expect("Wallet A").adresse();
    let b = Konsensschluessel::neu_erzeugen(&wallet_b).expect("Wallet B").adresse();

    let mut alpha = Knoten::starten(KnotenKonfig { erzeugt_bloecke: true, ..konfig(&o.0, "alpha", vec![]) }, false)
        .await
        .expect("Alpha");
    alpha.erzeuge_block().await;
    let adresse = alpha.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let einstieg = format!("{}/p2p/{}", adresse, alpha.peer_id());
    let beta_mit = |konto: Address, wechsel: Option<PathBuf>| KnotenKonfig {
        konsensschluesseldatei: Some(konsens.clone()),
        konto: Some(konto),
        kontowechseldatei: wechsel,
        ..konfig(&o.0, "beta", vec![einstieg.clone()])
    };

    // 1. Erste Eintragung: Anmeldung und Konto A, unterschrieben vom Miner.
    let mut beta = Knoten::starten(beta_mit(a, None), false).await.expect("Beta");
    assert!(
        treiben(&mut alpha, &mut beta, Duration::from_secs(60), |k| eingetragen(k, &ich) == Some(a)).await,
        "Konto A kam nicht an: {:?}, Miner angemeldet: {}",
        eingetragen(&alpha, &ich),
        alpha.kette().zustand().miner.contains_key(&ich)
    );
    // Die Kontoauskunft kennt A jetzt als Auszahlungskonto, B noch nicht.
    assert!(alpha.kontoauskunft(&a).bekannt(), "A ist auf der Kette, die Auskunft kennt es nicht");
    assert_eq!(alpha.kontoauskunft(&a).auszahlung_fuer, 1);
    assert!(!alpha.kontoauskunft(&b).bekannt(), "B ist unbenutzt und doch bekannt");
    drop(beta);

    // 2. ⛔️ Gegenprobe: B ohne den Schlüssel von A bleibt A.
    let mut beta = Knoten::starten(beta_mit(b, None), false).await.expect("Beta");
    treiben(&mut alpha, &mut beta, Duration::from_secs(15), |_| false).await;
    assert_eq!(eingetragen(&alpha, &ich), Some(a), "ohne den alten Schlüssel wechselte das Konto");
    let protokoll = std::fs::read_to_string(beta.protokollpfad()).unwrap_or_default();
    assert!(
        protokoll.lines().any(|z| z.contains("\"art\":\"auszahlungskonto\"") && z.contains("\"ist\"")),
        "der Knoten sagte nicht, dass ein anderes Konto eingetragen ist"
    );
    drop(beta);

    // 3. Mit dem Schlüssel von A wechselt es zu B.
    let mut beta = Knoten::starten(beta_mit(b, Some(wallet_a)), false).await.expect("Beta");
    assert!(
        treiben(&mut alpha, &mut beta, Duration::from_secs(60), |k| eingetragen(k, &ich) == Some(b)).await,
        "Konto B kam nicht an: {:?}",
        eingetragen(&alpha, &ich)
    );
}
