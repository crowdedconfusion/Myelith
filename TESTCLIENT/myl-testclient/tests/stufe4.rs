//! **Stufe 4 über den ganzen Weg** (P9b, 2026-10-22): Gegenproben an einer
//! echten Kette, mit echtem Shardprozess und echtem Artefakt.
//!
//! # ⚑ Was hier zusammenkommt
//!
//! Zusage über die Kette, Pod-Bildung nach Zusagen, Schnitt, Gutschrift,
//! Fristmeldung, Sperre; der Knoten startet seinen Shard aus der Kette und
//! misst ihn über die Tür; der Dateidienst holt einen Bereich des
//! Probeartefakts. Jede Probe für sich prüft einen Teil, diese Datei prüft,
//! dass die Teile ineinandergreifen.
//!
//! Die Gegenproben aus dem Plan: Überzusage (die Frist greift), Gleichstand
//! (zwei Knoten, dieselben Grenzen), zu wenig Kapazität (kein Pod statt
//! eines, der nicht rechnen kann), ein Mitglied mit Ebenenzeit 1 (die
//! Kapazität begrenzt), Teilablage (der Bereich bleibt im gehaltenen Teil).

use std::path::PathBuf;

use myl_consensus::block::{Anweisung, Transaktion};
use myl_node::kette::{probeanmeldung, probekonto, probeschluessel, Kette};
use myl_types::ids::{EpochId, MinerId};
use myl_types::zusage::{Kapazitaetszusage, Modellzusage};

fn wurzel() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn sparrow() -> PathBuf {
    wurzel().join("INTEGER_LLM").join("artifacts").join("sparrow-0.6b-int8")
}

fn kennung(w: u8) -> MinerId {
    MinerId::new(*probekonto(w).as_bytes())
}

/// Eine Zusage des Probekontos `w` über die Kette, ab Epoche 1.
fn zusage(w: u8, ebenen_hoechstens: u32, ebenenzeit_us: u64, gehalten: Option<(u32, u32)>) -> Transaktion {
    let sk = probeschluessel(w);
    let abdruck = myl_node::netz::PROBEKETTE.podmodell.abdruck;
    let mut z = Kapazitaetszusage::neu(kennung(w), EpochId(1), 0).mit_modell(Modellzusage {
        abdruck,
        ebenen_hoechstens,
        ebenenzeit_us,
        gehalten,
    });
    z.signiere(&sk).expect("signieren");
    Transaktion::signiere(&Kette::startwert(), &sk, 1, Anweisung::ZusageEinreichen { zusage: z }).expect("signieren")
}

/// Eine Kette mit sechs Minern und ihren Zusagen, bis Epoche 2 (der
/// Registrierungsschluss liegt dann hinter ihnen).
/// Je Miner: Ebenen samt Kopf, Ebenenzeit, gehaltener Bereich.
type Probezusage = (u32, u64, Option<(u32, u32)>);

fn kette_mit(zusagen: &[Probezusage]) -> Kette {
    let mut k = Kette::probestand();
    for (w, &(e, t, g)) in zusagen.iter().enumerate() {
        k.aufnehmen(probeanmeldung(w as u8));
        k.aufnehmen(zusage(w as u8, e, t, g));
    }
    k.baue_block();
    while k.zustand().epoch.0 < 2 {
        k.baue_block();
    }
    k
}

