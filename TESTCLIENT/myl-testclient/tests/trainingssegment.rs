//! Die Arbeitsklasse und ihr Erzeuger, an einer Stelle (TRAINING 2.1).
//!
//! # ⚑ Warum dieser Test hier steht und nirgends sonst
//!
//! `myl-types` kennt die Laufzeit nicht, und die Laufzeit kennt
//! `myl-types` nicht. Das ist Absicht: Die Ganzzahl-Laufzeit soll ohne
//! Konsenstypen bauen, und die Konsenstypen sollen ohne Modell bauen.
//! **`myl-testclient` ist die einzige Stelle, die beide sieht.**
//!
//! # ⚑ Was er belegt
//!
//! Dass die zweite Arbeitsklasse **einen Erzeuger hat**. Ein
//! Trainingssegment trägt ein Commitment über Δm; bis zum 2026-09-04 gab
//! es nichts, das ein Δm erzeugt, und der Typ wäre eine Kiste ohne
//! Aufrufer gewesen. Das ist die Fehlerklasse, die dieses Projekt
//! viermal getroffen hat (Funde 173 bis 175, dazu Fund 145).
//!
//! Hier läuft die Schleife, liefert ihr Δm-Commitment, und daraus
//! entsteht ein Segment, das seine eigene Prüfung besteht.

use myl_types::bls::BlsSignature;
use myl_types::hash::Hash;
use myl_types::ids::{MerkleRoot, MinerId, SegmentId};
use myl_types::trainingssegment::{Segmentfehler, Trainingssegment};

use integer_llm_runtime::trainingsschleife::{trainingsschleife, Trainingsvorgaben};

fn artefakte() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../INTEGER_LLM/artifacts/qwen2.5-0.5b")
}

/// ⚑ **Ein gerechneter Trainingsschritt wird zu einem gültigen
/// Segment.**
#[test]
fn ein_gerechneter_schritt_wird_zu_einem_segment() {
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
        return;
    }
    let dir = artefakte();
    if !dir.exists() {
        panic!("Artefakte fehlen: {dir:?}");
    }
    let m = integer_llm_runtime::loader::load_model(&dir).expect("Modell laedt");

    // ⚑ **Wenige Schritte.** Dieser Test prueft die Naht, nicht das
    // Lernen; das tut `runtime/tests/trainingsschleife.rs`.
    let v = Trainingsvorgaben { schritte: 3, ..Trainingsvorgaben::vorgabe() };
    let e = trainingsschleife(&m, &v).expect("die Schleife laeuft");

    assert_eq!(e.delta_commitment.len(), 64, "kein voller SHA-256");
    assert_ne!(
        e.delta_commitment, e.abdruck,
        "Delta und Endzustand haben denselben Wert: dann ist einer von beiden falsch gebildet"
    );

    let segment = Trainingssegment {
        id: SegmentId::new([1u8; 32]),
        modell_version: MerkleRoot::new([2u8; 32]),
        charge: Hash::from_bytes([3u8; 32]),
        startschritt: 0,
        schrittzahl: v.schritte as u32,
        folgen: 1,
        lr_zaehler: 1,
        lr_nenner: v.lr_nenner,
        delta_commitment: Hash::from_hex(&e.delta_commitment).expect("Hex"),
        bewegte_gewichte: 1,
        pod_pfad: vec![MinerId::new([4u8; 32])],
        signaturen: vec![BlsSignature([0u8; 96])],
    };
    segment.pruefen().expect("das Segment ist gesund");

    eprintln!(
        "\n  Ebene {}: {} von {} Gewichten bewegt\n  \
         Abdruck (Endzustand): {}\n  \
         Commitment (Delta):   {}\n  \
         Segmentbotschaft:     {}\n",
        e.ebene,
        e.bewegte_gewichte,
        e.gewichte_gesamt,
        &e.abdruck[..32],
        &e.delta_commitment[..32],
        hex(&segment.botschaft())
    );
}

