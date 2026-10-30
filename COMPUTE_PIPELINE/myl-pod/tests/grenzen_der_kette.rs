//! **Der Pod rechnet nach den Grenzen der Kette** (Stufe 4, 2026-10-22).
//!
//! # ⚑ Die Frage
//!
//! Die Kette schneidet einen Pod nach den Zusagen seiner Mitglieder,
//! ungleich, und schreibt danach gut. Rechnet der Pod mit denselben Grenzen
//! dasselbe wie im gleichen Schnitt? Nur dann ändert der Schnitt nichts am
//! Ergebnis, und nur dann ist die Gutschrift nach ihm ehrlich.

use std::path::PathBuf;
use std::sync::Arc;

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::trainingsschleife::Trainingsvorgaben;
use myl_pod::entsiegelung::Klartextwerk;
use myl_pod::pipelinewerk::{grenzen_pruefen, gleiche_grenzen, Pipelinewerk};
use myl_pod::trainingswerk::Trainingswerk;
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, PodId};
use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};
use myl_types::sitzung::Anfragebindung;

/// Ungleiche Grenzen über die 28 Ebenen des Probemodells, wie sie die
/// Verteilung bei verschieden starken Mitgliedern liefert.
const UNGLEICH: [(usize, usize); 4] = [(0, 3), (3, 10), (10, 23), (23, 28)];

fn artefakte() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest).join("..").join("..").join("INTEGER_LLM").join("artifacts").join("sparrow-0.6b-int8")
}

/// Prüft die Schreibweise der Grenzen, ohne Modell.
#[test]
fn grenzen_werden_geprueft_und_nicht_zurechtgerueckt() {
    assert_eq!(gleiche_grenzen(28), [(0, 7), (7, 14), (14, 21), (21, 28)]);
    assert_eq!(grenzen_pruefen(&UNGLEICH, 28), Ok(UNGLEICH.to_vec()));
    assert!(grenzen_pruefen(&UNGLEICH[..3], 28).is_err(), "drei statt vier");
    assert!(grenzen_pruefen(&[(0, 3), (4, 10), (10, 23), (23, 28)], 28).is_err(), "Luecke");
    assert!(grenzen_pruefen(&[(0, 3), (3, 3), (3, 23), (23, 28)], 28).is_err(), "leer");
    assert!(grenzen_pruefen(&UNGLEICH, 40).is_err(), "anderes Modell");
}

/// ⚑ **Inferenz: ungleiche Grenzen, dieselben Token.**
#[test]
fn die_inferenz_rechnet_mit_den_grenzen_der_kette_dasselbe() {
    let dir = artefakte();
    // 📌 Fund 595 (2026-10-22): Hier stand `assert!(vorhanden(..))`. Mit der
    //    Erlaubnis `MYL_OHNE_ARTEFAKTE` meldet `vorhanden` „ueberspringen“,
    //    und die Probe brach mit „das Probeartefakt fehlt“ ab, auch auf einer
    //    Maschine, die es hat. Ein fehlendes Artefakt ohne Erlaubnis meldet
    //    `vorhanden` selbst.
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let pipeline = Hash([7u8; 32]);
    let laden = |g: Option<&[(usize, usize)]>| {
        Pipelinewerk::laden_mit_grenzen(&dir, PodId::new([0xAA; 32]), EpochId(0), pipeline, 4, g).expect("laedt")
    };
    let prompt = b"Die Hauptstadt von Frankreich ist";
    let auftrag = Inferenzauftrag {
        sitzung: 1,
        bindung: Anfragebindung::neu(1, prompt, EpochId(0)),
        prompt_versiegelt: Vec::new(),
        max_token: 4,
        pipeline,
    };
    let token = |a: Inferenzantwort| match a {
        Inferenzantwort::Ergebnis { token, .. } => token,
        andere => panic!("keine Rechnung: {andere:?}"),
    };
    let gleich = token(laden(None).rechne(&auftrag, prompt));
    let ungleich = token(laden(Some(&UNGLEICH)).rechne(&auftrag, prompt));
    assert!(!gleich.is_empty());
    assert_eq!(gleich, ungleich, "der Schnitt hat das Ergebnis geaendert");
    assert!(
        Pipelinewerk::laden_mit_grenzen(&dir, PodId::new([0xAA; 32]), EpochId(0), pipeline, 4, Some(&[(0, 3), (3, 28)]))
            .is_err(),
        "falsche Grenzen werden abgelehnt"
    );
}

/// ⚑ **Stufe 1: Zwillinge mit verschiedenem Schnitt bezeugen dasselbe
/// Ergebnis** (2026-10-28): dieselbe Ausgabenwurzel. 📌 Auch die
/// Segmentwurzel ist gleich, denn die Spur wird je Ebene geführt, nicht je
/// Übergang; der Vergleich braucht die Ausgabenwurzel trotzdem, weil sie nach
/// Kennung geordnet ist und die Segmentwurzel nach der Folge des Abschlusses.
#[test]
fn zwillinge_mit_verschiedenem_schnitt_haben_dieselbe_ausgabenwurzel() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let pipeline = Hash([7u8; 32]);
    let prompt: &[u8] = b"Die Hauptstadt von Frankreich ist";
    let rechnen = |g: Option<&[(usize, usize)]>, prompt: &[u8]| {
        let werk = Pipelinewerk::laden_mit_grenzen(&dir, PodId::new([0xAA; 32]), EpochId(0), pipeline, 4, g).expect("laedt");
        let auftrag = Inferenzauftrag {
            sitzung: 1,
            bindung: Anfragebindung::neu(1, prompt, EpochId(0)),
            prompt_versiegelt: Vec::new(),
            max_token: 3,
            pipeline,
        };
        assert!(matches!(werk.rechne(&auftrag, prompt), Inferenzantwort::Ergebnis { .. }));
        werk.buendelinhalt().expect("Bündel")
    };
    let a = rechnen(None, prompt);
    let b = rechnen(Some(&UNGLEICH), prompt);
    assert_eq!(a.segmente, b.segmente);
    assert_ne!(a.ausgabenwurzel.as_bytes(), &[0u8; 32], "eine Wurzel, nicht null");
    assert_eq!(a.ausgabenwurzel, b.ausgabenwurzel, "verschiedener Schnitt, dasselbe Ergebnis");
    assert_eq!(a.segments_root, b.segments_root, "die Spur je Ebene hängt nicht am Schnitt");
    // ⛔️ Gegenprobe: ein anderer Auftrag, eine andere Ausgabenwurzel.
    let c = rechnen(Some(&UNGLEICH), b"Zwei plus zwei");
    assert_ne!(a.ausgabenwurzel, c.ausgabenwurzel);
}

