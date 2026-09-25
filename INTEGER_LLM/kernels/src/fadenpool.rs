//! Ein stehender Satz Fäden statt eines neuen je Matrix.
//!
//! # 📌 Warum es ihn gibt: 23 % der Laufzeit waren Fadenstart
//!
//! **Gemessen am 2026-09-11** auf M5 Pro gegen `myelith-4b`: Ein
//! `std::thread::scope` kostet 18,5 µs bei zwei und 84,5 µs bei zwölf
//! Fäden, denn jeder `spawn` legt einen Betriebssystemfaden an. Der
//! Vorwärtspass ruft je Token **253** solche Bereiche (36 Ebenen × 7
//! Matrizen + Kopf); zusammen **15,7 ms von 68,7 ms**.
//!
//! **Ein geparkter Faden dagegen wird geweckt und nicht erschaffen.**
//!
//! # ⚑ Und es ändert keine Zahl
//!
//! Jede Ausgabezeile ist ein eigenes Skalarprodukt über ihre eigene
//! Gewichtszeile und schreibt in ihr eigenes Feld. Zwischen den Zeilen
//! gibt es keine gemeinsame Zwischensumme, also auch keine Reihenfolge,
//! die etwas ändern könnte. **Wer die Zeilen rechnet, ist deshalb eine
//! reine Laufzeitfrage**, genau wie die Fadenzahl, und das ist dieselbe
//! Eigenschaft, aus der das ganze Projekt seine Bitgleichheit zieht.
//!
//! # ⚠️ Der eine `unsafe`-Block und warum er trägt
//!
//! Die Aufgabe trägt zwei Zeiger: auf die Funktion, die eine Zeile
//! rechnet, und auf das Ausgabefeld. Beide leben auf dem Stapel des
//! **aufrufenden** Fadens, und Rust kann die Lebensdauer nicht über
//! eine Kanalgrenze hinweg beweisen. Sie trägt trotzdem, aus einem
//! Grund, der hier geprüft und nicht behauptet wird:
//!
//! **Der Aufrufer blockiert, bis jeder beteiligte Faden gemeldet hat,
//! dass er fertig ist.** [`rechnen`] kehrt erst zurück, wenn der Zähler
//! `fertige` die Zahl der beteiligten Fäden erreicht hat; vorher
//! verlässt kein Zeiger seinen Gültigkeitsbereich. Das ist dieselbe
//! Zusicherung, die `std::thread::scope` gibt, nur von Hand.
//!
//! ⚠️ **Zwei Stellen dürfen deshalb nie geändert werden, ohne das hier
//! mitzulesen:** Die Warteschleife am Ende von [`rechnen`] und die
//! Meldung `fertige += 1` im Faden. Wer eine davon entfernt, macht aus
//! einem gültigen Zeiger einen baumelnden.
//!
//! ⚑ **Und kein Faden schreibt in ein fremdes Feld.** Die Abschnitte
//! sind disjunkt (`start = index * je`, Länge höchstens `je`), also gibt
//! es keinen gemeinsamen Schreibzugriff, den eine Sperre ordnen müsste.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

/// Was ein Faden zu tun hat.
///
/// ⚠️ Die Zeiger sind roh, weil die Lebensdauer nicht über die
/// Kanalgrenze reicht. Was sie trägt, steht im Modulkopf.
#[derive(Clone, Copy)]
struct Aufgabe {
    /// `&dyn Fn(usize) -> i16 + Sync`, in zwei Worte zerlegt.
    f_daten: *const (),
    f_tabelle: *const (),
    /// Anfang des Ausgabefelds.
    out: *mut i16,
    /// Wie viele Zeilen es insgesamt sind.
    zeilen: usize,
    /// Wie viele Zeilen ein Faden nimmt.
    je: usize,
    /// Wie viele Fäden mitrechnen.
    faeden: usize,
}

// ⚠️ Beides ist hier richtig und anderswo falsch: Die Zeiger werden nur
// innerhalb einer Runde benutzt, und die Runde endet, bevor der
// Aufrufer zurückkehrt. Siehe Modulkopf.
unsafe impl Send for Aufgabe {}
unsafe impl Sync for Aufgabe {}

