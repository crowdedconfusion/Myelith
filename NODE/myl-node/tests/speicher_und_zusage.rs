//! Speicherorte und Zusage an einem laufenden Knoten.
//!
//! ⚑ **Geprueft wird, was ein Miner sieht**: Die Unterschrift unter der
//! Zusage laesst sich mit dem oeffentlichen Schluessel aus der Datei
//! nachpruefen, und die Zustandsaufnahme zaehlt, was in den Orten liegt.

use std::path::{Path, PathBuf};

use myl_node::konfig::KnotenKonfig;
use myl_node::Knoten;

fn arbeitsverzeichnis(marke: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "myl-node-zusage-{marke}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn konfig(v: &Path, kontoschluessel: bool) -> KnotenKonfig {
    KnotenKonfig {
        name: "miner".into(),
        schluesseldatei: v.join("miner.key"),
        protokollverzeichnis: v.join("logs"),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".into(), "/ip4/127.0.0.1/udp/0/quic-v1".into()],
        kettendatei: None,
        beobachtung: None,
        tuer: None,
        speicherorte: Some(v.join(myl_store::orte::DATEINAME)),
        kontoschluesseldatei: kontoschluessel.then(|| v.join("konto.key")),
        ..Default::default()
    }
}

/// Ein Ort fuer das Netz mit einem Shard darin, und die Zusage dazu.
fn ort_und_zusage(v: &Path) -> PathBuf {
    let ort = myl_store::Speicherort {
        name: "netz".into(),
        pfad: v.join("platte"),
        volumen_gib: 2,
        zwecke: vec![myl_store::Zweck::Netz],
    };
    let teile = myl_store::teile_bilden(&[1u8; 64]).unwrap();
    let manifest = myl_store::Manifest::neu(
        myl_store::Gegenstandsart::Shardgewichte,
        1,
        &teile,
        myl_store::Redundanzform::Kopien { anzahl: 1 },
    )
    .unwrap();
    let d = ort.ordner(myl_store::Zweck::Netz).join("shard-0");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(myl_store::orte::MANIFESTDATEI), borsh::to_vec(&manifest).unwrap()).unwrap();
    let mut orte = myl_store::Speicherorte::default();
    orte.setzen(ort.clone());
    orte.schreiben(&v.join(myl_store::orte::DATEINAME)).unwrap();
    let zusage = myl_store::Zusage {
        fassung: myl_store::Zusage::FASSUNG,
        probelauf: true,
        orte: vec![myl_store::Zusageort { name: ort.name, pfad: ort.pfad, volumen_bytes: ort.volumen_gib * myl_store::orte::GIB }],
        kerne: 4,
        rechenwerke: vec![],
        erstellt_ms: 1,
    };
    let p = v.join("zusage.toml");
    std::fs::write(&p, zusage.als_text()).unwrap();
    p
}

fn feld<'a>(text: &'a str, name: &str) -> &'a str {
    text.lines()
        .find_map(|z| z.strip_prefix(&format!("{name} ")))
        .unwrap_or_else(|| panic!("{name} fehlt in\n{text}"))
}

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
}

#[tokio::test]
async fn die_zusage_ist_nachpruefbar_unterschrieben_und_die_aufnahme_zaehlt_den_speicher() {
    let v = arbeitsverzeichnis("ja");
    let pfad = ort_und_zusage(&v);
    let mut k = Knoten::starten(konfig(&v, true), false).await.expect("Start");
    let ziel = k.zusage_unterschreiben(&pfad).expect("unterschrieben");
    assert_eq!(ziel, v.join("zusage.unterschrift"));

    let text = std::fs::read_to_string(&ziel).unwrap();
    let roh = std::fs::read(&pfad).unwrap();
    let epoche: u64 = feld(&text, "epoche").parse().unwrap();
    let oeffentlich: myl_types::bls::BlsPublicKey = borsh::from_slice(&bytes(feld(&text, "schluessel"))).unwrap();
    let signatur: myl_types::bls::BlsSignature = borsh::from_slice(&bytes(feld(&text, "unterschrift"))).unwrap();
    assert!(oeffentlich.verify(&myl_store::orte::botschaft(&roh, epoche), &signatur), "Unterschrift passt nicht");
    // ⚑ Eine andere Epoche ist eine andere Zusage.
    assert!(!oeffentlich.verify(&myl_store::orte::botschaft(&roh, epoche + 1), &signatur));
    // ⚑ Und eine geaenderte Datei auch.
    let anders = String::from_utf8(roh.clone()).unwrap().replace("kerne = 4", "kerne = 64");
    assert!(!oeffentlich.verify(&myl_store::orte::botschaft(anders.as_bytes(), epoche), &signatur));
    assert_eq!(feld(&text, "konto"), k.eigenes_konto().as_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>());

    k.aufnahme().await;
    let s = k.beobachtungsstelle().holen();
    assert_eq!((s.speicher_orte, s.speicher_gegenstaende, s.speicher_shards, s.speicher_unlesbar), (1, 1, 1, 0));
    assert_eq!(s.speicher_grenze_bytes, 2 * myl_store::orte::GIB);
    assert_eq!(s.speicher_zugesagt_bytes, 2 * myl_store::orte::GIB);
    assert!(s.speicher_belegt_bytes > 0);
    assert_eq!((s.inferenz_bearbeitet, s.inferenz_abgelehnt), (0, 0));
    let _ = std::fs::remove_dir_all(&v);
}

/// ⚑ **Ohne eigenen Kontoschluessel keine Unterschrift**: Der aus dem
/// Namen abgeleitete ist keiner (Fund 170).
#[tokio::test]
async fn ohne_kontoschluessel_wird_nichts_unterschrieben() {
    let v = arbeitsverzeichnis("nein");
    let pfad = ort_und_zusage(&v);
    let mut k = Knoten::starten(konfig(&v, false), false).await.expect("Start");
    let f = k.zusage_unterschreiben(&pfad).unwrap_err();
    assert!(f.contains("--kontoschluessel"), "{f}");
    assert!(!v.join("zusage.unterschrift").exists());
    let _ = std::fs::remove_dir_all(&v);
}
