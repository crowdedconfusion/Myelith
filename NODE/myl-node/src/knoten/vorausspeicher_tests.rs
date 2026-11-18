use super::*;
use crate::kette::Kette;

fn ordner(marke: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("myl-voraus-{marke}-{}", std::process::id()))
}

/// 📌 **Ein Erzeuger, der nur selbst baut, sieht den fremden Zweig im
/// Vorausspeicher** (Podlauf `kurz-faelscher`, 2026-11-06): Die Blöcke
/// eines anderen Zweigs liegen dort, weil sie weit voraus ankamen; das
/// Nachholen überspringt ihre Höhen. Nach dem eigenen Block schliesst der
/// fremde an der nächsten Höhe nicht an, und die Zweigsuche beginnt bei
/// dem, der ihn schickte. Bis dahin baute ein Validator so 23 Minuten
/// allein auf einer Gabel an Block 1.
#[tokio::test]
async fn ein_fremder_zweig_im_vorausspeicher_startet_die_suche() {
    let d = ordner("zweig");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("Ordner");
    let konfig = KnotenKonfig {
        name: "voraus".to_string(),
        schluesseldatei: d.join("knoten.key"),
        protokollverzeichnis: d.clone(),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        kettendatei: None,
        beobachtung: None,
        tuer: None,
        ..KnotenKonfig::default()
    };
    let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
    let mut eigen = Kette::probestand();
    let s = eigen.letzter_slot();
    eigen.baue_block_im_slot(s + 7);
    k.kette = eigen;
    let mut fremd = Kette::probestand();
    for i in 1..5 {
        fremd.baue_block_im_slot(s + i);
    }
    let von = libp2p::PeerId::random();
    for b in fremd.bloecke_von_bis(3, 4) {
        let roh = borsh::to_vec(&b).expect("Drahtform");
        assert_eq!(k.voraus.ablegen(k.kette.hoehe(), b, &roh, von), Ok(()));
    }
    assert!(k.zweigsuche.is_none());
    // Ohne Nachbarn geht er nirgends hin; gebaut ist er trotzdem.
    let _ = k.erzeuge_block().await;
    assert_eq!(k.kette.hoehe(), 2, "der eigene Block 2");
    assert!(k.zweigsuche.as_ref().is_some_and(|z| z.bei == von), "die Suche fragt, wer den fremden Block schickte");
    let _ = std::fs::remove_dir_all(&d);
}

/// 📌 **Einen Anker, dessen Abzug keiner hat, gibt der Knoten auf**
/// (Podlauf `kurz-verstummen`, 2026-11-07): Er fragte sonst reihum ohne
/// Ende und blieb bei Hoehe 0. Erst wenn jede Gegenstelle nichts hatte;
/// danach sucht er neu, und derselbe Anker zaehlt nicht mehr.
#[tokio::test]
async fn einen_anker_ohne_abzug_gibt_er_auf() {
    let d = ordner("anker");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("Ordner");
    let konfig = KnotenKonfig {
        name: "anker".to_string(),
        schluesseldatei: d.join("knoten.key"),
        protokollverzeichnis: d.clone(),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        kettendatei: None,
        beobachtung: None,
        tuer: None,
        ..KnotenKonfig::default()
    };
    let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
    let (a, b) = (libp2p::PeerId::random(), libp2p::PeerId::random());
    k.gegenstellen = vec![a, b];
    let anker = crate::abzug::Anker { hoehe: 8850, blockhash: myl_types::Hash::from_bytes([7u8; 32]) };
    k.abzug_holen = Some(AbzugHolen::neu_als(anker, crate::abzug::Art::Leicht));
    k.abzug_nichts(a);
    assert!(k.abzug_holen.is_some(), "die andere Gegenstelle kann ihn noch haben");
    assert!(k.anker_suchen.is_none());
    k.abzug_nichts(b);
    assert!(k.abzug_holen.is_none(), "keine hatte ihn");
    assert!(k.anker_suchen.is_some(), "er sucht neu");
    assert!(k.anker_verworfen.contains(&8850));
    let _ = std::fs::remove_dir_all(&d);
}

