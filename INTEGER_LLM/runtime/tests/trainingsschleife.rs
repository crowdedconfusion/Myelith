//! Der Kreis vom echten Ziel bis in die Gewichte (TRAINING V, dritter
//! Teil).
//!
//! Die Schleife selbst steht in
//! [`integer_llm_runtime::trainingsschleife`]; hier steht, was sie
//! belegen soll, und die Gegenproben dazu.
//!
//! # ⚑ Der Verlustwert steht hier und nicht in der Bibliothek
//!
//! Die Kreuzentropie braucht einen Logarithmus, **ihre Ableitung
//! nicht**. Ein Logarithmus im Rechenpfad wäre ein Bruch der Kernthese;
//! hier steht er in einem Test und bewegt kein Gewicht. Die Bibliothek
//! liegt im Ganzzahl-Audit, dieser Test nicht, und **die Trennung ist
//! die Aussage**: Was zwei Maschinen vergleichen, ist der Abdruck über
//! die Gewichte, nie eine Gleitkommazahl.

use integer_llm_runtime::loader::load_model;
use integer_llm_runtime::model::IntegerModel;
use integer_llm_runtime::trainingsschleife::{
    trainingsschleife, Trainingsergebnis, Trainingsvorgaben,
};

/// Ein Kontrollwort, auf das **nicht** trainiert wird.
///
/// ⚑ Es ist der Anfangs-Argmax des Modells auf dieser Folge. Sein
/// Verlust muss steigen, während der des Ziels fällt.
const KONTROLLE: usize = 5726;

fn artefakte() -> std::path::PathBuf {
    let modell = std::env::var("MYL_POD_MODELL").unwrap_or_else(|_| "qwen2.5-0.5b".to_string());
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../artifacts")
        .join(modell)
}

fn modell() -> Option<IntegerModel> {
    // ⛑ **Fund 218: Diese Abfrage stand unter der Pfadpruefung**, und
    // damit war der Schalter auf jeder Maschine wirkungslos, die die
    // Artefakte **hat**. Gemeint war er fuer zwei Leser: die CI, wo
    // nichts liegt, und den Entwickler, der waehrend einer Messung
    // keine Rechenzeit an eine Pruefsammlung abgeben will. Nur der
    // erste wurde bedient. Aufgefallen, als `MYL_OHNE_ARTEFAKTE=1
    // cargo test` neben einem laufenden Training doch das 4B-Modell
    // lud und 59 Sekunden rechnete. Der Schalter heisst „ohne
    // Artefakte" und bedeutet jetzt genau das, unabhaengig davon, ob
    // welche da sind.
    if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
        eprintln!("SKIP (MYL_OHNE_ARTEFAKTE gesetzt)");
        return None;
    }
    let dir = artefakte();
    if !dir.exists() {
        panic!("Artefakte fehlen: {dir:?}");
    }
    Some(load_model(&dir).expect("Modell laedt"))
}

/// Die Kreuzentropie **aus den Ganzzahllogits**, in Gleitkomma.
///
/// ⚑ **Bewusst aus den Logits und nicht aus einem gerundeten `p`.** Ein
/// Verlust aus `p` hat einen Boden: Sobald `p[ziel]` auf null fällt,
/// meldet er `frac · ln 2` und rührt sich nicht mehr, egal was das
/// Modell tut. Genau darauf lief der erste Entwurf dieses Tests am
/// 2026-09-04 auf: **9,7041 = 14 · ln 2**, dreissig Schritte lang
/// unverändert (Fund 177). Der Logarithmus der Summe kennt diesen Boden
/// nicht.
fn verlust(logits: &[i32], ziel: usize, logit_frac: u8) -> f64 {
    let skala = f64::from(1u32 << logit_frac);
    let m = f64::from(*logits.iter().max().unwrap());
    let summe: f64 = logits.iter().map(|z| ((f64::from(*z) - m) / skala).exp()).sum();
    summe.ln() + m / skala - f64::from(logits[ziel]) / skala
}

/// Verlust vor und nach dem Lauf, für ein Wort.
fn spanne(e: &Trainingsergebnis, wort: usize, v: &Trainingsvorgaben) -> (f64, f64) {
    (
        verlust(&e.logits_erst, wort, v.logit_frac),
        verlust(&e.logits_letzt, wort, v.logit_frac),
    )
}