/// ⚑ **Überzusage: Wer schneller ansagt, als er rechnet, verliert seine
/// Zusage.** Ein Mitglied sagt 1 µs je Ebene an und bekommt dafür viele
/// Ebenen; drei andere messen, was es wirklich braucht, unterschreiben,
/// und die Meldung geht über die Kette. In der nächsten Epoche steht es in
/// keinem Pod, und sein Konto trägt einen Verstoß; danach gilt die Zusage
/// wieder.
#[test]
fn eine_ueberzusage_wird_gemeldet_und_gesperrt() {
    // Das schnelle Mitglied muss an einer Position stehen, sonst misst
    // niemand seinen Bereich. Die Besetzung hängt an der Saat und nicht an
    // den Zahlen der Zusagen; gesucht wird darum das erste Probekonto, das
    // eine Position bekommt.
    let mut gefunden = None;
    for schnell in 0..6u8 {
        let mut zusagen = vec![(48u32, 300u64, None); 6];
        zusagen[schnell as usize] = (48, 1, None);
        let k = kette_mit(&zusagen);
        let z = Kette::zuteilung_der_laufenden_epoche(k.zustand());
        if z.pods.first().is_some_and(|p| p.shards.iter().any(|s| s.miner.miner_id == kennung(schnell))) {
            gefunden = Some((schnell, k));
            break;
        }
    }
    let (schnell, mut k) = gefunden.expect("ein Probekonto an einer Position");
    let zuteilung = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert_eq!(zuteilung.pods.len(), 1, "sechs Miner mit Zusage, ein Pod");
    let pod = &zuteilung.pods[0];
    let stelle = pod.shards.iter().position(|s| s.miner.miner_id == kennung(schnell)).expect("Position");
    let grenzen = Kette::grenzen_des_pods(k.zustand(), pod).expect("machbar");
    let ebenen = grenzen[stelle].1 - grenzen[stelle].0;
    let frist = myl_node::frist::frist_des_mitglieds(k.zustand(), pod.pod_index, &kennung(schnell)).expect("Frist");
    // Was es wirklich braucht: 300 µs je Ebene, wie die anderen.
    let wirklich = 300 * (ebenen as u64 + if stelle + 1 == pod.shards.len() { 5 } else { 0 });
    assert!(wirklich > frist, "die Probe misst keine Überzusage: {wirklich} gegen {frist}");
    // Drei Zeugen aus dem Pod messen und unterschreiben.
    let zeugen: Vec<u8> = (0..6u8).filter(|w| *w != schnell && pod.mitglieder().any(|m| m.miner_id == kennung(*w))).take(3).collect();
    let mut sammlung = myl_node::frist::Fristsammlung::default();
    let epoche = k.zustand().epoch;
    let mut meldung = None;
    for w in &zeugen {
        let z = myl_types::fristmeldung::Fristzeugnis::neu(&probeschluessel(*w), kennung(*w), epoche, pod.pod_index, &kennung(schnell), wirklich)
            .expect("Zeugnis");
        meldung = sammlung.aufnehmen(k.zustand(), epoche, pod.pod_index, kennung(schnell), z).or(meldung);
    }
    let meldung = meldung.expect("drei von fünf über der Frist");
    let einreicher = (0..6u8).find(|w| kennung(*w) == meldung.zeugnisse[0].zeuge).expect("Zeuge");
    k.aufnehmen(
        Transaktion::signiere(&Kette::startwert(), &probeschluessel(einreicher), 2, Anweisung::FristverstossEinreichen { meldung })
            .expect("signieren"),
    );
    k.baue_block();
    assert_eq!(
        k.zustand().fristsperren.get(&kennung(schnell)),
        Some(&EpochId(epoche.0 + 1)),
        "die Meldung kam nicht an"
    );
    assert_eq!(k.zustand().verstoesse_im_fenster(&probekonto(schnell), myl_ledger::state::VERSTOSS_FENSTER), 1);
    // Die nächste Epoche: ohne Pod.
    while k.zustand().epoch.0 == epoche.0 {
        k.baue_block();
    }
    let danach = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert!(danach.ohne_zusage.iter().any(|m| m.miner_id == kennung(schnell)), "gesperrt heißt ohne Zusage");
    assert!(danach.pods.iter().all(|p| p.mitglieder().all(|m| m.miner_id != kennung(schnell))));
    // Und die übernächste: wieder dabei.
    while k.zustand().epoch.0 == epoche.0 + 1 {
        k.baue_block();
    }
    let spaeter = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert!(spaeter.ohne_zusage.iter().all(|m| m.miner_id != kennung(schnell)), "die Sperre gilt eine Epoche");
}

/// ⚑ **Gleichstand: zwei Knoten, dieselben Grenzen.** Erzeuger und
/// Übernehmer kommen bei gleichen Zusagen zur selben Zuteilung, zum selben
/// Schnitt und zur selben Zustandswurzel.
#[test]
fn gleichstand_ergibt_auf_zwei_knoten_dieselben_grenzen() {
    let mut erzeuger = Kette::probestand();
    let mut uebernehmer = Kette::probestand();
    for w in 0..6u8 {
        erzeuger.aufnehmen(probeanmeldung(w));
        erzeuger.aufnehmen(zusage(w, 48, 300, None));
    }
    while erzeuger.zustand().epoch.0 < 2 {
        let b = erzeuger.baue_block();
        uebernehmer.uebernimm(&b).expect("Übernahme");
    }
    assert_eq!(erzeuger.zustandswurzel(), uebernehmer.zustandswurzel());
    let a = Kette::zuteilung_der_laufenden_epoche(erzeuger.zustand());
    let b = Kette::zuteilung_der_laufenden_epoche(uebernehmer.zustand());
    assert_eq!(a, b);
    let ga = Kette::grenzen_des_pods(erzeuger.zustand(), &a.pods[0]).expect("machbar");
    let gb = Kette::grenzen_des_pods(uebernehmer.zustand(), &b.pods[0]).expect("machbar");
    assert_eq!(ga, gb);
    // Gleich starke Mitglieder: Die Bereiche ohne Kopf unterscheiden sich
    // um höchstens eine Ebene.
    let ohne_kopf: Vec<u32> = ga[..3].iter().map(|(v, b)| b - v).collect();
    assert!(ohne_kopf.iter().max().unwrap() - ohne_kopf.iter().min().unwrap() <= 1, "{ga:?}");
}

