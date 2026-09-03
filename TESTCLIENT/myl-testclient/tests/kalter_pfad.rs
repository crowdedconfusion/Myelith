//! Der kalte Pfad von Ende zu Ende (GATEWAY Stufe 4).
//!
//! # ⚑ Warum dieser Test hier steht und nirgends sonst
//!
//! Er braucht **beide Enden**: einen echten Knoten aus `myl-node` und
//! einen echten Shard-Dienst aus `myl-pod`. Die beiden Kisten kennen
//! einander nicht und sollen es nicht, denn `myl-pod` zieht die
//! Ganzzahl-Laufzeit nach und `myl-node` zieht libp2p nach.
//! **`myl-testclient` ist die einzige Stelle, die beide sieht**, also
//! ist er die Naht.
//!
//! Dieselbe Begründung wie beim Deckelvergleich in `myl-node`: Wo zwei
//! Seiten getrennt gebaut werden, muss ein Dritter zeigen, dass sie
//! zusammenpassen. **Das ist die Fehlerklasse, die dieses Projekt
//! neunmal getroffen hat:** beide Seiten gebaut, beide für sich
//! geprüft, die Naht fehlt.
//!
//! # Was der Weg ist
//!
//! Auftrag über das Netz an einen Knoten, von dort über die lokale
//! Leitung an den Shard-Prozess, Antwort denselben Weg zurück. **Alle
//! vier Sprünge sind echt**: zwei Sockets, kein nachgebautes Ende.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use myl_node::knoten::Knoten;
use myl_node::konfig::{KnotenKonfig, Rolle};
use myl_pod::ortsdienst::{Ortsdienst, Rechenwerk};
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, SegmentId};
use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};
use myl_types::ortsleitung::SCHLUESSEL_DATEI;
use myl_types::sitzung::Anfragebindung;

/// Ein Rechenwerk, das nicht rechnet, sondern bezeugt, dass es gefragt
/// wurde.
///
/// ⚑ **Kein Modell im Test**, und das ist Absicht: Was hier auf dem
/// Prüfstand steht, ist der **Weg** und nicht die Inferenz. Die
/// Pipeline hat ihre eigenen Tests gegen echte Artefakte; ein Modell
/// hier machte den Test langsam und würde trotzdem nichts über den Weg
/// aussagen.
struct Zeugenwerk {
    gesehen: Arc<AtomicUsize>,
}

impl Rechenwerk for Zeugenwerk {
    fn rechne(&self, auftrag: &Inferenzauftrag) -> Inferenzantwort {
        self.gesehen.fetch_add(1, Ordering::SeqCst);
        Inferenzantwort::Ergebnis {
            sitzung: auftrag.sitzung,
            token: vec![31, 41, 59],
            segment: SegmentId::new([8; 32]),
            prompt_token: 5,
            text: "Paris".to_string(),
        }
    }
    fn pipeline(&self) -> Hash {
        Hash::sha256(b"kalter-pfad")
    }
    fn shards(&self) -> u32 {
        4
    }
}