/// ⚑ **Ein echtes Ziel bewegt echte Gewichte, und der Verlust sinkt.**
///
/// ⚑ **Was der Lauf nicht zeigt:** dass ein Modell lernt. Eine Folge,
/// ein Zielwort; ein Verlust, der dabei fällt, ist Auswendiglernen. Der
/// Beleg braucht Korpus und Haltemenge, und genau daran ist am
/// 2026-08-22 schon einmal eine Messung gescheitert, die den
/// Trainingsverlust dafür hielt.
#[test]
fn der_verlust_sinkt_ueber_die_schleife() {
    let Some(m) = modell() else { return };
    let v = Trainingsvorgaben::vorgabe();
    let e = trainingsschleife(&m, &v).expect("die Schleife laeuft");

    let (erster, letzter) = spanne(&e, v.ziel, &v);
    let (k_erster, k_letzter) = spanne(&e, KONTROLLE, &v);
    let (argmax_vorher, argmax_nachher) = e.argmax();

    eprintln!(
        "\n  Ebene {} ({} von {} Gewichten bewegt)\
         \n  Ziel        {}: Kreuzentropie {erster:.4} -> {letzter:.4}\
         \n  Kontrolle {KONTROLLE}: Kreuzentropie {k_erster:.4} -> {k_letzter:.4}\
         \n  Argmax {argmax_vorher} -> {argmax_nachher} ueber {} Schritte\
         \n  Trainingsabdruck: {}\n",
        e.ebene, e.bewegte_gewichte, e.gewichte_gesamt, v.ziel, v.schritte, e.abdruck
    );

    // ⚑ **Der Verlust faellt, und zwar deutlich.** Die Schranke liegt
    // bei 1,0 gegen einen Startwert von 9,22 und einen gemessenen
    // Endwert von 0,137; sie prueft die Aussage, nicht die Nachkommastelle.
    assert!(letzter < 1.0, "der Verlust blieb ueber 1,0: {erster:.4} auf {letzter:.4}");

    // ⚑ **Das Ziel ist am Ende die Vorhersage.** Das ist die eigentliche
    // Behauptung: nicht dass irgendeine Zahl faellt, sondern dass das
    // Modell das verlangte Wort nennt.
    assert_eq!(argmax_nachher, v.ziel, "das Ziel ist nicht der Argmax");

    // ⚑ **Die Gegenprobe, und sie muss beissen.** Ein Schritt, der bloss
    // alle Logits anhebt, liesse auch den Verlust des Kontrollworts
    // fallen. Der Kreuzentropiegradient ist `p - onehot`: Er hebt genau
    // ein Wort und drueckt alle uebrigen. Steigt der Kontrollverlust
    // nicht, wirkt der Schritt nicht zielgerichtet, sondern global, und
    // dann belegt der fallende Zielverlust nichts.
    //
    // Gemessen mit dem Zielterm heraus: der Zielverlust steigt auf
    // 12,8000 und der Argmax landet bei 468.
    assert!(
        k_letzter > k_erster,
        "der Verlust des Kontrollworts fiel mit: {k_erster:.4} auf {k_letzter:.4}"
    );

    assert_eq!(e.abdruck.len(), 64, "der Abdruck ist kein voller SHA-256");
}

/// ⚑ **Zweimal derselbe Lauf, zweimal derselbe Abdruck.**
///
/// # ⚑ Was das belegt und was nicht
///
/// **Belegt:** Der Lauf hängt an nichts ausser seinen Eingaben. Kein
/// Zeitstempel, keine Speicheradresse, keine Reihenfolge einer
/// Hashtabelle, kein uninitialisierter Speicher. Das ist die Klasse von
/// Fehlern, die auf **einer** Maschine sichtbar ist.
///
/// **Nicht belegt:** Bitgleichheit über Architekturen hinweg. Dafür
/// braucht es zwei Maschinen, und genau dafür meldet der Testclient den
/// Abdruck. ⚑ **Ein Determinismusbeleg aus einem einzigen Prozess ist
/// keiner**, und wer ihn dafür ausgibt, verwechselt Wiederholbarkeit
/// mit Übereinstimmung.
///
/// Fünf Schritte statt dreissig: Jede Rechenvorschrift des Laufs kommt
/// darin vor, und der Test kostet ein Sechstel.
#[test]
fn zwei_laeufe_liefern_denselben_abdruck() {
    let Some(m) = modell() else { return };
    let v = Trainingsvorgaben { schritte: 5, ..Trainingsvorgaben::vorgabe() };
    let a = trainingsschleife(&m, &v).expect("Lauf a");
    let b = trainingsschleife(&m, &v).expect("Lauf b");
    assert_eq!(a.abdruck, b.abdruck, "derselbe Lauf, zwei Abdruecke");
    assert_eq!(a.logits_letzt, b.logits_letzt, "derselbe Lauf, zwei Logitreihen");
}