/// ⚑ **Dieselbe Arbeit, dasselbe Δm, dieselbe Botschaft.**
///
/// Das ist die Eigenschaft, an der die ganze Verifikation hängt: Ein
/// Segment ist eine reine Funktion seiner Eingabe, und zwei Miner mit
/// denselben Angaben unterschreiben dieselbe Botschaft.
#[test]
fn zwei_laeufe_ergeben_dieselbe_segmentbotschaft() {
    let dir = artefakte();
    if !dir.exists() {
        return;
    }
    let m = integer_llm_runtime::loader::load_model(&dir).expect("Modell laedt");
    let v = Trainingsvorgaben { schritte: 3, ..Trainingsvorgaben::vorgabe() };

    let bauen = |commitment: &str| Trainingssegment {
        id: SegmentId::new([1u8; 32]),
        modell_version: MerkleRoot::new([2u8; 32]),
        charge: Hash::from_bytes([3u8; 32]),
        startschritt: 0,
        schrittzahl: v.schritte as u32,
        folgen: 1,
        lr_zaehler: 1,
        lr_nenner: v.lr_nenner,
        delta_commitment: Hash::from_hex(commitment).expect("Hex"),
        bewegte_gewichte: 1,
        pod_pfad: vec![MinerId::new([4u8; 32])],
        signaturen: vec![BlsSignature([0u8; 96])],
    };

    let a = trainingsschleife(&m, &v).expect("Lauf a");
    let b = trainingsschleife(&m, &v).expect("Lauf b");
    assert_eq!(a.delta_commitment, b.delta_commitment, "zwei Laeufe, zwei Deltas");
    assert_eq!(bauen(&a.delta_commitment).botschaft(), bauen(&b.delta_commitment).botschaft());
}

/// ⚑ **Eine andere Arbeit ist eine andere Botschaft.**
///
/// Die Gegenprobe: Ohne sie bliebe offen, ob die Botschaft überhaupt vom
/// Ergebnis abhängt.
#[test]
fn eine_andere_schrittzahl_ist_eine_andere_botschaft() {
    let dir = artefakte();
    if !dir.exists() {
        return;
    }
    let m = integer_llm_runtime::loader::load_model(&dir).expect("Modell laedt");
    let bauen = |schritte: u64, commitment: &str| Trainingssegment {
        id: SegmentId::new([1u8; 32]),
        modell_version: MerkleRoot::new([2u8; 32]),
        charge: Hash::from_bytes([3u8; 32]),
        startschritt: 0,
        schrittzahl: schritte as u32,
        folgen: 1,
        lr_zaehler: 1,
        lr_nenner: 1 << 12,
        delta_commitment: Hash::from_hex(commitment).expect("Hex"),
        bewegte_gewichte: 1,
        pod_pfad: vec![MinerId::new([4u8; 32])],
        signaturen: vec![BlsSignature([0u8; 96])],
    };
    let drei = trainingsschleife(
        &m,
        &Trainingsvorgaben { schritte: 3, ..Trainingsvorgaben::vorgabe() },
    )
    .expect("drei");
    let vier = trainingsschleife(
        &m,
        &Trainingsvorgaben { schritte: 4, ..Trainingsvorgaben::vorgabe() },
    )
    .expect("vier");
    assert_ne!(drei.delta_commitment, vier.delta_commitment, "drei und vier Schritte gleich?");
    assert_ne!(
        bauen(3, &drei.delta_commitment).botschaft(),
        bauen(4, &vier.delta_commitment).botschaft()
    );
}

/// ⚑ **Und was die Schleife liefert, kann die Prüfung ablehnen.**
///
/// Damit der Test oben nicht nur eine Prüfung durchläuft, die alles
/// bestehen lässt.
#[test]
fn eine_lernrate_ohne_nenner_wird_abgelehnt() {
    let s = Trainingssegment {
        id: SegmentId::new([1u8; 32]),
        modell_version: MerkleRoot::new([2u8; 32]),
        charge: Hash::from_bytes([3u8; 32]),
        startschritt: 0,
        schrittzahl: 3,
        folgen: 1,
        lr_zaehler: 1,
        lr_nenner: 0,
        delta_commitment: Hash::from_bytes([4u8; 32]),
        bewegte_gewichte: 1,
        pod_pfad: vec![MinerId::new([4u8; 32])],
        signaturen: vec![BlsSignature([0u8; 96])],
    };
    assert_eq!(s.pruefen(), Err(Segmentfehler::LernrateOhneNenner));
}

fn hex(b: &[u8; 32]) -> String {
    b.iter().take(16).map(|x| format!("{x:02x}")).collect()
}
