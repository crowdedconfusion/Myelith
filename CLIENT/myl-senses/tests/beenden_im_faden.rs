//! ⚑ **Die Pause des Loops beendet seine Befehle und nur seine.**
//!
//! Ein eigenes Testziel wie `alle_beenden`: Gezaehlt wird jeder Lauf im
//! Prozess, und Proben laufen nebeneinander in Faeden desselben Prozesses.

use std::process::Command;
use std::time::{Duration, Instant};

/// Ein Befehl, der dreissig Sekunden schlaeft, auf jedem System (siehe
/// `alle_beenden.rs`).
fn schlaefer() -> Command {
    if cfg!(windows) {
        let mut b = Command::new("powershell");
        b.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
        b
    } else {
        let mut b = Command::new("/bin/sh");
        b.args(["-c", "exec sleep 30"]);
        b
    }
}

#[test]
fn beenden_im_faden_trifft_nur_diesen_faden() {
    let starten = || {
        std::thread::spawn(|| {
            let anfang = Instant::now();
            let a = myl_senses::prozess::laufen(&mut schlaefer(), 60, 1024).expect("startet");
            (a, anfang.elapsed())
        })
    };
    let loop_faden = starten();
    let anderer = starten();

    let bis = Instant::now() + Duration::from_secs(10);
    while myl_senses::prozess::laufende() < 2 {
        assert!(Instant::now() < bis, "die Laeufe wurden nie eingetragen");
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(myl_senses::prozess::beenden_im_faden(loop_faden.thread().id()), 1);
    let (a, dauer) = loop_faden.join().expect("Faden");
    assert!(dauer < Duration::from_secs(15), "der Befehl des Loops lief weiter: {dauer:?}");
    assert!(a.abgebrochen, "ein erschlagener Lauf meldet sich nicht als abgebrochen: {a:?}");
    assert_eq!(myl_senses::prozess::laufende(), 1, "der andere Lauf wurde mit beendet");

    // Aufraeumen: Der andere laeuft noch und darf die Probe nicht aufhalten.
    assert_eq!(myl_senses::prozess::alle_beenden(), 1);
    let (b, _) = anderer.join().expect("Faden");
    assert!(b.abgebrochen);
}