fn arbeitsverzeichnis(marke: &str) -> std::path::PathBuf {
    let v = std::env::temp_dir().join(format!(
        "myl-kalter-pfad-{marke}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&v).expect("Arbeitsverzeichnis");
    v
}

fn konfig(
    verz: &std::path::Path,
    name: &str,
    bootstrap: Vec<String>,
    orts: Option<(std::net::SocketAddr, std::path::PathBuf)>,
) -> KnotenKonfig {
    let (ortsleitung, ortsausweis) = match orts {
        Some((a, p)) => (Some(a), Some(p)),
        None => (None, None),
    };
    KnotenKonfig {
        name: name.to_string(),
        schluesseldatei: verz.join(format!("{name}.key")),
        protokollverzeichnis: verz.join("protokolle"),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        bootstrap,
        rolle: Rolle::Teilnehmer,
        nat: myl_net::NatKonfig::default(),
        aufnahme_sekunden: 30,
        beobachtung: None,
        tuer: None,
        ortsleitung,
        ortsausweis,
        pod: None,
        modellname: "myelith-qwen2.5-0.5b".to_string(),
        kontoschluesseldatei: None,
        konto: None,
        testverkehr_sekunden: None,
        erzeugt_bloecke: false,
        teilnehmer: Vec::new(),
        kettendatei: None,
        stimmsatzdatei_pfad: None,
        konsensschluesseldatei: None,
    }
}

/// Öffnet einen Shard-Dienst und lässt ihn im Hintergrund bedienen.
fn shard_starten(verz: &std::path::Path) -> (std::net::SocketAddr, Arc<AtomicUsize>) {
    let gesehen = Arc::new(AtomicUsize::new(0));
    let (dienst, befund) = Ortsdienst::oeffnen(
        "127.0.0.1:0".parse().expect("Adresse"),
        verz,
        Box::new(Zeugenwerk {
            gesehen: Arc::clone(&gesehen),
        }),
    )
    .expect("der Shard-Dienst geht auf");
    assert!(!befund.nach_aussen, "der Shard-Dienst haengt nach aussen");
    #[cfg(unix)]
    assert!(befund.ausweis_geschuetzt, "der Ausweis liegt offen");
    let adresse = befund.adresse;
    std::thread::spawn(move || loop {
        if dienst.bediene_eine().is_err() {
            break;
        }
    });
    (adresse, gesehen)
}

async fn verbinden(alpha: &mut Knoten, beta: &mut Knoten) {
    let bis = std::time::Instant::now() + Duration::from_secs(20);
    while std::time::Instant::now() < bis && beta.peers().await == 0 {
        beta.laufe_fuer(Duration::from_millis(200)).await;
        alpha.laufe_fuer(Duration::from_millis(200)).await;
    }
    assert!(beta.peers().await > 0, "die beiden Knoten fanden einander nicht");
}

fn auftrag() -> Inferenzauftrag {
    Inferenzauftrag {
        sitzung: 77,
        bindung: Anfragebindung::neu(77, b"was ist die hauptstadt von frankreich", EpochId(0)),
        prompt_versiegelt: b"versiegelter prompt".to_vec(),
        max_token: 16,
        pipeline: Hash::sha256(b"kalter-pfad"),
    }
}

/// ⚑ **Ein Auftrag erreicht durch zwei Sockets ein Rechenwerk und
/// kommt zurück.**
#[tokio::test]
async fn ein_auftrag_geht_durch_den_knoten_zum_shard_und_zurueck() {
    let verz = arbeitsverzeichnis("ganz");
    let (shard, gesehen) = shard_starten(&verz);

    let mut alpha = Knoten::starten(
        konfig(&verz, "alpha", vec![], Some((shard, verz.join(SCHLUESSEL_DATEI)))),
        false,
    )
    .await
    .expect("Alpha startet mit Ortsleitung");
    let adresse = alpha
        .warte_auf_adresse(Duration::from_secs(10))
        .await
        .expect("Alpha nennt eine Adresse");
    let mut beta = Knoten::starten(
        konfig(&verz, "beta", vec![adresse.to_string()], None),
        false,
    )
    .await
    .expect("Beta startet");
    verbinden(&mut alpha, &mut beta).await;

    assert!(
        beta.inferenz_senden(alpha.peer_id(), auftrag()).await,
        "der Auftrag ging nicht raus"
    );

    let bis = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < bis && beta.letzte_inferenzantwort().is_none() {
        alpha.laufe_fuer(Duration::from_millis(200)).await;
        beta.laufe_fuer(Duration::from_millis(200)).await;
    }

    assert_eq!(
        beta.letzte_inferenzantwort(),
        Some(&Inferenzantwort::Ergebnis {
            sitzung: 77,
            token: vec![31, 41, 59],
            segment: SegmentId::new([8; 32]),
            prompt_token: 5,
            text: "Paris".to_string(),
        }),
        "der Auftrag kam nicht durch den ganzen Weg zurueck"
    );
    assert_eq!(
        gesehen.load(Ordering::SeqCst),
        1,
        "das Rechenwerk hat den Auftrag nicht gesehen"
    );

    let _ = std::fs::remove_dir_all(&verz);
}

/// ⚑ **Die Gegenprobe zum ganzen Weg:** Ohne Ortsleitung antwortet
/// derselbe Knoten mit `Abgelehnt`.
///
/// Ohne sie prüfte der Test oben nur, dass **irgendeine** Antwort
/// zurückkommt, und das täte er auch, wenn die Leitung gar nicht
/// benutzt würde.
#[tokio::test]
async fn ohne_ortsleitung_lehnt_derselbe_knoten_ab() {
    let verz = arbeitsverzeichnis("ohne");
    let mut alpha = Knoten::starten(konfig(&verz, "alpha", vec![], None), false)
        .await
        .expect("Alpha startet");
    let adresse = alpha
        .warte_auf_adresse(Duration::from_secs(10))
        .await
        .expect("Alpha nennt eine Adresse");
    let mut beta = Knoten::starten(
        konfig(&verz, "beta", vec![adresse.to_string()], None),
        false,
    )
    .await
    .expect("Beta startet");
    verbinden(&mut alpha, &mut beta).await;

    assert!(beta.inferenz_senden(alpha.peer_id(), auftrag()).await);
    let bis = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < bis && beta.letzte_inferenzantwort().is_none() {
        alpha.laufe_fuer(Duration::from_millis(200)).await;
        beta.laufe_fuer(Duration::from_millis(200)).await;
    }
    assert_eq!(
        beta.letzte_inferenzantwort(),
        Some(&Inferenzantwort::Abgelehnt { sitzung: 77 }),
        "ein Knoten ohne Shard hat nicht abgelehnt"
    );
    let _ = std::fs::remove_dir_all(&verz);
}

/// ⚑ **Ein Knoten, der `--ortsleitung` sagt und keinen Ausweis
/// findet, startet nicht.**
///
/// Klasse von Fund 56: Wer eine Absicht erklärt, die nicht erfüllbar
/// ist, soll das beim Start erfahren und nicht beim ersten Nutzer. Ein
/// Knoten, der jeden Auftrag ablehnt, sieht im Betrieb aus wie ein
/// Shard, der schweigt.
#[tokio::test]
async fn ohne_ausweis_startet_der_knoten_nicht() {
    let verz = arbeitsverzeichnis("ausweislos");
    let ergebnis = Knoten::starten(
        konfig(
            &verz,
            "alpha",
            vec![],
            Some((
                "127.0.0.1:4170".parse().expect("Adresse"),
                verz.join(SCHLUESSEL_DATEI),
            )),
        ),
        false,
    )
    .await;
    assert!(
        ergebnis.is_err(),
        "der Knoten startete mit einer Ortsleitung ohne Ausweis"
    );
    // Gegenprobe: mit Ausweis startet derselbe Knoten.
    myl_types::ortsleitung::schluessel_ablegen(&verz.join(SCHLUESSEL_DATEI), &[5u8; 32])
        .expect("ablegen");
    let ergebnis = Knoten::starten(
        konfig(
            &verz,
            "alpha",
            vec![],
            Some((
                "127.0.0.1:4170".parse().expect("Adresse"),
                verz.join(SCHLUESSEL_DATEI),
            )),
        ),
        false,
    )
    .await;
    assert!(ergebnis.is_ok(), "mit Ausweis startete der Knoten trotzdem nicht");
    let _ = std::fs::remove_dir_all(&verz);
}