/// Eine Stimmsatzdatei mit vier Probeschluesseln.
fn stimmsatz_in(d: &std::path::Path, start: Option<u64>) -> std::path::PathBuf {
    let mut text = String::from("netz probe\n");
    if let Some(s) = start {
        text.push_str(&format!("start {s}\n"));
    }
    for name in ["a", "b", "c", "d"] {
        text.push_str(&crate::schluessel::Konsensschluessel::probe(name).expect("Schluessel").genesiszeile(100).expect("Zeile"));
        text.push('\n');
    }
    let p = d.join("stimmsatz.txt");
    std::fs::write(&p, text).expect("schreiben");
    p
}

fn einsteiger(d: &std::path::Path, start: Option<u64>, netz: &'static crate::netz::Netz) -> KnotenKonfig {
    KnotenKonfig {
        name: "einsteiger".to_string(),
        netz,
        schluesseldatei: d.join("knoten.key"),
        protokollverzeichnis: d.to_path_buf(),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        kettendatei: None,
        beobachtung: None,
        tuer: None,
        stimmsatzdatei_pfad: Some(stimmsatz_in(d, start)),
        ..KnotenKonfig::default()
    }
}

fn einstiegsordner(marke: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("myl-einstieg-{marke}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("Ordner");
    d
}

/// ⚑ **Ohne eigenen Stand: nur ab einem Punkt, der noch traegt** (A43).
/// Gegenprobe: ohne `einstieg_pruefen` startet der Knoten in allen Faellen.
#[tokio::test]
async fn ein_knoten_ohne_stand_steigt_nur_ab_einem_jungen_punkt_ein() {
    let d = einstiegsordner("punkt");
    let jetzt = crate::protokoll::jetzt_ms().max(0) as u64;
    let netz = &crate::netz::PROBEKETTE;
    let fehler = |e: Result<Knoten, KnotenFehler>| e.err().map(|f| format!("{f:?}"));
    let alt = fehler(Knoten::starten(einsteiger(&d, Some(0), netz), false).await).expect("ein altes Netz ohne Anker");
    assert!(alt.contains("Vertrauensfrist"), "{alt}");
    let ohne = fehler(Knoten::starten(einsteiger(&d, None, netz), false).await).expect("keine Startzeit, kein Anker");
    assert!(ohne.contains("Startzeit"), "{ohne}");
    assert!(Knoten::starten(einsteiger(&d, Some(jetzt), netz), false).await.is_ok(), "ein junges Netz");
    let slot = jetzt / KnotenKonfig::default().slot_ms();
    let frisch = crate::wechsel::AnkerVonAussen { hoehe: 150, block: myl_types::Hash::from_bytes([1; 32]), bindung: myl_types::Hash::from_bytes([2; 32]), slot };
    let mut k = einsteiger(&d, Some(0), netz);
    k.anker_von_aussen = Some(frisch);
    assert!(Knoten::starten(k, false).await.is_ok(), "ein altes Netz mit frischem Anker");
    let mut k = einsteiger(&d, Some(0), netz);
    k.anker_von_aussen = Some(crate::wechsel::AnkerVonAussen { slot: 0, ..frisch });
    let zu_alt = fehler(Knoten::starten(k, false).await).expect("ein alter Anker");
    assert!(zu_alt.contains("--anker ist aelter"), "{zu_alt}");
    let _ = std::fs::remove_dir_all(&d);
}

/// ⛔️ **Schweigen laesst den Anker nicht aus** (A43): Findet die Suche
/// keinen Kandidaten, geht ein Knoten mit `--anker` nicht ab Genesis
/// weiter, sondern sucht weiter. Ohne Anker in einem jungen Netz schon.
#[tokio::test]
async fn schweigen_laesst_den_anker_nicht_aus() {
    let d = einstiegsordner("schweigen");
    let jetzt = crate::protokoll::jetzt_ms().max(0) as u64;
    let netz = &crate::netz::PROBEKETTE;
    let mut konfig = einsteiger(&d, Some(jetzt), netz);
    konfig.anker_von_aussen = Some(crate::wechsel::AnkerVonAussen { hoehe: 150, block: myl_types::Hash::from_bytes([1; 32]), bindung: myl_types::Hash::from_bytes([2; 32]), slot: jetzt / KnotenKonfig::default().slot_ms() });
    let mut mit = Knoten::starten(konfig, false).await.expect("Knoten");
    assert!(mit.anker_suchen.is_some());
    mit.anker_keine = 1;
    mit.anker_suche_pflegen();
    assert!(mit.anker_suchen.is_some(), "er sucht weiter");
    assert_eq!(mit.anker_keine, 0);
    let mut ohne = Knoten::starten(einsteiger(&d, Some(jetzt), netz), false).await.expect("Knoten");
    ohne.anker_keine = 1;
    ohne.anker_suche_pflegen();
    assert!(ohne.anker_suchen.is_none(), "ab Genesis, solange das Netz jung ist");
    let _ = std::fs::remove_dir_all(&d);
}

/// ⛔️ **Ein Block mit erfundener Epoche haelt niemanden an** (A46). Bis
/// 0.117.0 kam die Epoche aus dem noch ungeprueften Kopf, und eine einzige
/// Nachricht beendete jeden vollen Knoten ausserhalb der Probekette.
/// Gegenprobe: mit `block.header.epoch` im Vergleich haelt er an.
#[tokio::test]
async fn ein_block_mit_erfundener_epoche_haelt_niemanden_an() {
    let d = einstiegsordner("epoche");
    let jetzt = crate::protokoll::jetzt_ms().max(0) as u64;
    let mut k = Knoten::starten(einsteiger(&d, Some(jetzt), &crate::netz::TESTNETZ_INT4), false).await.expect("Knoten");
    let mut block = Kette::probestand().baue_block();
    block.header.epoch = 1_000_000;
    let daten = borsh::to_vec(&block).expect("kodieren");
    k.verarbeite(&myl_net::InboundMessage { topic: GossipTopic::Blocks, data: daten, von: libp2p::PeerId::random(), pruefmarke: None });
    assert_eq!(k.angehalten(), None);
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Der Puffer ist gedeckelt, und zugleich laufen nur wenige Stapel**
/// (A47). Gegenproben: ohne die Grenze in `tx_annehmen` wächst der Puffer,
/// ohne den Platz in `tx_puffer_pflegen` leert er sich trotz belegter
/// Plätze.
#[tokio::test]
async fn eine_flut_von_transaktionen_bleibt_gedeckelt() {
    let d = einstiegsordner("flut");
    let konfig = KnotenKonfig {
        name: "flut".to_string(),
        schluesseldatei: d.join("knoten.key"),
        protokollverzeichnis: d.clone(),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        kettendatei: None,
        beobachtung: None,
        tuer: None,
        ..KnotenKonfig::default()
    };
    let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
    let tx = myl_consensus::block::Transaktion {
        absender: myl_types::bls::BlsPublicKey([0; myl_types::bls::BLS_PK_LEN]),
        nonce: 0,
        max_gebuehr: 0,
        anweisung: myl_consensus::block::Anweisung::Burn { betrag: 1 },
        signatur: myl_types::bls::BlsSignature([0; myl_types::bls::BLS_SIG_LEN]),
    };
    for _ in 0..TX_PUFFER_HOECHSTENS + 10 {
        k.tx_annehmen(tx.clone(), None);
    }
    assert_eq!(k.tx_puffer.len(), TX_PUFFER_HOECHSTENS);
    let belegt: Vec<_> = (0..TX_PRUEFUNGEN_ZUGLEICH).map(|_| k.tx_plaetze.nehmen().expect("Platz")).collect();
    k.tx_puffer_pflegen();
    assert_eq!(k.tx_puffer.len(), TX_PUFFER_HOECHSTENS, "alle Plätze belegt, kein neuer Stapel");
    drop(belegt);
    k.tx_puffer_pflegen();
    assert_eq!(k.tx_puffer.len(), TX_PUFFER_HOECHSTENS - TX_STAPEL_HOECHSTENS, "ein Stapel je Runde");
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Eine Reserve lässt sich nicht hin und her schicken** (A49).
/// Gegenprobe: ohne die Pause gilt der zweite Auftrag.
#[tokio::test]
async fn eine_uebernahme_haelt_ihre_pause() {
    let d = einstiegsordner("uebernahme");
    let konfig = KnotenKonfig {
        name: "uebernahme".to_string(),
        schluesseldatei: d.join("knoten.key"),
        protokollverzeichnis: d.clone(),
        horchadressen: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
        kettendatei: None,
        beobachtung: None,
        tuer: None,
        ..KnotenKonfig::default()
    };
    let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
    let a = crate::shardfuehrung::Shardauftrag { epoche: 1, pod: 0, position: 0, positionen: 2, von: 0, bis: 4 };
    let b = crate::shardfuehrung::Shardauftrag { position: 1, ..a.clone() };
    let t = std::time::Instant::now();
    assert_eq!(k.uebernahme_setzen(a.clone(), true, t), Ok(()));
    assert_eq!(k.uebernahme_setzen(a.clone(), true, t + Duration::from_secs(1)), Ok(()), "derselbe Auftrag noch einmal");
    assert!(k.uebernahme_setzen(b.clone(), true, t + Duration::from_secs(1)).is_err());
    assert_eq!(k.uebernahme, Some(a.clone()), "der erste bleibt");
    assert_eq!(k.uebernahme_setzen(b.clone(), true, t + UEBERNAHME_PAUSE), Ok(()));
    assert_eq!(k.uebernahme, Some(b.clone()));

    // A49, der Absender: Ein Fremder wartet länger und kann einen Miner nicht verdrängen.
    let t2 = t + UEBERNAHME_PAUSE * 2;
    assert!(k.uebernahme_setzen(a.clone(), false, t2).is_err(), "Fremder in der Pause eines Miners");
    assert_eq!(k.uebernahme, Some(b.clone()));
    let t3 = t + UEBERNAHME_PAUSE + UEBERNAHME_PAUSE_FREMD;
    assert_eq!(k.uebernahme_setzen(a.clone(), false, t3), Ok(()), "nach seiner langen Pause");
    // Ein Miner verdrängt den Auftrag eines Fremden sofort und blockiert dann selbst.
    assert_eq!(k.uebernahme_setzen(b.clone(), true, t3 + Duration::from_secs(1)), Ok(()));
    assert_eq!(k.uebernahme, Some(b.clone()));
    assert!(k.uebernahme_setzen(a.clone(), true, t3 + Duration::from_secs(2)).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

fn einfacher_knoten(marke: &str) -> (std::path::PathBuf, KnotenKonfig) {
    let d = einstiegsordner(marke);
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
    (d, konfig)
}

/// ⚑ **Jede teure Anfrage geht durch die Drossel** (A48): Eine Flut von
/// einer Gegenstelle wird ab der Fassung des Eimers abgewiesen, je Art
/// der Anfrage. Gegenprobe: eine Art aus der Liste in `vermerke` nehmen,
/// dann läuft ihre Flut ungedrosselt durch.
#[tokio::test]
async fn jede_teure_anfrage_geht_durch_die_drossel() {
    use crate::nachschub::Nachforderung as N;
    use myl_types::ids::{EpochId, PodId};
    let (d, konfig) = einfacher_knoten("drossel");
    let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
    let frage = myl_types::Spuranfrage { epoche: EpochId(0), pod: PodId::new([1; 32]), segment: 0 };
    let arten = [
        N::Wechselbelege { ab: 0 },
        N::Abzug { hoehe: 1, ab: 0 },
        N::Datei { name: "x".to_string(), ab: 0, laenge: 1 },
        N::Spur(frage),
        N::Verlauf(frage),
        N::VerlaufTeil { anfrage: frage, ab: 0 },
        N::Schiedsbeleg(frage),
        N::SchiedsbelegTeil { anfrage: frage, ab: 0 },
    ];
    for art in arten {
        let daten = art.als_bytes().expect("kodieren");
        let von = libp2p::PeerId::random();
        let vorher = k.gedrosselt;
        for marke in 0..64 {
            k.vermerke(NodeEvent::AnfrageEingegangen { von, daten: daten.clone(), marke });
        }
        assert!(k.gedrosselt > vorher, "ungedrosselt: {art:?}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Was je Gegenstelle gemerkt wird, geht mit ihr** (A48). Gegenprobe:
/// ohne die Zeilen in `Getrennt` bleibt es stehen.
#[tokio::test]
async fn eine_getrennte_gegenstelle_hinterlaesst_nichts() {
    let (d, konfig) = einfacher_knoten("getrennt");
    let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
    let peer = libp2p::PeerId::random();
    k.latenz_je_peer.insert(peer, 5);
    k.leer_geantwortet.insert(peer, (1, std::time::Instant::now()));
    k.zweig_gesperrt.insert(peer, std::time::Instant::now());
    k.vermerke(NodeEvent::Getrennt { peer, grund: "probe".to_string() });
    assert!(!k.latenz_je_peer.contains_key(&peer));
    assert!(!k.leer_geantwortet.contains_key(&peer));
    assert!(!k.zweig_gesperrt.contains_key(&peer));
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Der Deckel einer Stimme steht vor ihrer Prüfung** (A48): Eine zu
/// weit vorausliegende kostet nichts. Gegenprobe: mit dem Deckel hinter
/// `traegt` schreibt sie ihre Ablehnung ins Protokoll.
#[tokio::test]
async fn eine_stimme_zu_weit_voraus_kostet_nichts() {
    let (d, konfig) = einfacher_knoten("stimme");
    let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
    let stimme = myl_consensus::pruefpunkt::Pruefpunktstimme {
        hoehe: 4 * myl_consensus::pruefpunkt::BLOECKE_JE_PRUEFPUNKT + 1,
        block_hash: myl_types::Hash::from_bytes([1; 32]),
        bindung: myl_types::Hash::from_bytes([2; 32]),
        quelle: myl_consensus::pruefpunkt::Quelle::GENESIS,
        waehler: myl_types::ids::MinerId::new([3; 32]),
        signatur: myl_types::bls::BlsSignature([0; myl_types::bls::BLS_SIG_LEN]),
    };
    let vorher = k.protokoll.geschrieben();
    k.pruefpunktstimme_aufnehmen(stimme);
    assert_eq!(k.protokoll.geschrieben(), vorher);
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Ein Block ohne belegten Erzeuger geht nicht weiter** (A47), ein
/// belegter schon. Gegenprobe: `Annehmen` in beiden Zweigen.
#[tokio::test]
async fn nur_ein_belegter_block_wird_weitergereicht() {
    let d = einstiegsordner("urteil");
    let jetzt = crate::protokoll::jetzt_ms().max(0) as u64;
    let testnetz = Knoten::starten(einsteiger(&d, Some(jetzt), &crate::netz::TESTNETZ_INT4), false).await.expect("Knoten");
    let fremd = Kette::probestand().baue_block();
    assert_eq!(testnetz.urteil_ueber_block(&fremd), myl_net::Urteil::Ignorieren);
    let (d2, konfig) = einfacher_knoten("urteil2");
    let probe = Knoten::starten(konfig, false).await.expect("Knoten");
    assert_eq!(probe.urteil_ueber_block(&fremd), myl_net::Urteil::Annehmen, "die Probekette lernt ihren Erzeuger");
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&d2);
}

/// ⚑ **Eine halbfertige Ablage verdrängt nur, was weniger Bytes hat**
/// (A48). Gegenprobe: nach Alter statt nach Stand fällt die grosse.
#[tokio::test]
async fn eine_halbfertige_ablage_wird_nicht_von_einem_byte_verdraengt() {
    let (d, konfig) = einfacher_knoten("ablage");
    let mut k = Knoten::starten(konfig, false).await.expect("Knoten");
    let gross = myl_types::Hash::from_bytes([1; 32]);
    let peer = libp2p::PeerId::random();
    assert!(k.ablage_stueck(Some(peer), 1_000, gross, 0, &[7; 500]).is_ok());
    for i in 2..VERLAUFTEILE_ZUGLEICH as u8 + 2 {
        let _ = k.ablage_stueck(Some(libp2p::PeerId::random()), 1_000, myl_types::Hash::from_bytes([i; 32]), 0, &[7; 1]);
    }
    assert!(k.ablage_teile.contains_key(&(Some(peer), gross)), "die grosse ist noch da");
    assert!(k.ablage_teile.len() <= VERLAUFTEILE_ZUGLEICH);
    let _ = std::fs::remove_dir_all(&d);
}

/// ⚑ **Der Sitzungsstand zählt Credits wie die Kette** (Fund 707):
/// Verfallenes gar nicht, und `credits_haltbar` nur, was den nächsten
/// Wechsel übersteht. Gegenprobe: Wer `haltbar` wie `credits` zählt,
/// bekommt 900 statt 500; eine unbekannte Sitzung hat keinen Stand.
#[test]
fn der_sitzungsstand_trennt_geltende_und_haltbare_credits() {
    use myl_types::ids::EpochId;
    use myl_types::sitzung::{Grenzen, Sitzungskontrakt, Sitzungszustand};
    let inhaber = myl_types::ids::Address::new([5; 32]);
    let k = Sitzungskontrakt {
        inhaber,
        agent: myl_types::ids::Address::new([6; 32]),
        credits: Grenzen { budget: 4_000, einzellimit: 1_000, schwelle: u64::MAX, zeugenleiter: Vec::new() },
        myl: Grenzen::gesperrt(),
        empfaenger_wurzel: myl_types::Hash::from_bytes([0; 32]),
        gueltig_ab: EpochId(1),
        gueltig_bis: EpochId(9),
        max_schritte: 10,
    };
    let id = k.adresse();
    let mut z = myl_ledger::LedgerState::genesis(67);
    z.epoch = EpochId(3);
    z.account_mut(&inhaber).balance = 1_000_000;
    for (vtfe, verfall) in [(300u64, 2u64), (400, 3), (500, 4)] {
        z.account_mut(&inhaber).credits.push(myl_types::InferenceCredit { owner: inhaber, vtfe, expiry: EpochId(verfall) });
    }
    let mut zustand = Sitzungszustand::neu();
    zustand.verbraucht_credits = 120;
    zustand.hoechste_abrechnung = 2;
    z.sitzungen.insert(id, myl_ledger::state::Sitzung { kontrakt: k, zustand });
    let je = myl_consensus::block::BLOECKE_JE_EPOCHE;
    let s = sitzungsstand(&z, 3 * je + 1, 2_000, &id).expect("Stand");
    assert_eq!((s.credits, s.credits_haltbar), (900, 500));
    assert_eq!((s.budget, s.einzellimit, s.verbraucht, s.hoechste_abrechnung), (4_000, 1_000, 120, 2));
    assert_eq!((s.epoche, s.credit_preis, s.guthaben, s.gueltig_bis), (3, 67, 1_000_000, 9));
    assert_eq!(s.bloecke_bis_wechsel, je - 1);
    assert_eq!(sitzungsstand(&z, 4 * je - 1, 2_000, &id).expect("Stand").bloecke_bis_wechsel, 1);
    assert_eq!(sitzungsstand(&z, 4 * je, 2_000, &id).expect("Stand").bloecke_bis_wechsel, je);
    assert!(sitzungsstand(&z, 0, 2_000, &myl_types::ids::SitzungId::new([9; 32])).is_none());
}