/// ⚑ **Training: ungleiche Grenzen, dasselbe Commitment.** Die Spur je
/// Shard ist eine andere (anders geschnitten), das Commitment über den
/// ganzen Pod nicht.
#[test]
fn das_training_rechnet_mit_den_grenzen_der_kette_dasselbe() {
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell"));
    let v = Trainingsvorgaben { schritte: 1, ..Trainingsvorgaben::vorgabe() };
    let gleich = Trainingswerk::aus_modell(Arc::clone(&modell)).expect("Werk").segment_fahren(&v).expect("Segment");
    let mut werk = Trainingswerk::aus_modell_mit_grenzen(modell, Some(&UNGLEICH)).expect("Werk");
    assert_eq!(werk.grenzen(), [0, 3, 10, 23, 28]);
    let ungleich = werk.segment_fahren(&v).expect("Segment");
    assert_eq!(gleich.logits_letzt, ungleich.logits_letzt, "der Schnitt hat das Ergebnis geaendert");
    assert_eq!(gleich.bewegte_gewichte, ungleich.bewegte_gewichte);
}

/// ⚑ **Die Zeitprobe rechnet und bindet ihr Ergebnis**: zweimal dasselbe,
/// ein anderer Bereich etwas anderes, und sie hält keine Sitzung fest.
#[test]
fn die_zeitprobe_rechnet_und_laesst_nichts_zurueck() {
    use myl_pod::shard::ShardNode;
    let dir = artefakte();
    if !myl_pod::artefakte::vorhanden(&dir) {
        return;
    }
    let modell = Arc::new(load_model(&dir).expect("Modell"));
    let sk = || myl_types::bls::BlsSecretKey::key_gen(&[3; 32]).expect("Schluessel");
    let mitte = ShardNode::new(1, 3, 10, false, false, Arc::clone(&modell), sk(), 4);
    let kopf = ShardNode::new(3, 23, 28, false, true, modell, sk(), 4);
    let a = mitte.zeitprobe(2);
    assert_eq!(a, mitte.zeitprobe(2), "dieselbe Probe, derselbe Abdruck");
    assert_ne!(a, kopf.zeitprobe(2), "ein anderer Bereich rechnet anderes");
    // ⚑ Fund 669: Mit Saat hängt der Abdruck an ihr; dieselbe Saat gibt
    //   denselben, eine andere einen anderen, und keiner ist der ohne Saat.
    let (s1, s2) = ([1u8; 32], [2u8; 32]);
    let b1 = mitte.zeitprobe_bis(2, None, Some(&s1));
    assert_eq!(b1, mitte.zeitprobe_bis(2, None, Some(&s1)), "dieselbe Saat, derselbe Abdruck");
    assert_ne!(b1, mitte.zeitprobe_bis(2, None, Some(&s2)), "eine andere Saat, derselbe Abdruck");
    assert_ne!(b1, a, "mit Saat wie ohne");
    assert_eq!(mitte.gehaltene_sitzungen(), 0, "die Probe hielt eine Sitzung fest");

    // ⚑ **Über die Tür des Shards**, wie der Knoten sie stellt
    // (`myl_types::zeitprobe`): dieselbe Antwort.
    let mitte = Arc::new(mitte);
    let ausweis = [0x5A; 32];
    let stelle = myl_pod::shardweg::Shardstelle::oeffnen(Arc::clone(&mitte), "127.0.0.1:0", ausweis).expect("Tuer");
    let adresse = stelle.adresse().expect("Adresse");
    let frist = std::time::Duration::from_secs(60);
    std::thread::scope(|s| {
        // ⚑ **Ohne den Ausweis nimmt die Tür nichts an** (Fund 611): Die
        //   Probe mit einem falschen bekommt keine Antwort, die Tür meldet
        //   den Ausweis, und danach geht die richtige durch.
        let faden = s.spawn(|| stelle.bediene_eine());
        assert!(myl_types::zeitprobe::fragen(adresse, &[0x5B; 32], 2, frist).is_err(), "die Tür rechnete ohne Ausweis");
        assert_eq!(faden.join().expect("Faden"), Err("falscher Ausweis".to_string()));
        let faden = s.spawn(|| stelle.bediene_eine());
        let ueber_die_tuer = myl_types::zeitprobe::fragen(adresse, &ausweis, 2, frist).expect("Probe");
        faden.join().expect("Faden").expect("bedient");
        assert_eq!(ueber_die_tuer, a, "die Tür rechnet dieselbe Probe");
    });
}
