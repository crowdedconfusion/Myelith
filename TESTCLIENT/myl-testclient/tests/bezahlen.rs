//! **Bezahlen aus dem Wallet über einen echten Knoten** (2026-10-26).
//!
//! Ein Testwallet bekommt MYL, kauft Credits (Burn), eröffnet einen
//! Sitzungskontrakt für einen Agenten und stellt ihm die Vollmacht aus; die
//! Kette führt die Sitzung mit Budget und Frist. Der Inhaber widerruft sie.
//! Gegenproben: ein fremdes Wallet kann nicht widerrufen, eine verfälschte
//! Transaktion lässt der Knoten nicht ins Netz.

use std::path::{Path, PathBuf};
use std::time::Duration;

use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_node::Knoten;

/// Ein Probeordner im Zwischenordner, der beim Fallen verschwindet.
struct Probeordner(PathBuf);

impl Drop for Probeordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn konfig(o: &Path, name: &str, bootstrap: Vec<String>, relais: Option<std::net::SocketAddr>) -> KnotenKonfig {
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
        // Die Vorgabe: Die Tür darf nicht auf die Aufnahme warten (Fund 648).
        aufnahme_sekunden: 30,
        beobachtung: None,
        tuer: None,
        erzeugt_bloecke: relais.is_some(),
        podrelais: relais,
        podrelaisausweis: relais.map(|_| d.join("relais.ausweis")),
        ..KnotenKonfig::default()
    }
}

async fn runde(knoten: &mut [Knoten]) {
    knoten[0].erzeuge_block().await;
    for k in knoten.iter_mut() {
        k.laufe_fuer(Duration::from_millis(25)).await;
    }
}

/// Treibt die Knoten, bis die Arbeit im eigenen Faden fertig ist.
async fn treiben_bis<T: Send + 'static>(knoten: &mut [Knoten], arbeit: tokio::task::JoinHandle<T>) -> T {
    let ende = tokio::time::Instant::now() + Duration::from_secs(60);
    while !arbeit.is_finished() {
        assert!(tokio::time::Instant::now() < ende, "die Arbeit kam nicht zurück");
        runde(knoten).await;
    }
    arbeit.await.expect("Faden")
}