/// ⚑ **Zu wenig Kapazität: kein Pod statt eines, der nicht rechnen kann.**
/// Sechs Miner, die zusammen den Kopf und alle Ebenen nicht tragen.
#[test]
fn zu_wenig_kapazitaet_ergibt_keinen_pod() {
    let k = kette_mit(&[(8, 300, None); 6]);
    let z = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    assert!(z.pods.is_empty(), "{z:?}");
    assert_eq!(z.ohne_pod.len(), 6, "sie stehen benannt da, nicht verschwunden");
}

/// ⚑ **Ebenenzeit 1: die Kapazität begrenzt.** Ein Mitglied, das 1 µs
/// ansagt und zehn Ebenen hält, bekommt höchstens zehn (mit Kopf zehn
/// samt Kopf).
#[test]
fn ein_mitglied_mit_ebenenzeit_eins_bekommt_nicht_mehr_als_es_haelt() {
    let mut zusagen = vec![(48u32, 300u64, None); 6];
    zusagen[4] = (10, 1, None);
    let k = kette_mit(&zusagen);
    let z = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    for pod in &z.pods {
        let grenzen = Kette::grenzen_des_pods(k.zustand(), pod).expect("machbar");
        if let Some(i) = pod.shards.iter().position(|s| s.miner.miner_id == kennung(4)) {
            let kopf = if i + 1 == pod.shards.len() { 20 } else { 0 };
            assert!(grenzen[i].1 - grenzen[i].0 + kopf <= 10, "{grenzen:?}");
        }
    }
}

/// ⚑ **Teilablage: der Bereich bleibt im gehaltenen Teil.** Ein Mitglied
/// hält nur die Ebenen 0 bis 9; steht es an einer Position, liegt sein
/// Bereich darin, sonst hat die Pod-Bildung es weichen lassen.
#[test]
fn die_teilablage_haelt_den_bereich() {
    let mut zusagen = vec![(48u32, 300u64, None); 6];
    zusagen[1] = (48, 300, Some((0, 9)));
    let k = kette_mit(&zusagen);
    let z = Kette::zuteilung_der_laufenden_epoche(k.zustand());
    for pod in &z.pods {
        let grenzen = Kette::grenzen_des_pods(k.zustand(), pod).expect("machbar");
        if let Some(i) = pod.shards.iter().position(|s| s.miner.miner_id == kennung(1)) {
            assert!(grenzen[i].1 <= 9, "{grenzen:?}");
        }
    }
}

/// Die Datei eines Konsensschlüssels für ein Probekonto, wie
/// `myl-node` sie schreibt: Kopfzeile, dann der Startwert in Hex.
fn konsensdatei(ordner: &std::path::Path, w: u8) -> PathBuf {
    let ikm = myl_types::hash::Hash::sha256(format!("myelith-probekonto-{w}").as_bytes());
    let p = ordner.join(format!("konsens-{w}.key"));
    std::fs::write(&p, format!("# Probe\n{}\n", ikm.to_hex())).expect("Schlüsseldatei");
    p
}

