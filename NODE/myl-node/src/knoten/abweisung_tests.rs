use super::*;
use crate::kette::{probeanmeldung, probezusage, Kette};
use myl_types::ids::{EpochId, MinerId};

/// Der Ordner einer Probe, unter dem Zwischenordner.
fn ordner(marke: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("myl-abweisung-{marke}-{}", std::process::id()))
}

/// Der Ordner einer bestandenen Probe bleibt nicht liegen.
fn aufraeumen(marke: &str) {
    let _ = std::fs::remove_dir_all(ordner(marke));
}

/// Ein Knoten ohne Nachbarn, dessen Kette einen Pod aus sechs
/// Probekonten in Epoche 3 trägt.
async fn knoten_mit_pod(marke: &str) -> Knoten {
    let d = ordner(marke);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("Ordner");
    let konfig = KnotenKonfig {
        name: marke.to_string(),
        schluesseldatei: d.join("knoten.key"),
        protokollverzeichnis: d.clone(),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        kettendatei: None,
        beobachtung: None,
        tuer: None,
        ..KnotenKonfig::default()
    };
    let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
    let mut kette = Kette::probestand();
    for w in 0..6u8 {
        kette.aufnehmen(probeanmeldung(w));
        kette.aufnehmen(probezusage(w, EpochId(1), 1));
    }
    kette.baue_block();
    while kette.zustand().epoch.0 < 3 {
        kette.baue_block();
    }
    k.kette = kette;
    k
}

/// Eine laufende Messung an das zweite Mitglied des Pods, mit
/// `in_folge` Abweisungen seit `seit`.
fn messung(k: &Knoten, epoche: u64, in_folge: u32, seit: Duration) -> (MinerId, Messung) {
    let z = k.kette.zustand();
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(z);
    let pod = &zuteilung.pods[0];
    let mitglied = pod.shards[1].miner.miner_id;
    let frist = crate::frist::frist_des_mitglieds(z, pod.pod_index, &mitglied).expect("Frist");
    let jetzt = std::time::Instant::now();
    let m = Messung {
        pod: pod.pod_index,
        mitglied,
        ab: jetzt,
        positionen: crate::frist::probe_positionen(frist),
        frist,
        probe_us: None,
        proben: 0,
        saat: [0u8; 32],
        epoche,
        messende: crate::frist::messende(z, &mitglied).len(),
        abgewiesen_in_folge: in_folge,
        abgewiesen_seit: Some(jetzt.checked_sub(seit).unwrap_or(jetzt)),
        abgewiesen: in_folge,
        wieder_ab: None,
    };
    (mitglied, m)
}

/// Eine Abweisung lässt die Messung stehen und fragt mit Abstand neu;
/// erst eine lange Folge endet als Überschreitung, im ersten Fenster
/// als Verdacht.
#[tokio::test]
async fn eine_lange_verweigerung_wird_im_knoten_zum_verdacht() {
    let mut k = knoten_mit_pod("verweigerung").await;
    let e = k.kette.zustand().epoch.0;
    let von = libp2p::PeerId::random();
    // ⛔️ Gegenprobe 1: drei in Folge über eine Stunde, die vierte
    //    kommt erst jetzt. Vier sind nötig, die Dauer genügt: Verdacht.
    let (mitglied, m) = messung(&k, e, 3, Duration::from_secs(3_600));
    k.messungen.insert(von, m);
    k.nichts_erhalten(von, "nichts", true);
    assert!(!k.messungen.contains_key(&von), "die Messung endet");
    assert!(k.fristverdacht.contains_key(&(e, mitglied)), "als Verdacht");
    // ⛔️ Gegenprobe 2: die vierte in Folge, aber erst seit einer
    //    Sekunde: weiter fragen, kein Verdacht.
    k.fristverdacht.clear();
    let (mitglied, m) = messung(&k, e, 3, Duration::from_secs(1));
    k.messungen.insert(von, m);
    k.nichts_erhalten(von, "nichts", true);
    let m = k.messungen.get(&von).expect("die Messung bleibt");
    assert_eq!(m.abgewiesen_in_folge, 4);
    assert!(m.wieder_ab.is_some(), "mit Abstand neu fragen");
    assert!(!k.fristverdacht.contains_key(&(e, mitglied)));
    // ⛔️ Gegenprobe 3: eine Stunde, aber erst die dritte in Folge.
    let (mitglied, m) = messung(&k, e, 2, Duration::from_secs(3_600));
    k.messungen.insert(von, m);
    k.nichts_erhalten(von, "gescheitert", true);
    assert!(k.messungen.get(&von).is_some_and(|m| m.wieder_ab.is_some()));
    assert!(!k.fristverdacht.contains_key(&(e, mitglied)));
    // Ein `Nichts`, während die Messung wartet, zählt nicht.
    k.nichts_erhalten(von, "nichts", true);
    assert_eq!(k.messungen.get(&von).map(|m| m.abgewiesen_in_folge), Some(3));        aufraeumen("verweigerung");
}

