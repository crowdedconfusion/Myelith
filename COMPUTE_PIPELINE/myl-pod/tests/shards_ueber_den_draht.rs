//! Vier Shards, vier Türen, ein Pod (Fund 186).
//!
//! # ⚑ Was dieser Test beweist, und was er nicht beweist
//!
//! **Er beweist:** Ein Koordinator, der die Shards **nur über TCP**
//! erreicht, bekommt bis aufs Bit dieselben Token wie einer, der sie im
//! selben Prozess ruft. Jede Nachricht geht durch Borsh, über einen
//! Socket, und zurück; Rechnen, Unterschreiben und der Dekodier-Abdruck
//! laufen alle über denselben Draht.
//!
//! ⚑ **Er beweist nicht die Isolation.** Vier Fäden teilen einen
//! Adressraum; erst vier Prozesse zeigen, dass ein Shard ohne die
//! Schlüssel der anderen auskommt. Dafür gibt es
//! `vier_prozesse_ein_pod.rs`.
//!
//! # ⚑ Warum das nötig war
//!
//! `PodMessage` war seit Langem ein fertiges Format mit Rundlauftest,
//! **und überquerte nie eine Prozessgrenze**: `Coordinator` rief
//! `shard.process` unmittelbar, `Pipelinewerk` legte alle vier Shards in
//! einen Prozess. Damit war die Kernaussage des Shardings unbelegt.

use std::sync::Arc;

use myl_pod::coordinator::Coordinator;
use myl_pod::shard::ShardNode;
use myl_pod::shardweg::{ImProzess, Shardstelle, UeberDenDraht};
use myl_types::bls::BlsSecretKey;
use myl_types::ids::{EpochId, PodId};

const SHARDS: usize = 4;

fn artefakte() -> std::path::PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    std::path::PathBuf::from(manifest)
        .join("..")
        .join("..")
        .join("INTEGER_LLM")
        .join("artifacts")
        .join("myelith-0.6b")
}

/// Vier Shards, wie `Pipelinewerk` sie schneidet.
///
/// ⚑ **Dieselben abgeleiteten Schlüssel wie dort**, damit die beiden
/// Läufe vergleichbar sind. Ein echter Miner bringt seinen eigenen mit;
/// hier geht es darum, dass **derselbe** Pod zweimal dasselbe rechnet.
fn shards(modell: Arc<integer_llm_runtime::model::IntegerModel>) -> Vec<Arc<ShardNode>> {
    let layer = modell.num_layers;
    let grenzen: Vec<usize> = (0..=SHARDS).map(|s| layer * s / SHARDS).collect();
    (0..SHARDS)
        .map(|s| {
            let ikm = [(s as u8 + 1).wrapping_mul(17); 32];
            let sk = BlsSecretKey::key_gen(&ikm).expect("Schluessel");
            Arc::new(ShardNode::new(
                s,
                grenzen[s],
                grenzen[s + 1],
                s == 0,
                s == SHARDS - 1,
                Arc::clone(&modell),
                sk,
                16,
            ))
        })
        .collect()
}

const PROMPT: [u32; 6] = [9707, 374, 264, 1273, 315, 279];

