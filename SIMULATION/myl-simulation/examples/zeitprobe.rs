//! Zeitprobe fuer den Skalarkoerper: rechnen [`Skalar`]-Funktionen mit
//! geheimen Zahlen wirklich in konstanter Zeit?
//!
//!   cd SIMULATION/myl-simulation && cargo run --release --example zeitprobe [-- PROBEN [null|zahl]]
//!
//! `null` (Vorgabe) misst mit der Null als festem Wert, `zahl` mit einer
//! gewoehnlichen festen Zahl. Die Zeile `A/A` misst zufaellig gegen
//! zufaellig und darf nie auffallen; tut sie es, ist die Messung schief.
//! Ein drittes Argument `dit` schaltet vorher auf aarch64 Data Independent
//! Timing ein (PSTATE.DIT): Erst damit sagt Arm, und auf seinen Chips Apple,
//! wertunabhaengige Laufzeiten fuer die Rechenbefehle zu.
//!
//! ⚑ **Das Verfahren ist dudect** (Reparaz, Balasch, Verbauwhede, „Dude, is
//! my code constant time?", DATE 2017): Dieselbe Funktion rechnet abwechselnd
//! in zufaelliger Reihenfolge mit einem **festen** und einem **zufaelligen**
//! Geheimnis; ein Welch-t-Test vergleicht die Laufzeiten beider Klassen.
//! Haengt die Zeit am Wert, trennen sich die Klassen, und `|t|` waechst mit
//! der Zahl der Proben. Wie dort gilt `|t|` ueber 4,5 als Hinweis auf eine
//! Abhaengigkeit, ueber 10 als Befund. Gerechnet wird ueber alle Proben und
//! ueber die schnellsten 50, 75 und 90 %, damit Ausreisser des
//! Betriebssystems keinen Befund verdecken.
//!
//! ⛔️ **Mit Gegenprobe:** Eine Funktion, die absichtlich am Wert verzweigt,
//! muss hier auffallen. Tut sie es nicht, misst die Probe nichts, und ein
//! gruenes Ergebnis der echten Funktionen saegt nichts.
//!
//! 📌 **Gemessen am 2026-11-14 auf einem Apple M5 Pro**, je acht Laeufe mit
//! 200 000 Proben (fester Wert Null oder eine gewoehnliche Zahl, DIT aus
//! und an): `A/A` blieb immer unauffaellig (|t| bis 1,9), die Gegenprobe
//! immer ueber 5 000. **`invers` blieb in allen Laeufen unauffaellig** (|t|
//! bis 2,5); es rechnet 255 Quadrate und Multiplikationen, deren Operanden
//! vom Geheimnis abhaengen, also genau `mal` und `quadrat` in langer Folge.
//! ⚠️ **Die einzelnen kurzen Rechnungen ergaben kein stabiles Bild:** Ihr
//! |t| wanderte mit der Buendelgroesse und dem DIT-Schalter zwischen 2 und 55,
//! ohne Richtung. Gelesen als Effekt der Anordnung (die feste Klasse laedt
//! denselben Wert immer wieder), nicht belegt als Eigenschaft der Rechnung;
//! dafuer braucht es eine Messung mit Zyklenzaehler oder eine Pruefung
//! durch Dritte.
//!
//! ⚠️ **Im Release-Bau**, denn konstante Zeit ist eine Eigenschaft des
//! uebersetzten Codes; der Debug-Bau zeigt eine andere Maschine. Und eine
//! Messung auf einer Maschine belegt diese Maschine und diesen Uebersetzer,
//! nicht jede. Sie ersetzt keine Pruefung durch Dritte.

use std::hint::black_box;
use std::time::Instant;

use myl_types::skalar::Skalar;

/// Aufrufe je Messung: Eine einzelne Rechnung liegt unter der Aufloesung
/// der Uhr (unter macOS rund 42 ns). 📌 Mit 32 Aufrufen schwankten `plus` und
/// `minus` zwischen den Laeufen von |t| 1 bis 10: Die Uhr zaehlte dort nur
/// zwei, drei Schritte. Deshalb die billigen Rechnungen zu 512.
const JE_MESSUNG: usize = 512;

struct Zufall(u64);