#[derive(Default)]
struct Lage {
    /// Die laufende Aufgabe, falls eine läuft.
    aufgabe: Option<Aufgabe>,
    /// Dasselbe für den breiten Eingang; genau eines von beiden ist
    /// gesetzt.
    breit: Option<Breitaufgabe>,
    /// Wie viele Fäden diese Runde beendet haben.
    fertige: usize,
}

// 📌 **Hier stand ein Drehzähler, und er hat es langsamer gemacht.**
//
// Der Gedanke war richtig und die Messung dagegen: 253 Runden je Token
// liegen dicht beieinander, ein Faden, der kurz nachsieht, spart den
// Weckvorgang. **Gemessen am 2026-09-11 wurde es um 27 % langsamer**
// (18,5 auf 13,5 Token/s beim Prefill).
//
// **Der Grund ist die Aufteilung, nicht das Drehen.** An einer Runde
// sind meist nur zwei bis zwölf der Fäden beteiligt; die übrigen
// drehen dann nicht *statt* zu warten, sondern **gegen** die
// rechnenden: Sie nehmen Kerne und Speicherbandbreite weg, und genau
// die sind hier knapp.
//
// ⚑ **Ein Drehen wäre erst dann richtig, wenn alle Fäden an jeder
// Runde mitrechnen**, und das hiesse, die Aufteilung aufzugeben, die
// nach Arbeitsmenge entscheidet.

struct Pool {
    /// Zählt jede Runde hoch; daran erkennt ein Faden neue Arbeit.
    ///
    /// ⚑ **Ausserhalb der Sperre**, damit ein drehender Faden sie lesen
    /// kann, ohne sie zu nehmen.
    runde: AtomicU64,
    lage: Mutex<Lage>,
    /// Weckt die Fäden.
    arbeit: Condvar,
    /// Weckt den Aufrufer.
    fertig: Condvar,
    /// Wie viele Fäden bereitstehen.
    groesse: usize,
}

static POOL: OnceLock<&'static Pool> = OnceLock::new();

thread_local! {
    /// **Steht, solange dieser Faden an einer Runde rechnet**: als
    /// Poolfaden immer, als Aufrufer, waehrend er seinen eigenen Abschnitt
    /// traegt.
    ///
    /// ⚑ **Ein Aufruf aus einer Runde heraus rechnet selbst**, statt eine
    /// zweite zu beginnen. Die Runden laufen nacheinander (`REIHE`), und
    /// eine Runde, die auf eine zweite wartet, wartete auf sich selbst.
    /// Bis 2026-09-14 musste das jeder Aufrufer wissen; die Aufmerksamkeit
    /// wird aber aus beiden Lagen gerufen: im Decode allein, in der
    /// gebuendelten Vorbereitung aus einer Runde je Token.
    static IN_EINER_RUNDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Laeuft der aufrufende Faden gerade in einer Runde?
fn in_einer_runde() -> bool {
    IN_EINER_RUNDE.with(|r| r.get())
}

/// Setzt [`IN_EINER_RUNDE`] fuer die Dauer eines Abschnitts, auch wenn er
/// abbricht.
struct Rundenmarke;

impl Rundenmarke {
    fn setzen() -> Self {
        IN_EINER_RUNDE.with(|r| r.set(true));
        Rundenmarke
    }
}

impl Drop for Rundenmarke {
    fn drop(&mut self) {
        IN_EINER_RUNDE.with(|r| r.set(false));
    }
}

/// Wie viele Fäden der Pool hält.
///
/// ⚑ **Einmal und für den ganzen Prozess.** Die Kerngrenze des Nutzers
/// begrenzt, wie viele davon **mitrechnen** (siehe [`rechnen`]), nicht
/// wie viele bereitstehen: Ein geparkter Faden kostet nichts ausser
/// seinem Stapel, und eine Grenze, die sich zur Laufzeit ändern darf,
/// könnte die Zahl der Fäden sonst nicht mehr treffen.
fn groesse() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

fn pool() -> &'static Pool {
    POOL.get_or_init(|| {
        let groesse = groesse();
        let p: &'static Pool = Box::leak(Box::new(Pool {
            runde: AtomicU64::new(0),
            lage: Mutex::new(Lage::default()),
            arbeit: Condvar::new(),
            fertig: Condvar::new(),
            groesse,
        }));
        for index in 0..groesse {
            // ⚑ Die Fäden leben so lange wie der Prozess; deshalb ein
            // geleakter Verweis statt eines `Arc`, der bei jedem Wecken
            // einen Zähler anfasst.
            std::thread::Builder::new()
                .name(format!("myl-rechenfaden-{index}"))
                .spawn(move || faden(p, index))
                .expect("Rechenfaden");
        }
        p
    })
}

