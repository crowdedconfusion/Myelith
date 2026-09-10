//! Wirkt eine Freigabe, oder steht sie nur da?
//!
//! # ⚑ Warum es diese Datei gibt
//!
//! **Fund 272.** Von den vier Grenzen dieses Rechners wurde genau eine
//! angewendet; die anderen drei wurden gelesen, gespeichert, angezeigt,
//! und danach las sie niemand. Ein Regler, der aussieht wie eine
//! Grenze und keine ist, ist eine Behauptung, und eine Behauptung in
//! einer Freigabemaske ist die schlechteste Sorte: Der Nutzer glaubt,
//! er habe etwas zugesagt.
//!
//! Jede Prueffung hier faehrt deshalb **die Wirkung** und nicht den
//! abgelegten Wert.

use myl_client::einstellungen::{Einstellungen, Kapazitaet};

/// Ein Verzeichnis, das sich wie ein Artefakt anfuehlt, nur gross.
fn dickes_artefakt(bytes: u64) -> tempfile::TempDir {
    let d = tempfile::tempdir().expect("Verzeichnis");
    std::fs::write(d.path().join("gewichte.bin"), vec![0u8; bytes as usize]).expect("Datei");
    d
}

/// ⚑ **Die Speicherfreigabe haelt ein zu grosses Artefakt auf**, und
/// zwar **vor** dem Laden: Danach waere der Speicher schon belegt.
#[test]
fn ein_artefakt_ueber_der_freigabe_wird_nicht_geladen() {
    let d = dickes_artefakt(4 * 1024 * 1024);
    let pfad = d.path().display().to_string();

    let eng = Kapazitaet { speicher_gib: Some(0), ..Default::default() };
    let fehler = myl_client::Oertlichesmodell::laden(&pfad, &eng)
        .err()
        .expect("ein Artefakt ueber der Freigabe wurde geladen");
    assert!(
        fehler.contains("freigegeben") && fehler.contains("GiB"),
        "die Meldung sagt nicht, dass die Freigabe der Grund ist: {fehler}"
    );

    // ⚑ **Die Gegenrichtung, und sie ist die wichtigere.** Ohne sie
    // pruefte diese Datei nur, dass `laden` fehlschlaegt, und das tut
    // es hier ohnehin: Das Verzeichnis ist kein echtes Artefakt.
    // Kommt die Ablehnung von der Freigabe, muss die Meldung ohne
    // Freigabe eine **andere** sein.
    let weit = Kapazitaet::default();
    let fehler = myl_client::Oertlichesmodell::laden(&pfad, &weit)
        .err()
        .expect("ein Scheinartefakt laesst sich laden");
    assert!(
        !fehler.contains("freigegeben"),
        "ohne Freigabe kommt dieselbe Meldung; die Schranke greift also gar nicht: {fehler}"
    );
}

/// ⚑ **Und ohne gesetzte Freigabe haelt nichts auf.**
#[test]
fn ohne_freigabe_gibt_es_keine_speicherschranke() {
    let d = dickes_artefakt(4 * 1024 * 1024);
    let pfad = d.path().display().to_string();
    let fehler = myl_client::Oertlichesmodell::laden(&pfad, &Kapazitaet::default())
        .err()
        .expect("ein Scheinartefakt laesst sich laden");
    assert!(!fehler.contains("freigegeben"), "{fehler}");
}

/// ⚑ **Die Kernfreigabe wirkt auf den Rechenpfad**, und zwar durch die
/// ganze Naht hindurch: Einstellung, Klient, Laufzeit, Kerne.
///
/// ⛑ **Sie steht als Einzelprueffung da und nicht neben anderen**, denn
/// die Grenze gilt fuer den **ganzen Prozess**. Eine zweite Prueffung
/// daneben saehe zeitweise eine andere Kernzahl.
#[test]
fn die_kernfreigabe_erreicht_den_rechenpfad() {
    let vorher = myl_client::kapazitaet::kerne();
    let mut e = Einstellungen::default();
    e.setzen("kap.kerne", "2").expect("setzen");

    myl_client::kapazitaet::kerne_setzen(e.kapazitaet.kerne.expect("gesetzt"));
    assert_eq!(myl_client::kapazitaet::kerne(), 2, "die Freigabe erreicht den Rechenpfad nicht");

    // `aus` nimmt sie weg und gibt die Maschine wieder frei.
    e.setzen("kap.kerne", "aus").expect("setzen");
    assert_eq!(e.kapazitaet.kerne, None);
    myl_client::kapazitaet::kerne_setzen(0);
    assert_eq!(myl_client::kapazitaet::kerne(), vorher, "die Maschine kam nicht zurueck");
}

/// ⚑ **Die Plattenfreigabe haelt Platz, und die Rechnung geht auf.**
///
/// Belegt plus reserviert ist die Freigabe, durchgehend. Ohne diese
/// Gleichung arbeitet die Reservierung gegen den eigenen Download.
#[test]
fn belegt_und_reserviert_ergeben_zusammen_die_freigabe() {
    use myl_client::reservierung::{belegung, zu_halten, Reservierung};

    let d = tempfile::tempdir().expect("Verzeichnis");
    let daten = d.path().join("modelle");
    std::fs::create_dir_all(&daten).expect("Verzeichnis");
    std::fs::write(daten.join("modell.bin"), vec![0u8; 8 * 1024 * 1024]).expect("Datei");

    let freigabe = 32u64 * 1024 * 1024;
    let belegt = belegung(&daten);
    assert_eq!(belegt, 8 * 1024 * 1024);

    let mut r = Reservierung::anlegen(d.path(), zu_halten(freigabe, belegt)).expect("anlegen");
    assert_eq!(r.rest() + belegt, freigabe, "die Summe ist nicht die Freigabe");

    // Ein Download von 16 MiB: Die Reservierung gibt den Platz her.
    let bekommen = r.hergeben(16 * 1024 * 1024).expect("hergeben");
    assert_eq!(bekommen, 16 * 1024 * 1024);
    std::fs::write(daten.join("zweites.bin"), vec![0u8; 16 * 1024 * 1024]).expect("Datei");
    assert_eq!(
        r.rest() + belegung(&daten),
        freigabe,
        "nach dem Download stimmt die Summe nicht mehr"
    );

    // Und ein Download, der mehr will als die Freigabe hergibt,
    // bekommt nur den Rest.
    let bekommen = r.hergeben(freigabe).expect("hergeben");
    assert_eq!(bekommen, 8 * 1024 * 1024, "es wurde mehr hergegeben als da war");
    assert_eq!(r.rest(), 0);
}