impl Zufall {
    fn naechste(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn skalar(&mut self) -> Skalar {
        let mut b = [0u8; 64];
        for c in b.chunks_mut(8) {
            c.copy_from_slice(&self.naechste().to_le_bytes());
        }
        Skalar::aus_breit(&b)
    }
}

/// Welch-t ueber zwei Reihen.
fn welch(a: &[f64], b: &[f64]) -> f64 {
    let mittel = |x: &[f64]| x.iter().sum::<f64>() / x.len() as f64;
    let streu = |x: &[f64], m: f64| x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (x.len() as f64 - 1.0);
    let (ma, mb) = (mittel(a), mittel(b));
    let (sa, sb) = (streu(a, ma), streu(b, mb));
    (ma - mb) / (sa / a.len() as f64 + sb / b.len() as f64).sqrt()
}

/// Das groesste `|t|` ueber alle Proben und die schnellsten Anteile.
fn hoechstes_t(fest: &[f64], zufaellig: &[f64]) -> f64 {
    let mut alle: Vec<f64> = fest.iter().chain(zufaellig).copied().collect();
    alle.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut hoechst = welch(fest, zufaellig).abs();
    for anteil in [0.5, 0.75, 0.9] {
        let grenze = alle[((alle.len() as f64) * anteil) as usize];
        let f: Vec<f64> = fest.iter().copied().filter(|&v| v <= grenze).collect();
        let z: Vec<f64> = zufaellig.iter().copied().filter(|&v| v <= grenze).collect();
        if f.len() > 100 && z.len() > 100 {
            hoechst = hoechst.max(welch(&f, &z).abs());
        }
    }
    hoechst
}

/// Misst `f` mit einem festen und mit zufaelligen Geheimnissen, in
/// zufaelliger Reihenfolge, und gibt `|t|` zurueck.
fn messen(proben: usize, je: usize, zufall: &mut Zufall, fest: Option<Skalar>, f: &dyn Fn(&Skalar) -> Skalar) -> f64 {
    let (mut a, mut b) = (Vec::with_capacity(proben), Vec::with_capacity(proben));
    // Die Eingaben stehen vor der Uhr fest, damit ihr Erzeugen nicht mitzaehlt.
    let mut eingaben = [Skalar::NULL; JE_MESSUNG];
    while a.len() < proben || b.len() < proben {
        let klasse_fest = zufall.naechste() & 1 == 0;
        for e in eingaben[..je].iter_mut() {
            *e = match fest {
                Some(w) if klasse_fest => w,
                _ => zufall.skalar(),
            };
        }
        let start = Instant::now();
        for e in &eingaben[..je] {
            black_box(f(black_box(e)));
        }
        let dauer = start.elapsed().as_nanos() as f64;
        if klasse_fest {
            a.push(dauer);
        } else {
            b.push(dauer);
        }
    }
    hoechstes_t(&a, &b)
}

/// Schaltet DIT ein und liest es zurueck. ⚠️ Das einzige `unsafe` hier, und
/// es steht in diesem Messprogramm, nicht in der Bibliothek: ein Register,
/// keine Speicherstelle.
#[cfg(target_arch = "aarch64")]
fn dit_einschalten() -> bool {
    let bit: u64 = 1 << 24;
    let mut gelesen: u64;
    // S3_3_C4_C2_5 ist DIT; so geschrieben braucht der Assembler keine Erweiterung.
    unsafe {
        std::arch::asm!("msr s3_3_c4_c2_5, {0}", in(reg) bit, options(nomem, nostack));
        std::arch::asm!("mrs {0}, s3_3_c4_c2_5", out(reg) gelesen, options(nomem, nostack));
    }
    gelesen & bit != 0
}

#[cfg(not(target_arch = "aarch64"))]
fn dit_einschalten() -> bool {
    false
}

/// ⛔️ Die Gegenprobe: bricht ab, sobald ein Byte null ist, und rechnet sonst
/// weiter. Mit der Null als festem Wert ist sie dort schnell.
fn verzweigt(s: &Skalar) -> Skalar {
    let mut erg = *s;
    for b in s.als_bytes_be() {
        if b == 0 {
            return erg;
        }
        erg = erg.mal(s);
    }
    erg
}

/// Name, fester Wert (keiner: zufaellig gegen zufaellig), Rechnung, Proben,
/// Aufrufe je Messung.
type Fall = (&'static str, Option<Skalar>, Box<dyn Fn(&Skalar) -> Skalar>, usize, usize);

fn main() {
    let proben: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(50_000);
    if std::env::args().nth(3).as_deref() == Some("dit") {
        println!("DIT eingeschaltet: {}", if dit_einschalten() { "ja" } else { "nein" });
    }
    let mut zufall = Zufall(0x9e37_79b9_7f4a_7c15);
    let oeffentlich = zufall.skalar();
    let gewoehnlich = std::env::args().nth(2).as_deref() == Some("zahl");
    let zahl = zufall.skalar();
    let null = if gewoehnlich { zahl } else { Skalar::NULL };
    let eins = if gewoehnlich { zahl } else { Skalar::EINS };
    let faelle: [Fall; 8] = [
        ("A/A mal", None, Box::new(move |s| oeffentlich.mal(s)), proben, 64),
        ("mal", Some(null), Box::new(move |s| oeffentlich.mal(s)), proben, 64),
        ("quadrat", Some(null), Box::new(|s| s.quadrat()), proben, 64),
        ("plus", Some(null), Box::new(move |s| oeffentlich.plus(s)), proben / 4, 512),
        ("minus", Some(null), Box::new(move |s| oeffentlich.minus(s)), proben / 4, 512),
        ("negiert", Some(null), Box::new(|s| s.negiert()), proben / 4, 512),
        // Die Null nimmt `invers` vorher heraus (dokumentiert); gemessen wird
        // der Weg dahinter, mit der Eins als festem Wert. Teuer, also weniger.
        ("invers", Some(eins), Box::new(|s| s.invers().unwrap_or(Skalar::NULL)), proben / 50, 1),
        ("GEGENPROBE verzweigt", Some(Skalar::NULL), Box::new(verzweigt), proben, 64),
    ];
    let mut befund = false;
    for (name, fest, f, n, je) in &faelle {
        let t = messen((*n).max(1_000), *je, &mut zufall, *fest, f.as_ref());
        let urteil = if t > 10.0 { "BEFUND" } else if t > 4.5 { "Hinweis" } else { "kein Unterschied" };
        println!("{name:<22} |t| = {t:>8.2}   {urteil}");
        let gegenprobe = name.starts_with("GEGENPROBE");
        if gegenprobe && t <= 10.0 {
            println!("⛔️ Die Gegenprobe fiel nicht auf; die Messung traegt nicht.");
            befund = true;
        }
        if !gegenprobe && t > 4.5 {
            befund = true;
        }
    }
    std::process::exit(if befund { 1 } else { 0 });
}