/// Ein Faden: warten, seinen Abschnitt rechnen, melden.
fn faden(p: &'static Pool, index: usize) {
    IN_EINER_RUNDE.with(|r| r.set(true));
    let mut gesehen = 0u64;
    loop {
        let (aufgabe, breit) = {
            let mut lage = p.lage.lock().unwrap_or_else(|e| e.into_inner());
            while p.runde.load(Ordering::Acquire) == gesehen
                || (lage.aufgabe.is_none() && lage.breit.is_none())
            {
                lage = p.arbeit.wait(lage).unwrap_or_else(|e| e.into_inner());
            }
            gesehen = p.runde.load(Ordering::Acquire);
            (lage.aufgabe, lage.breit)
        };
        if let Some(b) = breit {
            if index + 1 >= b.faeden {
                continue;
            }
            unsafe { breit_rechnen(&b, index) };
            {
                let mut lage = p.lage.lock().unwrap_or_else(|e| e.into_inner());
                lage.fertige += 1;
            }
            p.fertig.notify_all();
            continue;
        }
        let Some(a) = aufgabe else { continue };
        // 📌 **Nur wer mitrechnet, meldet sich.** Die erste Fassung liess
        // **alle** Faeden melden, auch die unbeteiligten, und der
        // Aufrufer wartete auf sie. Damit kostete eine kleine Matrix mit
        // zwei Rechnern trotzdem zwoelf Weckvorgaenge und zwoelf
        // Sperrzugriffe auf denselben Zaehler.
        if index + 1 >= a.faeden {
            continue;
        }
        // ⚠️ Der Zeiger ist gültig, solange diese Runde läuft; der
        // Aufrufer wartet auf die Meldung unten. Siehe Modulkopf.
        unsafe { abschnitt_rechnen(&a, index) };
        {
            let mut lage = p.lage.lock().unwrap_or_else(|e| e.into_inner());
            lage.fertige += 1;
        }
        p.fertig.notify_all();
    }
}

/// # Sicherheit
///
/// `a` muss aus einer laufenden Runde stammen, in der der Aufrufer noch
/// wartet. Der Abschnitt `index` wird von genau einem Faden berührt.
unsafe fn abschnitt_rechnen(a: &Aufgabe, index: usize) {
    let f: &(dyn Fn(usize) -> i16 + Sync) =
        unsafe { std::mem::transmute((a.f_daten, a.f_tabelle)) };
    let start = index * a.je;
    if start >= a.zeilen {
        return;
    }
    let ende = (start + a.je).min(a.zeilen);
    for i in start..ende {
        unsafe { *a.out.add(i) = f(i) };
    }
}

