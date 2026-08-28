//! Gegenprobe zum Testprofil: optimiert ja, nachlässig nein.
//!
//! ⚑ **Diese Datei bewacht eine Zeile in `Cargo.toml`.** Das Testprofil
//! baut mit `opt-level = 2`, weil die rechengebundenen Tests im
//! unoptimierten Bau etwa zwanzigmal länger laufen als nötig. Der
//! naheliegende Weg dorthin wäre `--release` gewesen, und der wäre
//! falsch: Er schaltet `debug-assertions` ab und damit die
//! Überlaufprüfung.
//!
//! **Was daran hängt.** Die dokumentierten Vorbedingungen des
//! Ganzzahlpfades stehen als `debug_assert!` (Fund 75), und ein
//! stillschweigend umlaufender `i32` in einem Konsenspfad ist genau die
//! Sorte Fehler, die kein Test sieht und jeder Knoten anders rechnet.
//!
//! **Fällt einer dieser Tests, ist nicht der Test kaputt**, sondern das
//! Profil ist auf Geschwindigkeit umgestellt worden, ohne die Kosten zu
//! nennen.

/// Der Überlauf muss weiterhin panicken und nicht umlaufen.
///
/// `black_box` verhindert, dass der Optimierer die Rechnung zur
/// Übersetzungszeit ausführt; ohne ihn wäre `i32::MAX + 1` ein
/// Übersetzungsfehler und der Test bewiese nichts über den Lauf.
#[test]
#[should_panic(expected = "attempt to add with overflow")]
fn ueberlauf_paniked_noch_immer() {
    let a = std::hint::black_box(i32::MAX);
    let b = std::hint::black_box(1i32);
    let _ = std::hint::black_box(a + b);
}

/// Auch die Subtraktion, denn `saturating_sub` steht an mehreren
/// Stellen genau deshalb da, wo sie sonst unterliefe.
#[test]
#[should_panic(expected = "attempt to subtract with overflow")]
fn unterlauf_paniked_noch_immer() {
    let a = std::hint::black_box(0u64);
    let b = std::hint::black_box(1u64);
    let _ = std::hint::black_box(a - b);
}

/// `debug_assert!` muss weiterhin feuern; sonst sind die
/// dokumentierten Vorbedingungen aus Fund 75 unbewacht.
#[test]
#[should_panic(expected = "eine Vorbedingung")]
fn debug_assert_feuert_noch_immer() {
    debug_assert!(std::hint::black_box(false), "eine Vorbedingung");
}

/// Und die Schaltervariable selbst, damit die Ursache im Fehlerbild
/// steht und nicht nur die Wirkung.
#[test]
fn debug_assertions_sind_an() {
    // `black_box`, weil `cfg!` sonst zur Übersetzungszeit feststeht und
    // clippy die Zusicherung als konstant beanstandet. Die Aussage soll
    // aber genau die konstante sein.
    assert!(
        std::hint::black_box(cfg!(debug_assertions)),
        "Das Testprofil läuft ohne debug_assertions. Damit sind Überlaufprüfung \
         und jedes debug_assert! im Ganzzahlpfad abgeschaltet."
    );
}
