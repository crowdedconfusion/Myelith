//! Was der Trainingsdraht ablehnt, bevor er Speicher belegt.
//!
//! # ⚑ Die Klasse: Verstärkung
//!
//! `MAX_NACHRICHT` begrenzt, was **hereinkommt**. Ein Mitschnitt ist
//! ein Vielfaches davon: Er hält je Position und je Ebene mehrere
//! Zwischenwerte. Bei 896 Kanälen und sechs Ebenen je Shard kostet eine
//! Position rund hundert Kilobyte; vierundsechzig Megabyte Eingabe
//! reichen für etwa siebenunddreissigtausend Positionen, also **vier
//! Gigabyte Mitschnitt aus vierundsechzig Megabyte Nachricht**.
//!
//! ⚑ **Wer nur die Eingabe begrenzt, begrenzt nicht den Speicher.** Die
//! Grenze gehört dorthin, wo der Speicher entsteht.
//!
//! # ⚑ Und die zweite: Anhäufung
//!
//! Ein Mitschnitt bleibt liegen, bis der Rückwärtspass kommt. Wer nur
//! Vorwärtspässe schickt und nie zurück, häuft Mitschnitte an, ohne je
//! etwas zu rechnen. **Beides sind Befunde am Code dieses Tages**, und
//! sie stehen hier, weil eine Grenze ohne Test eine Absichtserklärung
//! ist.

use std::sync::Arc;

use myl_pod::shardweg::{
    bedienen_training, Shardanfrage, Shardantwort, Shardtrainer, MAX_TRAINPOSITIONEN,
    MAX_TRAINSITZUNGEN,
};

fn artefakte() -> std::path::PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    std::path::PathBuf::from(manifest)
        .join("..")
        .join("..")
        .join("INTEGER_LLM")
        .join("artifacts")
        .join("qwen2.5-0.5b")
}

fn trainer() -> Option<Shardtrainer> {
    if !artefakte().exists() {
        return None;
    }
    let m = Arc::new(integer_llm_runtime::loader::load_model(&artefakte()).expect("Modell"));
    // Ein schmaler Bereich genuegt: geprueft wird die Grenze, nicht das
    // Rechnen.
    Some(Shardtrainer::neu(m, 0, 1).expect("Trainer"))
}

/// ⚑ **Zu viele Positionen werden abgelehnt, nicht belegt.**
#[test]
fn zu_viele_positionen_werden_abgelehnt() {
    let Some(mut t) = trainer() else {
        eprintln!("Artefakt fehlt, uebersprungen");
        return;
    };
    // Eine Position mehr als erlaubt, und absichtlich schmal: Die
    // Ablehnung darf **nicht** an der Breite haengen, sondern an der
    // Zahl der Positionen.
    let hidden = vec![vec![0i16; 4]; MAX_TRAINPOSITIONEN + 1];
    let antwort = bedienen_training(
        &mut t,
        &Shardanfrage::TrainVorwaerts { sitzung: 1, hidden, schritt: 0, lr_nenner: 256 },
    );
    match antwort {
        Some(Shardantwort::Fehler(e)) => {
            assert!(e.contains("Positionen"), "die Begruendung nennt die Positionen nicht: {e}");
        }
        andere => panic!("erwartet wurde eine Ablehnung, kam: {andere:?}"),
    }
}

/// ⚑ **Die Gegenprobe:** Genau an der Grenze wird nicht abgelehnt.
///
/// Ohne sie waere der Test oben auch dann gruen, wenn der Shard **jede**
/// Anfrage abwiese.
#[test]
fn an_der_grenze_wird_nicht_wegen_der_laenge_abgelehnt() {
    let Some(mut t) = trainer() else {
        eprintln!("Artefakt fehlt, uebersprungen");
        return;
    };
    // Eine einzelne Position, also weit unter der Grenze. Sie darf an
    // der Laengenpruefung nicht scheitern; ob sie danach rechnet, ist
    // hier nicht die Frage.
    let m = integer_llm_runtime::loader::load_model(&artefakte()).expect("Modell");
    let hidden = vec![vec![0i16; m.hidden_size]; 1];
    let antwort = bedienen_training(
        &mut t,
        &Shardanfrage::TrainVorwaerts { sitzung: 1, hidden, schritt: 0, lr_nenner: 256 },
    );
    if let Some(Shardantwort::Fehler(e)) = &antwort {
        assert!(
            !e.contains("Positionen") && !e.contains("Trainingssitzungen"),
            "eine gueltige Laenge wurde wegen einer Grenze abgelehnt: {e}"
        );
    }
}

/// ⚑ **Offene Sitzungen häufen sich nicht unbegrenzt an.**
#[test]
fn zu_viele_offene_sitzungen_werden_abgelehnt() {
    let Some(mut t) = trainer() else {
        eprintln!("Artefakt fehlt, uebersprungen");
        return;
    };
    let m = integer_llm_runtime::loader::load_model(&artefakte()).expect("Modell");
    let hidden = vec![vec![0i16; m.hidden_size]; 2];
    // So viele Sitzungen oeffnen, wie erlaubt sind, und nie
    // zurueckrechnen.
    for s in 0..MAX_TRAINSITZUNGEN as u64 {
        let a = bedienen_training(
            &mut t,
            &Shardanfrage::TrainVorwaerts {
                sitzung: s,
                hidden: hidden.clone(),
                schritt: 0,
                lr_nenner: 256,
            },
        );
        assert!(
            matches!(a, Some(Shardantwort::TrainAusgang(_))),
            "Sitzung {s} haette angenommen werden muessen, kam: {a:?}"
        );
    }
    // Die naechste faellt durch.
    let zu_viel = bedienen_training(
        &mut t,
        &Shardanfrage::TrainVorwaerts {
            sitzung: 999,
            hidden,
            schritt: 0,
            lr_nenner: 256,
        },
    );
    match zu_viel {
        Some(Shardantwort::Fehler(e)) => {
            assert!(e.contains("Trainingssitzungen"), "falsche Begruendung: {e}");
        }
        andere => panic!("erwartet wurde eine Ablehnung, kam: {andere:?}"),
    }
}