/// Treibt, bis die Bedingung gilt.
async fn bis(knoten: &mut [Knoten], was: &str, f: impl Fn(&Knoten) -> bool) {
    let ende = tokio::time::Instant::now() + Duration::from_secs(60);
    while !f(&knoten[0]) {
        assert!(tokio::time::Instant::now() < ende, "{was}");
        runde(knoten).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ein_wallet_kauft_credits_eroeffnet_eine_sitzung_und_widerruft_sie() {
    let o = Probeordner(std::env::temp_dir().join(format!("myl-bezahlen-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&o.0);
    std::fs::create_dir_all(&o.0).expect("Ordner");
    // ⚑ Die Wallets liegen im Probeordner, nicht neben den Einstellungen.
    std::env::set_var("MYL_WALLETS", o.0.join("wallets"));

    let relais = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).expect("Port");
    let mut tor = Knoten::starten(konfig(&o.0, "tor", vec![], Some(relais)), false).await.expect("Tor");
    let adresse = tor.warte_auf_adresse(Duration::from_secs(5)).await.expect("Adresse");
    let einstieg = vec![format!("{}/p2p/{}", adresse, tor.peer_id())];
    let zuhoerer = Knoten::starten(konfig(&o.0, "zuhoerer", einstieg, None), false).await.expect("Zuhörer");
    let mut knoten = vec![tor, zuhoerer];
    let leitung = myl_client::bezahlen::Knotenleitung {
        adresse: relais,
        ausweis: myl_types::relais::ausweis_lesen(&o.0.join("tor").join("relais.ausweis")).expect("Ausweis"),
    };

    // Zwei Wallets: der Zahler und ein Fremder.
    let zahler = myl_client::wallet::speichern("zahler", &myl_client::wallet::entwerfen().expect("Entwurf")).expect("Zahler");
    let fremd = myl_client::wallet::speichern("fremd", &myl_client::wallet::entwerfen().expect("Entwurf")).expect("Fremd");

    // MYL für den Zahler, aus einem Probekonto (nur im Test).
    let nonce = knoten[0].kette().zustand().account(&myl_node::kette::probekonto(1)).nonce;
    let tx = myl_consensus::block::Transaktion::signiere(
        &knoten[0].kette().netzwert(),
        &myl_node::kette::probeschluessel(1),
        nonce,
        myl_consensus::block::Anweisung::Ueberweisung { nach: zahler.adresse, betrag: 8_000_000 },
    )
    .expect("signieren");
    knoten[0].kette_mut().aufnehmen(tx);
    let a = zahler.adresse;
    bis(&mut knoten, "die MYL kamen nicht an", |k| k.kette().zustand().account(&a).balance == 8_000_000).await;

    // ---- Credits kaufen -----------------------------------------------------
    // Die Gebühr zahlt der Zahler dazu, beim Grundpreis von jetzt.
    let (l, adr) = (leitung, a);
    let vorher = treiben_bis(&mut knoten, tokio::task::spawn_blocking(move || l.kontostand(&adr))).await.expect("Kontostand");
    let g = myl_client::bezahlen::gebuehr(&vorher, &myl_consensus::block::Anweisung::Burn { betrag: 5_000_000 });
    assert!(g > 0, "keine Gebühr: {vorher:?}");
    let (l, z) = (leitung, zahler.clone());
    let r = treiben_bis(&mut knoten, tokio::task::spawn_blocking(move || myl_client::bezahlen::credits_kaufen(&l, &z, 5_000_000))).await;
    assert!(r.is_ok(), "{r:?}");
    bis(&mut knoten, "der Burn kam nicht an", |k| k.kette().zustand().account(&a).balance == 3_000_000 - g).await;
    let (l, adr) = (leitung, a);
    let stand = treiben_bis(&mut knoten, tokio::task::spawn_blocking(move || l.kontostand(&adr))).await.expect("Kontostand");
    assert!(stand.credits > 0, "keine Credits nach dem Burn: {stand:?}");

    // ---- Sitzung für einen Agenten ------------------------------------------
    let (l, z) = (leitung, zahler.clone());
    let sitzung = treiben_bis(
        &mut knoten,
        tokio::task::spawn_blocking(move || myl_client::bezahlen::sitzung_eroeffnen(&l, &z, "probeagent", 1_000, 100, 10)),
    )
    .await
    .expect("Sitzung");
    let id = sitzung.id;
    bis(&mut knoten, "die Sitzung kam nicht an", |k| k.kette().zustand().sitzungen.contains_key(&id)).await;
    let s = knoten[0].kette().zustand().sitzungen[&id].clone();
    assert_eq!(s.kontrakt.inhaber, zahler.adresse);
    assert_eq!(s.kontrakt.credits.budget, 1_000);
    assert_eq!(s.kontrakt.credits.einzellimit, 100);
    // Die Vollmacht gilt nur für diese Sitzung und trägt den Beweis für den Betreiber.
    let v = myl_types::vollmacht::Vollmacht::aus_bearer(&sitzung.vollmacht).expect("Vollmacht lesbar");
    assert_eq!(myl_types::ids::Address::aus_schluessel(&v.agent), s.kontrakt.agent, "die Vollmacht stammt nicht vom Agenten des Kontrakts");
    assert!(s.kontrakt.zahlt_an(&sitzung.betreiber, &v.empfaengerbeweis(&sitzung.betreiber)), "der Betreiber steht nicht im Kontrakt");
    assert!(myl_client::bezahlen::agentenordner().join("probeagent.vollmacht").exists());

    // ⚑ **Die Vollmacht trägt durch die Prüfung der Tür**: die Abschrift
    //   des Knotens selbst, dieselbe Zugangsstelle, dieselbe Vollmacht aus
    //   der Datei des Agenten. Bei Ablehnung steht der Grund da.
    // 📌 Fund 648: gleich nach dem Block, der die Sitzung trägt, nicht
    //   erst nach der nächsten Aufnahme. Vorher frischte der Knoten die
    //   Abschrift nur dort auf, und eine eben eröffnete Sitzung bekam 401.
    let abschrift = knoten[0].kontraktabschrift();
    assert_eq!(abschrift.epoche(), knoten[0].kette().zustand().epoch, "die Abschrift hängt an einer alten Epoche");
    let datei = std::fs::read_to_string(myl_client::bezahlen::agentenordner().join("probeagent.vollmacht")).expect("Vollmacht");
    let v = myl_types::vollmacht::Vollmacht::aus_bearer(datei.trim()).expect("Vollmacht lesbar");
    let rahmen = myl_types::vollmacht::Anfragerahmen {
        jetzt: abschrift.epoche(),
        sitzung: v.sitzung().expect("Sitzung in der Vollmacht"),
        credits: 0,
        modell: myl_types::hash::Hash::sha256(b"{}"),
    };
    assert_eq!(rahmen.sitzung, id, "die Vollmacht nennt eine andere Sitzung");
    assert_eq!(v.pruefen(&s.kontrakt.agent, &rahmen), Ok(()), "die Vollmacht besteht ihre eigene Prüfung nicht");
    let mut stelle = myl_gateway::zugang::Zugangsstelle::neu(abschrift.clone());
    assert!(stelle.durchlassen_mit_vollmacht(&v, &rahmen, 1_000).erlaubt(), "die Tür ließe die Vollmacht nicht durch");

    // ---- Überweisen, wie die Vorschau es zeigt ---------------------------------
    let (l, z, f) = (leitung, zahler.clone(), fremd.adresse);
    let ueber = treiben_bis(&mut knoten, tokio::task::spawn_blocking(move || myl_client::bezahlen::ueberweisung_vorschauen(&l, &z, f, 500_000_000))).await;
    assert!(ueber.is_err(), "eine Überweisung über das Guthaben bekam eine Vorschau");
    let (l, z) = (leitung, zahler.clone());
    let vorschau = treiben_bis(&mut knoten, tokio::task::spawn_blocking(move || myl_client::bezahlen::ueberweisung_vorschauen(&l, &z, f, 1_000_000)))
        .await
        .expect("Vorschau");
    // Der Fremde ist ein neues Konto: Die Vorschau nennt Gebühr und Aufschlag.
    assert!(vorschau.gebuehr > 0 && vorschau.max_gebuehr == 2 * vorschau.gebuehr, "{vorschau:?}");
    assert_eq!(vorschau.aufschlag, myl_node::netz::PROBEKETTE.gebuehren.aufschlag_konto);
    let vor_ueberweisung = knoten[0].kette().zustand().account(&a).balance;
    let (l, z, v) = (leitung, zahler.clone(), vorschau.clone());
    let r = treiben_bis(&mut knoten, tokio::task::spawn_blocking(move || myl_client::bezahlen::ueberweisen(&l, &z, &v))).await;
    assert!(r.is_ok(), "{r:?}");
    bis(&mut knoten, "die Überweisung kam nicht an", |k| k.kette().zustand().account(&f).balance == 1_000_000).await;
    assert_eq!(
        knoten[0].kette().zustand().account(&a).balance,
        vor_ueberweisung - 1_000_000 - vorschau.gebuehr - vorschau.aufschlag,
        "abgebucht wurde nicht, was die Vorschau zeigte"
    );
    // ⛔️ Dieselbe Vorschau ein zweites Mal: Die Nonce ist verbraucht, es
    //   wird nichts unterschrieben.
    let (l, z, v) = (leitung, zahler.clone(), vorschau);
    let zweimal = treiben_bis(&mut knoten, tokio::task::spawn_blocking(move || myl_client::bezahlen::ueberweisen(&l, &z, &v))).await;
    assert!(zweimal.is_err(), "eine veraltete Vorschau wurde unterschrieben");

    // ---- ⛔️ Gegenproben -------------------------------------------------------
    // Ein fremdes Wallet kann nicht widerrufen: Die Transaktion geht ins Netz,
    // die Kette lehnt sie ab, und die Sitzung bleibt offen.
    let (l, f) = (leitung, fremd.clone());
    let r = treiben_bis(&mut knoten, tokio::task::spawn_blocking(move || myl_client::bezahlen::sitzung_widerrufen(&l, &f, id))).await;
    assert!(r.is_ok(), "{r:?}");
    for _ in 0..20 {
        runde(&mut knoten).await;
    }
    assert!(!knoten[0].kette().zustand().sitzungen[&id].zustand.widerrufen, "ein Fremder hat widerrufen");
    // Eine verfälschte Transaktion lässt der Knoten gar nicht ins Netz.
    let nonce = knoten[0].kette().zustand().account(&a).nonce;
    let mut tx = myl_consensus::block::Transaktion::signiere(
        &knoten[0].kette().netzwert(),
        &myl_node::kette::probeschluessel(2),
        nonce,
        myl_consensus::block::Anweisung::Burn { betrag: 1 },
    )
    .expect("signieren");
    tx.nonce += 1;
    let bytes = borsh::to_vec(&tx).expect("Borsh");
    let r = treiben_bis(
        &mut knoten,
        tokio::task::spawn_blocking(move || {
            myl_types::relais::fragen(relais, &leitung.ausweis, &myl_types::relais::Relaisfrage::Transaktion { tx: bytes }, Duration::from_secs(10))
        }),
    )
    .await;
    assert!(matches!(&r, Ok(myl_types::relais::Relaisantwort::Fehler(f)) if f.contains("ungültig")), "{r:?}");

    // ---- Der Inhaber widerruft ----------------------------------------------
    let (l, z) = (leitung, zahler.clone());
    let r = treiben_bis(&mut knoten, tokio::task::spawn_blocking(move || myl_client::bezahlen::sitzung_widerrufen(&l, &z, id))).await;
    assert!(r.is_ok(), "{r:?}");
    bis(&mut knoten, "der Widerruf kam nicht an", |k| k.kette().zustand().sitzungen[&id].zustand.widerrufen).await;
    std::env::remove_var("MYL_WALLETS");
}