/// ⚑ **Über den Draht muss dasselbe herauskommen wie im Prozess.**
#[test]
fn vier_tueren_ein_pod_und_dieselben_token() {
    let dir = artefakte();
    if !dir.exists() {
        eprintln!("Artefakt fehlt, uebersprungen: {}", dir.display());
        return;
    }
    let modell = Arc::new(integer_llm_runtime::loader::load_model(&dir).expect("Modell"));
    let pod = PodId::new([0xAA; 32]);
    let epoche = EpochId(0);

    // --- Im Prozess ------------------------------------------------
    let mut im_prozess = Coordinator::new(pod, epoche, shards(Arc::clone(&modell)), 5_000);
    let token_direkt = im_prozess.run_prompt(1, &PROMPT, 8);
    let abdruck_direkt = im_prozess.dekodier_digest(1).expect("Abdruck");
    let buendel_direkt = im_prozess.build_signed_poi_bundle().expect("Buendel");

    // --- Über vier Türen -------------------------------------------
    let stellen: Vec<Arc<Shardstelle>> = shards(Arc::clone(&modell))
        .into_iter()
        .map(|s| Arc::new(Shardstelle::oeffnen(s, "127.0.0.1:0").expect("Tuer")))
        .collect();
    let adressen: Vec<std::net::SocketAddr> =
        stellen.iter().map(|s| s.adresse().expect("Adresse")).collect();
    // ⚑ Jede Stelle bedient in einem eigenen Faden, sonst blockierte der
    // Koordinator sich selbst.
    for stelle in &stellen {
        let s = Arc::clone(stelle);
        std::thread::spawn(move || s.laufen());
    }

    let weg = Arc::new(UeberDenDraht::neu(adressen).mit_frist(std::time::Duration::from_secs(120)));
    let mut ueber_draht =
        Coordinator::ueber_weg(pod, epoche, weg, 5_000).expect("Besetzung erhoben");
    let token_draht = ueber_draht.run_prompt(1, &PROMPT, 8);
    let abdruck_draht = ueber_draht.dekodier_digest(1).expect("Abdruck");
    let buendel_draht = ueber_draht.build_signed_poi_bundle().expect("Buendel");

    // --- Der Vergleich ---------------------------------------------
    assert_eq!(token_draht, token_direkt, "ueber den Draht kamen andere Token");
    assert!(!token_direkt.is_empty(), "es wurde gar nichts gerechnet");
    assert_eq!(abdruck_draht, abdruck_direkt, "der Dekodier-Abdruck weicht ab");
    assert_eq!(
        buendel_draht.vtfe_claimed, buendel_direkt.vtfe_claimed,
        "die beanspruchte Arbeit weicht ab"
    );
    assert_eq!(
        buendel_draht.segments_root, buendel_direkt.segments_root,
        "die Segmentwurzel weicht ab"
    );
    // ⚑ **Die Aggregatsignatur entsteht über den Draht.** Jeder Shard
    // unterschreibt selbst, mit seinem eigenen Schlüssel; der
    // Koordinator kann es nicht.
    assert_eq!(
        buendel_draht.aggregate_sig, buendel_direkt.aggregate_sig,
        "die Aggregatsignatur weicht ab"
    );
    eprintln!(
        "\n--- Vier Tueren, ein Pod ---\n  Token: {token_draht:?}\n  \
         Abdruck: {abdruck_draht:?}\n  vTFE: {}",
        buendel_draht.vtfe_claimed
    );
}

/// ⚑ **Die Gegenprobe: ein stummer Shard fällt auf.**
///
/// Ohne sie prüfte der Test darüber nichts: Ein Weg, der stillschweigend
/// auf den Prozessweg zurückfiele, bestünde ihn.
#[test]
fn ein_nicht_erreichbarer_shard_wird_gemeldet() {
    // Ein Port, auf dem niemand horcht.
    let horcher = std::net::TcpListener::bind("127.0.0.1:0").expect("binden");
    let tot = horcher.local_addr().expect("Adresse");
    drop(horcher);

    let weg = Arc::new(
        UeberDenDraht::neu(vec![tot; SHARDS]).mit_frist(std::time::Duration::from_millis(200)),
    );
    let fehler = match Coordinator::ueber_weg(PodId::new([1; 32]), EpochId(0), weg, 5_000) {
        Err(e) => e,
        Ok(_) => panic!("ein toter Shard muss auffallen"),
    };
    assert!(
        fehler.contains("nicht erreichbar"),
        "die Meldung sagt nicht, was los ist: {fehler}"
    );
}

/// ⚑ **Und der Prozessweg sagt in seinem Namen, was er ist.**
///
/// `ImProzess` hält alle Schlüssel des Pods. Das ist kein Pod, sondern
/// eine Maschine, die mehrere Rollen spielt, und der Typ soll niemanden
/// darüber täuschen.
#[test]
fn der_prozessweg_haelt_alle_shards() {
    let dir = artefakte();
    if !dir.exists() {
        return;
    }
    let modell = Arc::new(integer_llm_runtime::loader::load_model(&dir).expect("Modell"));
    let weg = ImProzess::neu(shards(modell));
    assert_eq!(weg.shards().len(), SHARDS, "alle vier liegen in einem Prozess");
}