/// ⛔️ **Nur eine Antwort auf die Zeitprobe zählt** (2026-10-30): Ein
/// `Nichts` oder ein Fehlschlag ohne den Zweck der Probe, etwa auf eine
/// Blocknachforderung an dasselbe Mitglied, berührt die Messung nicht;
/// mit dem Zweck ist es eine Abweisung.
#[tokio::test]
async fn nur_die_antwort_auf_die_probe_zaehlt() {
    let mut k = knoten_mit_pod("zweck").await;
    let e = k.kette.zustand().epoch.0;
    let von = libp2p::PeerId::random();
    let (_, m) = messung(&k, e, 0, Duration::from_secs(0));
    k.messungen.insert(von, m);
    let nichts = Nachlieferung::Nichts.als_bytes().expect("Nichts");
    k.ereignis_einspeisen(NodeEvent::AntwortEingegangen { von, daten: nichts.clone(), zweck: None });
    k.ereignis_einspeisen(NodeEvent::AnfrageGescheitert { an: von, grund: "Timeout".into(), zweck: None });
    assert_eq!(k.messungen.get(&von).map(|m| (m.abgewiesen, m.wieder_ab.is_some())), Some((0, false)), "ohne Zweck nichts");
    k.ereignis_einspeisen(NodeEvent::AntwortEingegangen { von, daten: nichts, zweck: Some(ZWECK_ZEITPROBE) });
    assert_eq!(k.messungen.get(&von).map(|m| m.abgewiesen), Some(1), "die Probe wurde abgewiesen");
    aufraeumen("zweck");
}

/// 📌 **Fund 674:** Ein Wählfehler ist keine Abweisung: Die Messung
/// bleibt, zählt nichts und fragt später neu; auch nach einer langen
/// Folge solcher Fehler kein Verdacht. Der Text von libp2p ist
/// festgehalten, damit ein Update ihn nicht still ändert.
#[tokio::test]
async fn nicht_erreicht_ist_keine_weigerung() {
    let mut k = knoten_mit_pod("unerreicht").await;
    let e = k.kette.zustand().epoch.0;
    let von = libp2p::PeerId::random();
    assert!(ist_waehlfehler(&libp2p::request_response::OutboundFailure::DialFailure.to_string()));
    // ⛔️ Gegenproben: andere Fehler sind keine Wählfehler.
    assert!(!ist_waehlfehler(&libp2p::request_response::OutboundFailure::Timeout.to_string()));
    assert!(!ist_waehlfehler(&libp2p::request_response::OutboundFailure::ConnectionClosed.to_string()));
    let (mitglied, m) = messung(&k, e, 3, Duration::from_secs(3_600));
    k.messungen.insert(von, m);
    for _ in 0..10 {
        if let Some(m) = k.messungen.get_mut(&von) {
            m.wieder_ab = None;
        }
        k.messung_unerreicht(von);
    }
    let m = k.messungen.get(&von).expect("die Messung bleibt");
    assert_eq!(m.abgewiesen_in_folge, 3, "nicht gezählt");
    assert!(m.wieder_ab.is_some(), "später neu fragen");
    assert!(!k.fristverdacht.contains_key(&(e, mitglied)), "kein Verdacht");
    // ⛔️ Gegenprobe: dieselbe Lage mit einer Abweisung endet im Verdacht.
    if let Some(m) = k.messungen.get_mut(&von) {
        m.wieder_ab = None;
    }
    k.nichts_erhalten(von, "nichts", true);
    assert!(k.fristverdacht.contains_key(&(e, mitglied)));
    aufraeumen("unerreicht");
}

/// 📌 **Fund 670:** Eine Messung aus der vorigen Epoche wird nicht mehr
/// bewertet; dieselbe aus der laufenden ergibt den Verdacht.
#[tokio::test]
async fn eine_messung_aus_der_vorigen_epoche_verfaellt() {
    let mut k = knoten_mit_pod("verfall").await;
    let e = k.kette.zustand().epoch.0;
    let (mitglied, mut m) = messung(&k, e - 1, 0, Duration::ZERO);
    m.probe_us = Some(u64::MAX / 2);
    k.messung_auswerten(m.clone(), "beantwortet");
    assert!(!k.fristverdacht.contains_key(&(e, mitglied)), "verfallen");
    // ⛔️ Gegenprobe: in der laufenden Epoche ein Verdacht.
    m.epoche = e;
    k.messung_auswerten(m, "beantwortet");
    assert!(k.fristverdacht.contains_key(&(e, mitglied)));        aufraeumen("verfall");
}
