//! ⛔️ **Wer das Programm beendet, beendet seine Befehle mit.**
//!
//! Ein eigenes Testziel und keine Probe neben den anderen: `alle_beenden`
//! trifft jeden Lauf im Prozess, und Proben laufen nebeneinander in
//! Faeden desselben Prozesses. Hier laeuft nur diese eine.

use std::process::Command;
use std::time::{Duration, Instant};

/// Ein Befehl, der dreissig Sekunden schlaeft, auf jedem System.
///
/// ⚑ `exec`, damit die Shell sich ersetzt: Beendet wird das Kind und
/// nicht seine Enkel, und ohne `exec` waere das unter Linux eine
/// Shell mit einem `sleep` darunter.
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
fn alle_beenden_trifft_den_laufenden_befehl() {
    assert_eq!(myl_senses::prozess::alle_beenden(), 0, "ohne Lauf gibt es nichts zu beenden");

    let faden = std::thread::spawn(|| {
        let anfang = Instant::now();
        let a = myl_senses::prozess::laufen(&mut schlaefer(), 60, 1024).expect("startet");
        (a, anfang.elapsed())
    });

    // ⚑ Auf die Wirkung warten, nicht auf die Uhr: bis der Lauf
    //   eingetragen ist, hoechstens zehn Sekunden.
    let bis = Instant::now() + Duration::from_secs(10);
    while myl_senses::prozess::laufende() == 0 {
        assert!(Instant::now() < bis, "der Lauf wurde nie eingetragen");
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(myl_senses::prozess::alle_beenden(), 1);
    let (a, dauer) = faden.join().expect("Faden");

    // Die Frist war sechzig Sekunden, der Befehl haette dreissig gebraucht.
    assert!(dauer < Duration::from_secs(15), "der Befehl lief weiter: {dauer:?}");
    assert!(a.abgebrochen, "ein erschlagener Lauf meldet sich nicht als abgebrochen: {a:?}");
    assert_eq!(myl_senses::prozess::laufende(), 0, "der Lauf hat sich nicht ausgetragen");
}