/// ⚑ **Ein anderer Lauf ist ein anderer Abdruck.**
///
/// Die Gegenprobe zum vorigen Test: Ein Abdruck, der zwei verschiedene
/// Läufe gleich meldete, belegte nichts.
#[test]
fn ein_anderer_lauf_ergibt_einen_anderen_abdruck() {
    let Some(m) = modell() else { return };
    let fuenf = Trainingsvorgaben { schritte: 5, ..Trainingsvorgaben::vorgabe() };
    let sechs = Trainingsvorgaben { schritte: 6, ..Trainingsvorgaben::vorgabe() };
    let anderes_ziel = Trainingsvorgaben { schritte: 5, ziel: 468, ..Trainingsvorgaben::vorgabe() };
    let a = trainingsschleife(&m, &fuenf).expect("Lauf a");
    assert_ne!(
        a.abdruck,
        trainingsschleife(&m, &sechs).expect("Lauf b").abdruck,
        "ein Schritt mehr aenderte nichts"
    );
    assert_ne!(
        a.abdruck,
        trainingsschleife(&m, &anderes_ziel).expect("Lauf c").abdruck,
        "ein anderes Ziel aenderte nichts"
    );
}

/// ⚑ **Ein Ziel ausserhalb des Vokabulars bricht ab, statt zu rechnen.**
#[test]
fn ein_ziel_ausserhalb_des_vokabulars_wird_abgelehnt() {
    let Some(m) = modell() else { return };
    let v = Trainingsvorgaben { schritte: 1, ziel: 999_999_999, ..Trainingsvorgaben::vorgabe() };
    let fehler = trainingsschleife(&m, &v).expect_err("haette abbrechen muessen");
    assert!(fehler.contains("ausserhalb des Vokabulars"), "{fehler}");
}

/// ⚑ **Die Gegenprobe zur Schranke: Beisst der Abbruch?**
///
/// Eine absurd grosse Lernrate (`lr_nenner = 1`, also Schrittweite eins
/// je Gradienteneinheit) treibt die Gewichte binnen weniger Schritte aus
/// der Übertragungsform. Der Lauf muss das **melden** und aufhören,
/// nicht weiterrechnen und später beim Zurückschreiben abstürzen.
///
/// ⚑ **Ohne diesen Test wäre die Schranke eine Behauptung.** Sie stand
/// bis zum 2026-09-05 nur als Panik in `gewicht_aus_master`, also am
/// fernen Ende, und der 30B-Lauf fand ihren Fall nur, weil er von Hand
/// danach sah.
#[test]
fn eine_absurde_lernrate_verlaesst_die_form_und_der_lauf_meldet_es() {
    let Some(m) = modell() else { return };
    let v = Trainingsvorgaben { schritte: 40, lr_nenner: 1, ..Trainingsvorgaben::vorgabe() };
    let e = trainingsschleife(&m, &v).expect("Lauf");
    let schritt = e.aus_der_form.expect(
        "bei lr_nenner = 1 muss der Lauf die Uebertragungsform verlassen; \
         tut er es nicht, prueft die Schranke nichts",
    );
    assert!(schritt < v.schritte, "der Abbruch kam nach dem letzten Schritt");
    eprintln!("  aus der Form bei Schritt {schritt} von {}", v.schritte);
}

/// Und der Normalfall meldet nichts.
///
/// ⚑ **Die andere Richtung gehört dazu.** Eine Schranke, die immer
/// anschlägt, hielte den ersten Test auch, wäre aber nutzlos.
#[test]
fn der_gewoehnliche_lauf_bleibt_in_der_form() {
    let Some(m) = modell() else { return };
    let v = Trainingsvorgaben { schritte: 10, ..Trainingsvorgaben::vorgabe() };
    let e = trainingsschleife(&m, &v).expect("Lauf");
    assert_eq!(e.aus_der_form, None, "die Vorgabelernrate treibt nichts aus der Form");
}