/// **Rechnet `zeilen` Zeilen mit `faeden` Fäden und kehrt erst zurück,
/// wenn alle fertig sind.**
///
/// ⚑ `faeden` ist eine Laufzeitentscheidung des Aufrufers und ändert
/// kein Ergebnis; mehr als [`Pool::groesse`] werden nicht beteiligt.
pub fn rechnen<F>(zeilen: usize, faeden: usize, f: F) -> Vec<i16>
where
    F: Fn(usize) -> i16 + Sync,
{
    let p = pool();
    let faeden = faeden.clamp(1, p.groesse);
    let mut out = vec![0i16; zeilen];
    if faeden == 1 || zeilen < 2 || in_einer_runde() {
        for (i, ziel) in out.iter_mut().enumerate() {
            *ziel = f(i);
        }
        return out;
    }

    let als_dyn: &(dyn Fn(usize) -> i16 + Sync) = &f;
    let (f_daten, f_tabelle): (*const (), *const ()) =
        unsafe { std::mem::transmute(als_dyn) };
    let aufgabe = Aufgabe {
        f_daten,
        f_tabelle,
        out: out.as_mut_ptr(),
        zeilen,
        je: zeilen.div_ceil(faeden),
        faeden,
    };

    // ⚑ **Eine Runde nach der anderen.** Die Sperre um die ganze Runde
    // macht den Pool zu einem Nadelöhr für nebenläufige Aufrufer, und
    // das ist hier richtig: Der Rechenpfad ruft ihn aus **einem** Faden,
    // und zwei gleichzeitige Runden bräuchten zwei Zählerwerke.
    let _reihe = REIHE.lock().unwrap_or_else(|e| e.into_inner());
    {
        let mut lage = p.lage.lock().unwrap_or_else(|e| e.into_inner());
        lage.aufgabe = Some(aufgabe);
        lage.fertige = 0;
        // ⚑ **Die Runde wird zuletzt hochgezählt**, mit `Release`: Wer
        // sie drehend liest, sieht damit auch die Aufgabe, die davor
        // geschrieben wurde.
        p.runde.fetch_add(1, Ordering::Release);
    }
    p.arbeit.notify_all();

    // ⚑ **Der Aufrufer rechnet mit, statt zu warten.** Er hat ohnehin
    // nichts zu tun, bis die anderen fertig sind; so trägt er den
    // letzten Abschnitt und spart einen Weckvorgang.
    {
        let _marke = Rundenmarke::setzen();
        unsafe { abschnitt_rechnen(&aufgabe, faeden - 1) };
    }

    // ⚠️ **Diese Schleife trägt den ganzen `unsafe`-Block.** Sie wartet,
    // bis jeder **beteiligte** Faden gemeldet hat; erst danach darf
    // `out` aus dem Gültigkeitsbereich fallen. Unbeteiligte Fäden
    // berühren weder `out` noch die Funktion.
    {
        let mut lage = p.lage.lock().unwrap_or_else(|e| e.into_inner());
        while lage.fertige < faeden - 1 {
            lage = p.fertig.wait(lage).unwrap_or_else(|e| e.into_inner());
        }
        lage.aufgabe = None;
    }
    out
}

/// **Wie [`rechnen`], aber jede Zeile schreibt `breite` Werte.**
///
/// ⚑ **Für Matrizen, die für mehrere Eingaben zugleich gerechnet
/// werden.** Die Gewichtszeile `z` wird einmal gelesen und für alle
/// `breite` Eingaben benutzt; genau daran hängt der Gewinn.
///
/// Die Rückgabe liegt zeilenweise (`out[z * breite + b]`), denn so
/// schreibt jeder Faden in seinen eigenen zusammenhängenden Bereich.
/// Das Umlegen auf Eingabevektoren macht der Aufrufer.
pub fn rechnen_breit<F>(zeilen: usize, breite: usize, faeden: usize, f: F) -> Vec<i16>
where
    F: Fn(usize, &mut [i16]) + Sync,
{
    let mut out = vec![0i16; zeilen * breite];
    if faeden <= 1 || zeilen < 2 || in_einer_runde() {
        for z in 0..zeilen {
            f(z, &mut out[z * breite..(z + 1) * breite]);
        }
        return out;
    }
    let p = pool();
    let faeden = faeden.clamp(2, p.groesse);

    // ⚑ **Derselbe Aufbau wie in [`rechnen`]**, nur schreibt eine Zeile
    // mehrere Werte. Die Abschnitte bleiben disjunkt, also gilt die
    // Begründung im Modulkopf unverändert.
    let als_dyn: &(dyn Fn(usize, &mut [i16]) + Sync) = &f;
    let (f_daten, f_tabelle): (*const (), *const ()) = unsafe { std::mem::transmute(als_dyn) };
    let aufgabe = Breitaufgabe {
        f_daten,
        f_tabelle,
        out: out.as_mut_ptr(),
        zeilen,
        breite,
        je: zeilen.div_ceil(faeden),
        faeden,
    };

    let _reihe = REIHE.lock().unwrap_or_else(|e| e.into_inner());
    {
        let mut lage = p.lage.lock().unwrap_or_else(|e| e.into_inner());
        lage.breit = Some(aufgabe);
        lage.fertige = 0;
        p.runde.fetch_add(1, Ordering::Release);
    }
    p.arbeit.notify_all();
    {
        let _marke = Rundenmarke::setzen();
        unsafe { breit_rechnen(&aufgabe, faeden - 1) };
    }
    {
        let mut lage = p.lage.lock().unwrap_or_else(|e| e.into_inner());
        while lage.fertige < faeden - 1 {
            lage = p.fertig.wait(lage).unwrap_or_else(|e| e.into_inner());
        }
        lage.breit = None;
    }
    out
}