/// ⚑ **P7 mit echtem Prozess: Der Knoten startet seinen Shard aus der
/// Kette**, mit dem Bereich seiner Position und dem Schlüssel seiner
/// Anmeldung; die Zeitprobe geht über die Tür, und ohne Auftrag endet der
/// Shard.
#[test]
fn der_knoten_startet_seinen_shard_aus_der_kette() {
    // Ohne Probeartefakt nur mit ausdrücklicher Erlaubnis (Fund 113).
    if !myl_pod::artefakte::vorhanden(&sparrow()) {
        return;
    }
    let profil = if cfg!(debug_assertions) { "debug" } else { "release" };
    let programm = wurzel().join("SYSTEM/full-build").join(profil).join("myl-shard");
    assert!(
        programm.exists(),
        "myl-shard ist nicht gebaut. Bauen mit:\n  cd COMPUTE_PIPELINE/myl-pod && cargo test --no-run\n\
         (im Testbau, damit es das Probemodell des Prüfnetzes lädt)"
    );
    let k = kette_mit(&[(48, 300, None); 6]);
    let ordner = std::env::temp_dir().join(format!("myl-stufe4-{}", std::process::id()));
    std::fs::create_dir_all(&ordner).expect("Ordner");
    let ich = (0..6u8).find(|w| myl_node::shardfuehrung::auftrag_der_epoche(k.zustand(), &kennung(*w)).is_some()).expect("eine Position");
    let auftrag = myl_node::shardfuehrung::auftrag_der_epoche(k.zustand(), &kennung(ich)).expect("Auftrag");
    let mut f = myl_node::shardfuehrung::Shardfuehrung::neu(myl_node::shardfuehrung::Shardstart {
        programm,
        artefakte: sparrow(),
        konsensschluessel: konsensdatei(&ordner, ich),
        training: false,
        netzlast: ordner.join(myl_store::teilablage::NETZLAST_DATEI),
    });
    assert!(matches!(f.abgleichen(Some(auftrag.clone())), Ok(myl_node::shardfuehrung::Wechsel::Gestartet(_))));
    let bis = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let (adresse, ausweis) = loop {
        if let Some(a) = f.tuer() {
            break a;
        }
        assert!(std::time::Instant::now() < bis, "der Shard nannte keine Adresse");
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    // Der Shard unterschreibt mit dem Schlüssel der Anmeldung.
    use myl_pod::shardweg::{Shardanfrage, Shardantwort, Shardweg, UeberDenDraht};
    let weg = UeberDenDraht::neu(vec![(adresse, ausweis)]);
    match weg.frage(0, &Shardanfrage::Schluessel) {
        Ok(Shardantwort::Schluessel(pk)) => assert_eq!(pk, probeschluessel(ich).public_key().expect("pk")),
        anders => panic!("{anders:?}"),
    }
    match weg.frage(0, &Shardanfrage::Zuschnitt) {
        Ok(Shardantwort::Zuschnitt(z)) => assert_eq!((z.layer_start, z.layer_end), (auftrag.von as u64, auftrag.bis as u64)),
        anders => panic!("{anders:?}"),
    }
    let t = std::time::Instant::now();
    let abdruck = myl_types::zeitprobe::fragen(adresse, &ausweis, 2, std::time::Duration::from_secs(120)).expect("Zeitprobe");
    // ⚑ Ohne Ausweis antwortet der echte Shard nicht (Fund 611).
    let fremd = UeberDenDraht::neu(vec![(adresse, [0u8; 32])]).mit_frist(std::time::Duration::from_secs(5));
    assert!(fremd.frage(0, &Shardanfrage::Schluessel).is_err(), "der Shard antwortete ohne Ausweis");
    assert_ne!(abdruck, [0; 32]);
    assert!(t.elapsed() > std::time::Duration::ZERO);
    let last = myl_store::teilablage::Netzlast::lesen(&ordner.join(myl_store::teilablage::NETZLAST_DATEI)).expect("Netzlast");
    assert_eq!((last.von, last.bis), (auftrag.von, auftrag.bis));
    assert_eq!(f.abgleichen(None), Ok(myl_node::shardfuehrung::Wechsel::Beendet));
    let _ = std::fs::remove_dir_all(&ordner);
}

/// ⚑ **P11 am echten Artefakt: Ein Bereich wird geholt und geprüft.** Ein
/// Ordner mit nur dem Manifest holt die Dateien von Ebene 7 aus dem
/// Probeartefakt, Stück für Stück über den Dateidienst, und danach fehlt
/// nichts mehr.
#[test]
fn der_dateidienst_holt_einen_bereich_des_probeartefakts() {
    if !myl_pod::artefakte::vorhanden(&sparrow()) {
        return;
    }
    let ziel = std::env::temp_dir().join(format!("myl-stufe4-dateien-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ziel);
    std::fs::create_dir_all(&ziel).expect("Ordner");
    std::fs::copy(sparrow().join("weights_manifest.json"), ziel.join("weights_manifest.json")).expect("Manifest");
    let fehlt = myl_node::dateidienst::fehlende(&ziel, 7, 8, false, false).expect("fehlende");
    assert!(fehlt.len() >= 7, "eine Ebene des Probemodells hat mindestens sieben Dateien: {}", fehlt.len());
    let manifest = myl_node::dateidienst::manifest_lesen(&sparrow()).expect("Manifest");
    let mut holer = myl_node::dateidienst::Nachholer::neu(ziel.clone(), fehlt);
    while let Some((name, ab, laenge)) = holer.naechste_anfrage() {
        let (daten, groesse) = myl_node::dateidienst::stueck_lesen(&sparrow(), &manifest, &name, ab, laenge).expect("Stück");
        let e = holer.stueck_erhalten(&name, ab, &daten, groesse);
        assert!(!matches!(e, myl_node::dateidienst::Stueckergebnis::Verworfen(_) | myl_node::dateidienst::Stueckergebnis::Fremd), "{name}: {e:?}");
    }
    assert!(myl_node::dateidienst::fehlende(&ziel, 7, 8, false, false).expect("fehlende").is_empty());
    let _ = std::fs::remove_dir_all(&ziel);
}
