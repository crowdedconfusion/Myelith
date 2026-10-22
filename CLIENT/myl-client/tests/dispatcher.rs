//! ⚑ **Ein geladenes Modell meldet sich beim Dispatcher an**, und mit ihm
//! endet die Anmeldung; die Wache der Sinne ist danach gesetzt.
//!
//! Eine eigene Pruefsammlung, weil die Wache je Prozess einmal gesetzt
//! wird und das Register je Prozess gilt.

#[test]
fn ein_geladenes_modell_ist_angemeldet() {
    let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../../INTEGER_LLM/artifacts/sparrow-0.6b-int8");
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
        eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt): {pfad}");
        return;
    }
    if !std::path::Path::new(pfad).is_dir() {
        panic!(
            "Artefakte fehlen: {pfad}\n\
             MYL_OHNE_ARTEFAKTE=1 cargo test erlaubt den Sprung ausdruecklich."
        );
    }
    let angemeldet = || {
        myl_dispatcher::local::register::laufende()
            .into_iter()
            .find(|l| l.name == "sparrow-0.6b-int8" && l.vorrang == myl_dispatcher::local::plan::Vorrang::Agent)
    };
    assert!(angemeldet().is_none(), "angemeldet, bevor etwas geladen ist");
    let m = myl_client::Oertlichesmodell::laden(pfad, &Default::default()).expect("Modell laedt");
    let l = angemeldet().expect("das geladene Modell ist nicht angemeldet");
    // Ein dichtes Modell: Was bleiben muss, sind alle Gewichte ausser der
    // Einbettung, von der jedes Token nur eine Zeile liest (das 0,6B rechnet
    // die Logits mit seinem eigenen `i16`-Kopf).
    let b = myl_dispatcher::local::bedarf::Modellbedarf::aus_artefakt(std::path::Path::new(pfad)).expect("Bedarf");
    assert_eq!(l.dicht_bytes, b.dicht_bytes());
    assert!(b.rand_selten_bytes > 0 && l.dicht_bytes + b.rand_selten_bytes == b.gewichte_bytes());
    assert!(!myl_senses::speicherwache_setzen(|_, _| Ok(())), "die Wache der Sinne war nicht gesetzt");
    drop(m);
    assert!(angemeldet().is_none(), "die Anmeldung ueberlebte das Modell");
}

/// ⚑ **Die Freigabe gilt dem, was fest im Speicher bleiben muss**, nicht
/// der Groesse auf der Platte: Beim 0,6B sind das die Gewichte (dicht),
/// KV fuer 8192 Token und der Zuschlag, zusammen knapp 3 GiB. Eine
/// Freigabe von 2 GiB lehnt mit diesem Grund ab, eine von 4 GiB laedt.
///
/// Dazu die dritte Grenze des Arbeitskontexts: Was der Speicher traegt,
/// begrenzt ihn.
#[test]
fn die_freigabe_prueft_den_festen_bedarf_und_der_speicher_begrenzt_den_kontext() {
    let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../../INTEGER_LLM/artifacts/sparrow-0.6b-int8");
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
        eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt): {pfad}");
        return;
    }
    assert!(std::path::Path::new(pfad).is_dir(), "Artefakte fehlen: {pfad}");
    let mit = |gib: u32| myl_client::einstellungen::Kapazitaet { speicher_gib: Some(gib), ..Default::default() };
    let fehler = myl_client::Oertlichesmodell::laden(pfad, &mit(2)).err().expect("2 GiB reichen nicht");
    assert!(fehler.contains("fest im Arbeitsspeicher") && fehler.contains("8192 Token"), "{fehler}");
    let mut m = myl_client::Oertlichesmodell::laden(pfad, &mit(4)).expect("4 GiB reichen");
    // Mit 4 GiB Freigabe traegt der Speicher nicht die ganzen 40 960 Token
    // (KV allein 4,4 GiB); die Grenze kommt aus dem Plan.
    let tragbar = m.kontext_tragbar.expect("das Freie ist hier messbar");
    assert!((8192..40960).contains(&tragbar), "{tragbar}");
    m.kontextbudget = None;
    assert_eq!(m.arbeitsgrenze(), tragbar, "der Speicher begrenzt den Arbeitskontext nicht");
    m.kontextbudget = Some(8192);
    assert_eq!(m.arbeitsgrenze(), 8192, "die Einstellung gilt, wenn sie kleiner ist");
}