/// Wie [`Aufgabe`], aber mit mehreren Werten je Zeile.
#[derive(Clone, Copy)]
struct Breitaufgabe {
    f_daten: *const (),
    f_tabelle: *const (),
    out: *mut i16,
    zeilen: usize,
    breite: usize,
    je: usize,
    faeden: usize,
}

// ⚠️ Wie bei [`Aufgabe`]: Die Zeiger gelten nur innerhalb einer Runde,
// und die Runde endet, bevor der Aufrufer zurückkehrt.
unsafe impl Send for Breitaufgabe {}
unsafe impl Sync for Breitaufgabe {}

/// # Sicherheit
///
/// Wie [`abschnitt_rechnen`].
unsafe fn breit_rechnen(a: &Breitaufgabe, index: usize) {
    let f: &(dyn Fn(usize, &mut [i16]) + Sync) =
        unsafe { std::mem::transmute((a.f_daten, a.f_tabelle)) };
    let start = index * a.je;
    if start >= a.zeilen {
        return;
    }
    let ende = (start + a.je).min(a.zeilen);
    for z in start..ende {
        let ziel = unsafe { std::slice::from_raw_parts_mut(a.out.add(z * a.breite), a.breite) };
        f(z, ziel);
    }
}

/// **Wie [`rechnen`], mit einem i32 je Zeile.**
///
/// ⚑ **Für den LM-Kopf** (2026-09-14): Seine Logits sind i32, und er rechnete
/// einkernig, beim 30B 29 % eines Decode-Schritts. Umgesetzt über
/// [`rechnen_breit`] mit zwei Feldern je Zeile, dem unteren und dem oberen
/// Halbwort; zusammengesetzt wird bitgenau. Eine zweite Aufgabenart im Pool
/// wäre ein zweiter `unsafe`-Block mit derselben Begründung.
pub fn rechnen_i32<F>(zeilen: usize, faeden: usize, f: F) -> Vec<i32>
where
    F: Fn(usize) -> i32 + Sync,
{
    let flach = rechnen_breit(zeilen, 2, faeden, |z, ziel| {
        let wert = f(z) as u32;
        ziel[0] = wert as u16 as i16;
        ziel[1] = (wert >> 16) as u16 as i16;
    });
    flach
        .chunks_exact(2)
        .map(|p| (((p[1] as u16 as u32) << 16) | p[0] as u16 as u32) as i32)
        .collect()
}

/// **Wie [`rechnen`], mit einem beliebigen Ergebnis je Index.**
///
/// ⚑ Fuer Schritte, die je Token mehrere Felder verschiedener Art
/// liefern, etwa die Vorbereitung einer rekurrenten Ebene (2026-09-25).
/// Umgesetzt ueber [`rechnen`] und ohne eigenen `unsafe`-Block: Jeder
/// Index fuellt genau sein eigenes Fach, ein `OnceLock`, und `rechnen`
/// kehrt erst zurueck, wenn alle fertig sind. Die Reihenfolge der
/// Ergebnisse ist die der Indizes, gleich wie viele Faeden rechnen.
pub fn verteilen<T, F>(anzahl: usize, faeden: usize, f: F) -> Vec<T>
where
    T: Send + Sync,
    F: Fn(usize) -> T + Sync,
{
    let faecher: Vec<std::sync::OnceLock<T>> = (0..anzahl).map(|_| std::sync::OnceLock::new()).collect();
    let _ = rechnen(anzahl, faeden, |i| {
        if faecher[i].set(f(i)).is_err() {
            unreachable!("jeder Index wird genau einmal gerechnet");
        }
        0
    });
    faecher
        .into_iter()
        .map(|fach| fach.into_inner().expect("jeder Index ist gerechnet"))
        .collect()
}

/// Hält gleichzeitige Aufrufer auseinander; siehe [`rechnen`].
static REIHE: Mutex<()> = Mutex::new(());

/// Wie viele Fäden bereitstehen, für die Auskunft.
pub fn faeden() -> usize {
    pool().groesse
}

/// Wie oft der Pool bisher geweckt wurde; nur für Messungen.
pub static RUNDEN: AtomicUsize = AtomicUsize::new(0);

/// Zählt eine Runde mit, für `bench`-Auswertungen.
pub fn runde_zaehlen() {
    RUNDEN.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Der Pool rechnet dasselbe wie die Schleife.**
    ///
    /// 📌 Die Gegenprobe zur ganzen Datei: Jede Fadenzahl, jede
    /// Zeilenzahl, und alles muss bitgleich sein.
    /// **Ein Aufruf aus einer Runde heraus rechnet selbst und dasselbe**,
    /// statt auf eine zweite Runde zu warten, die nie beginnt. Ohne die
    /// Marke haengt diese Probe.
    #[test]
    fn eine_runde_in_einer_runde_rechnet_selbst() {
        let innen = |z: usize| (0..40).map(|i| ((z * 31 + i * 7) % 1000) as i16).collect::<Vec<_>>();
        let aussen = rechnen_breit(24, 40, 8, |z, ziel| {
            let werte = rechnen_breit(40, 1, 8, |i, feld| feld[0] = innen(z)[i]);
            ziel.copy_from_slice(&werte);
        });
        let erwartet: Vec<i16> = (0..24).flat_map(innen).collect();
        assert_eq!(aussen, erwartet);
        assert!(!in_einer_runde(), "nach der Runde steht die Marke wieder");
    }

    #[test]
    fn jede_fadenzahl_gibt_dasselbe() {
        for zeilen in [0usize, 1, 2, 3, 7, 16, 33, 64, 129, 1000] {
            let erwartet: Vec<i16> =
                (0..zeilen).map(|i| ((i * 37) % 65536) as i16).collect();
            for faeden in [1usize, 2, 3, 4, 8, 12, 64] {
                let bekommen = rechnen(zeilen, faeden, |i| ((i * 37) % 65536) as i16);
                assert_eq!(
                    bekommen, erwartet,
                    "{zeilen} Zeilen mit {faeden} Faeden weichen ab"
                );
            }
        }
    }

    /// **Auch hintereinander, viele Male.**
    ///
    /// ⚠️ Ein Pool, der eine Runde nicht sauber abschliesst, faellt
    /// nicht beim ersten Mal auf, sondern beim tausendsten.
    #[test]
    fn tausend_runden_bleiben_richtig() {
        for runde in 0..1000u32 {
            let n = 97;
            let bekommen = rechnen(n, 8, |i| (i as u32 + runde) as i16);
            let erwartet: Vec<i16> = (0..n).map(|i| (i as u32 + runde) as i16).collect();
            assert_eq!(bekommen, erwartet, "Runde {runde}");
        }
    }

    /// **Und aus mehreren Faeden zugleich.**
    ///
    /// ⚑ Der Rechenpfad ruft aus einem Faden, aber eine Pruefung darf
    /// nicht davon abhaengen: `REIHE` haelt sie auseinander.
    #[test]
    fn nebenlaeufige_aufrufer_stoeren_sich_nicht() {
        std::thread::scope(|s| {
            for k in 0..4u32 {
                s.spawn(move || {
                    for _ in 0..50 {
                        let n = 64 + k as usize;
                        let bekommen = rechnen(n, 6, |i| (i as u32 * (k + 1)) as i16);
                        let erwartet: Vec<i16> =
                            (0..n).map(|i| (i as u32 * (k + 1)) as i16).collect();
                        assert_eq!(bekommen, erwartet);
                    }
                });
            }
        });
    }

    /// **Der Pool hat mindestens einen Faden.**
    #[test]
    fn der_pool_steht() {
        assert!(faeden() >= 1);
    }

    /// **`verteilen` liefert je Index sein eigenes Ergebnis, in der
    /// Reihenfolge der Indizes**, bei jeder Fadenzahl und auch fuer
    /// Ergebnisse, die kein einzelner Wert sind.
    #[test]
    fn verteilen_ist_die_schleife() {
        let f = |i: usize| -> (Vec<i64>, u8) { ((0..i % 5).map(|j| (i * 31 + j) as i64).collect(), (i % 7) as u8) };
        for anzahl in [0usize, 1, 2, 17, 100] {
            let erwartet: Vec<_> = (0..anzahl).map(f).collect();
            for faeden in [1usize, 2, 3, 16] {
                assert_eq!(verteilen(anzahl, faeden, f), erwartet, "{anzahl} Indizes, {faeden} Faeden");
            }
        }
    }

}
