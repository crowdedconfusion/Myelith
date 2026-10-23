//! Gebuendelte W8A16-Matrizen auf Grafikkarten von NVIDIA (CUDA) und AMD
//! (ROCm), auch auf mehreren Karten am selben System zugleich.
//!
//! # ⚑ Was hier gerechnet wird, und was nicht
//!
//! **Dasselbe wie in [`crate::ressourcen::metal`]**: viele Eingaben auf denselben
//! Gewichten, also die Matrizen einer Ebene in der Vorbereitung eines
//! Prompts und die Experten eines Gemischs, fuer int8, ternaer und in vier
//! Bit gepackte Gewichte. Alles andere rechnet die CPU, auch der Decode.
//!
//! ⚑ **Der Decode auf der Karte ist zurueckgestellt** (Festlegung des
//! Projektinhabers, 2026-10-14: nur, wenn er sicher problemlos geht, sonst
//! bis Hardware da ist). Er braucht Gewichte, die dauernd auf der Karte
//! liegen, und einen Eingriff in das Modell; beides wird erst mit einer
//! Messung auf einer Karte entschieden.
//!
//! # ⚑ Die Gewichte werden je Aufruf hochgeladen
//!
//! Auf Apple-Silizium teilen CPU und GPU den Speicher, und Metal liest die
//! Gewichte ohne Kopie. Eine Steckkarte haengt am PCIe-Bus. **Ein Speicher
//! auf der Karte, der sich Gewichte nach ihrer Adresse merkt, waere
//! falsch**: Nach dem Freigeben eines Vektors liegen an derselben Stelle
//! andere Zahlen (derselbe Grund wie im Kopf von `metal/geraet.rs`), und
//! ein Training aendert Gewichte an Ort und Stelle. Den Inhalt zu pruefen
//! kostet so viel wie das Hochladen selbst. Deshalb hier: jedes Mal
//! hochladen, und die Schwelle so, dass sich das lohnt.
//!
//! # ⚑ Die Rechnung ist eigener Code
//!
//! Keine Rechenbibliothek des Herstellers: cuBLAS und rocBLAS rechnen in
//! Gleitkomma oder auf Tensor Cores. Die Kerne stehen in `kerne.cu`, ein
//! Dialekt fuer NVRTC und hiprtc, zur Laufzeit uebersetzt wie die Shader
//! von Metal. Sie summieren exakt in int32 je Stueck und int64 gesamt und
//! runden und klemmen genau wie `fixed_point.rs`.
//!
//! # ⚑ Drei Schranken, wie bei Metal
//!
//! 1. **Eine Selbstpruefung je Karte und Prozess**, bevor die Karte zum
//!    ersten Mal rechnen darf: Extremwerte, Kachelraender, alle Faelle der
//!    Umskalierung, ternaere Codes bis +2, vier Bit mit dem groessten
//!    Schritt und Versatz, mehrere Auftraege zugleich,
//!    verglichen mit der CPU. **Eine Karte, die anders rechnet, faellt
//!    allein heraus**; die anderen rechnen weiter.
//! 2. **Der Konformitaetslauf mit erzwungener Karte** (`run.sh cuda` bzw.
//!    `run.sh rocm`), abgelehnt, wenn dabei keine Karte gerechnet hat
//!    ([`gerechnet`]).
//! 3. **Der Nachbau** (`nachbau.rs`): dieselbe Quelle als C++ auf der CPU,
//!    gegen die CPU-Kerne, ohne Karte pruefbar.
//!
//! # ⚑ Mehrere Karten
//!
//! Der [`verteiler`] teilt die Arbeit, jede Karte rechnet in ihrem eigenen
//! Faden, und zusammengesetzt wird ohne Rechnung. Siehe dort, warum das
//! bitgleich bleibt.
//!
//! # Die Stellschrauben
//!
//! - `MYL_GPU_AB`: ab wie vielen Eingaben die Karten rechnen; `0` schaltet
//!   sie ab, `1` erzwingt sie (Konformitaetslauf). Ohne Angabe
//!   [`VORGABE_AB`].
//! - `MYL_GPU_GERAETE`: welche Karten, etwa `0,1` oder `cuda:0,rocm:1`.
//!   Ohne Angabe alle, die die Selbstpruefung bestehen.
//! - `MYL_GPU_GEWICHTE`: der Anteil je Karte in der Reihenfolge der
//!   genutzten Karten, etwa `3,1`. Ohne Angabe bei NVIDIA die Zahl der
//!   Multiprozessoren, bei AMD gleich.
//! - Wo die Bibliotheken liegen: `MYL_CUDA_TREIBER`, `MYL_NVRTC`,
//!   `MYL_HIP`, `MYL_HIPRTC`, `ROCM_PATH`; die Architektur einer AMD-Karte
//!   `MYL_HIP_ARCH`.

#[cfg(any(feature = "cuda", feature = "rocm"))]
mod lader;
#[cfg(any(feature = "cuda", feature = "rocm"))]
mod nachbaukarte;
#[cfg(any(feature = "cuda", feature = "rocm"))]
mod pruefung;
#[cfg(any(feature = "cuda", feature = "rocm"))]
mod treiber;
pub mod verteiler;

#[cfg(all(test, any(feature = "cuda", feature = "rocm")))]
mod nachbau;
// Die Verteilung an sechs nachgebildeten Karten, in jedem Bau.
#[cfg(test)]
mod attrappe;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Wer die Karte gebaut hat.
#[cfg(any(feature = "cuda", feature = "rocm"))]
pub use treiber::Hersteller;

/// Wer die Karte gebaut hat.
#[cfg(not(any(feature = "cuda", feature = "rocm")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hersteller {
    /// CUDA.
    Nvidia,
    /// ROCm, also HIP.
    Amd,
}

#[cfg(not(any(feature = "cuda", feature = "rocm")))]
impl Hersteller {
    /// Der Name des Rechenwegs, wie ihn `rechenpfad` und `run.sh` kennen.
    pub fn rechenweg(self) -> &'static str {
        match self {
            Hersteller::Nvidia => "cuda",
            Hersteller::Amd => "rocm",
        }
    }
}

/// Ab wie vielen Eingaben die Karten rechnen, wenn nichts anderes gesetzt ist.
///
/// ⚠️ **Eine Vorgabe und keine Messung.** Sie ist von Metal uebernommen
/// (dort gemessen). Auf einer Steckkarte kommt das Hochladen der Gewichte
/// dazu, die Schwelle liegt also eher hoeher. Gemessen wird sie auf der
/// ersten Mietmaschine (`karten_messung`).
pub const VORGABE_AB: usize = 16;

/// Noch nicht gelesen.
const UNGELESEN: usize = usize::MAX;

static AB: AtomicUsize = AtomicUsize::new(UNGELESEN);

/// Wie viele Auftraege die Karten in diesem Prozess gerechnet haben.
static GERECHNET: AtomicUsize = AtomicUsize::new(0);

/// Ob NVIDIA- bzw. AMD-Karten rechnen duerfen (der Schalter je Rechenweg).
static NVIDIA_AN: AtomicBool = AtomicBool::new(true);
static AMD_AN: AtomicBool = AtomicBool::new(true);

/// Die PCI-Adressen der Karten, die der Nutzer abgeschaltet hat.
static ABGESCHALTET: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// **Haben die Karten ueberhaupt gerechnet?**
///
/// 📌 **Die Lehre aus Fund 33**, wie bei Metal: Ein Konformitaetslauf, der
/// unter dem Namen einer Karte die CPU rechnet, saehe aus wie ein
/// bestandener. `run.sh` liest diesen Zaehler.
pub fn gerechnet() -> usize {
    GERECHNET.load(Ordering::Relaxed)
}

/// **Ab wie vielen Eingaben die Karten rechnen.** `0` heisst: nie.
pub fn schwelle() -> usize {
    let wert = AB.load(Ordering::Relaxed);
    if wert != UNGELESEN {
        return wert;
    }
    let aus_umgebung = std::env::var("MYL_GPU_AB")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .filter(|&n: &usize| n != UNGELESEN)
        .unwrap_or(VORGABE_AB);
    let _ = AB.compare_exchange(UNGELESEN, aus_umgebung, Ordering::Relaxed, Ordering::Relaxed);
    AB.load(Ordering::Relaxed)
}

/// Setzt die Schwelle fuer den ganzen Prozess. **Keine Zahl haengt
/// daran**, nur wo gerechnet wird.
pub fn schwelle_setzen(n: usize) {
    AB.store(n.min(UNGELESEN - 1), Ordering::Relaxed);
}

/// **Schaltet die Karten eines Herstellers ab oder wieder an.** Wie die
/// Schwelle: Es aendert, wo gerechnet wird, nicht was.
pub fn hersteller_setzen(h: Hersteller, an: bool) {
    match h {
        Hersteller::Nvidia => NVIDIA_AN.store(an, Ordering::Relaxed),
        Hersteller::Amd => AMD_AN.store(an, Ordering::Relaxed),
    }
}

/// **Schaltet eine einzelne Karte ab oder wieder an**, ueber ihre
/// PCI-Adresse (`0000:3b:00.0`, jede Schreibweise).
///
/// ⚑ **Je Karte und nicht je Hersteller**, denn der Client gibt jede Karte
/// mit ihrem eigenen Regler frei. 📌 Vorher gab es nur den Schalter je
/// Hersteller, und der Client rief ihn fuer jede Karte nacheinander: Bei
/// zwei NVIDIA-Karten, von denen eine aus sein sollte, entschied die
/// zuletzt bearbeitete fuer beide (Fund 534).
///
/// Gibt zurueck, ob eine bereite Karte diese Adresse traegt. `false` heisst:
/// Es gab nichts zu schalten (keine solche Karte, oder sie hat die
/// Selbstpruefung nicht bestanden). Gemerkt wird der Wunsch trotzdem.
pub fn karte_setzen(pci: &str, an: bool) -> bool {
    let adresse = pci_normal(pci);
    if let Ok(mut aus) = ABGESCHALTET.lock() {
        aus.retain(|a| *a != adresse);
        if !an {
            aus.push(adresse.clone());
        }
    }
    karten().iter().any(|k| k.pci == adresse)
}

/// Ob die Karte mit dieser PCI-Adresse rechnen darf.
pub fn karte_an(pci: &str) -> bool {
    let adresse = pci_normal(pci);
    ABGESCHALTET.lock().map(|aus| !aus.contains(&adresse)).unwrap_or(true)
}

/// **Eine PCI-Adresse in einer Schreibweise**: `dddd:bb:ss.f`, klein.
///
/// ⚑ CUDA nennt `0000:3B:00.0` oder `00000000:3B:00.0`, HIP und `sysfs`
/// `0000:3b:00.0`. Verglichen wird erst nach dieser Umformung; was sich
/// nicht lesen laesst, bleibt klein geschrieben stehen und trifft dann
/// eben nichts.
pub fn pci_normal(roh: &str) -> String {
    let t = roh.trim().to_ascii_lowercase();
    let teile: Vec<&str> = t.split(':').collect();
    let (domaene, bus, rest) = match teile.as_slice() {
        [d, b, r] => (*d, *b, *r),
        [b, r] => ("0", *b, *r),
        _ => return t,
    };
    let Some((steckplatz, funktion)) = rest.split_once('.') else {
        return t;
    };
    let hex = |s: &str| u32::from_str_radix(s, 16).ok();
    match (hex(domaene), hex(bus), hex(steckplatz), hex(funktion)) {
        (Some(d), Some(b), Some(s), Some(f)) => format!("{d:04x}:{b:02x}:{s:02x}.{f:x}"),
        _ => t,
    }
}


/// Ob die Karten dieses Herstellers rechnen duerfen.
pub fn hersteller_an(h: Hersteller) -> bool {
    match h {
        Hersteller::Nvidia => NVIDIA_AN.load(Ordering::Relaxed),
        Hersteller::Amd => AMD_AN.load(Ordering::Relaxed),
    }
}

/// Die Gewichte eines Auftrags.
#[derive(Clone, Copy)]
pub enum Gewichte<'a> {
    /// Flach, Zeile fuer Zeile.
    Int8(&'a [i8]),
    /// Ternaer gepackt wie in `crate::ternaer`.
    Ternaer { muster: &'a [u8], betraege: &'a [i16] },
    /// Vier Bit je Gewicht wie in `crate::vierbit`: Codes und je Gruppe
    /// Schritt und Versatz.
    Vierbit { codes: &'a [u8], gitter: &'a [u8] },
}

/// Ein Stapel fuer [`stapel_viele`], mit denselben Angaben wie [`stapel`].
pub struct Auftrag<'a> {
    pub xs: &'a [&'a [i16]],
    pub w: Gewichte<'a>,
    pub in_features: usize,
    pub w_shifts: &'a [u8],
    pub act_frac_bits: u8,
    pub ziel: &'a (dyn Fn(usize) -> u8 + Sync),
}

/// **Wuerde ein Stapel mit `eingaben` Eingaben auf einer Karte rechnen?**
/// Dieselbe Bedingung wie in [`stapel`], an einer Stelle.
pub fn nimmt(eingaben: usize) -> bool {
    let ab = schwelle();
    ab != 0 && eingaben >= ab && verfuegbar()
}

/// Rechnet diese Buendelung auf den Karten, wenn sie sich lohnt und eine
/// Karte bereit ist. `None` heisst: Die CPU rechnet, und das Ergebnis ist
/// dasselbe.
pub fn stapel(
    xs: &[&[i16]],
    w: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    ziel: &(dyn Fn(usize) -> u8 + Sync),
) -> Option<Vec<Vec<i16>>> {
    let a = Auftrag { xs, w: Gewichte::Int8(w), in_features, w_shifts, act_frac_bits, ziel };
    stapel_viele(std::slice::from_ref(&a)).and_then(|mut v| v.pop())
}

/// **Wie [`stapel`], fuer ternaer gepackte Gewichte.**
pub fn stapel_ternaer(
    xs: &[&[i16]],
    muster: &[u8],
    betraege: &[i16],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    ziel: &(dyn Fn(usize) -> u8 + Sync),
) -> Option<Vec<Vec<i16>>> {
    let a = Auftrag { xs, w: Gewichte::Ternaer { muster, betraege }, in_features, w_shifts, act_frac_bits, ziel };
    stapel_viele(std::slice::from_ref(&a)).and_then(|mut v| v.pop())
}

/// **Wie [`stapel`], fuer Gewichte in vier Bit** (`crate::vierbit`): Codes
/// und Gitter statt int8. Der Kern rechnet je Gruppe `a * (q * x) + b * x`,
/// dieselbe ganze Zahl wie die CPU.
pub fn stapel_vierbit(
    xs: &[&[i16]],
    codes: &[u8],
    gitter: &[u8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    ziel: &(dyn Fn(usize) -> u8 + Sync),
) -> Option<Vec<Vec<i16>>> {
    let a = Auftrag { xs, w: Gewichte::Vierbit { codes, gitter }, in_features, w_shifts, act_frac_bits, ziel };
    stapel_viele(std::slice::from_ref(&a)).and_then(|mut v| v.pop())
}

/// **Viele Stapel auf einmal**, verteilt auf alle bereiten Karten, oder
/// `None`, wenn die CPU rechnet.
///
/// ⚑ Die Schwelle gilt fuer die **Summe** der Eingaben, wie bei Metal.
pub fn stapel_viele(auftraege: &[Auftrag<'_>]) -> Option<Vec<Vec<Vec<i16>>>> {
    let ab = schwelle();
    let eingaben: usize = auftraege.iter().map(|a| a.xs.len()).sum();
    if ab == 0 || eingaben < ab || auftraege.is_empty() {
        return None;
    }
    rechnen(auftraege)
}

#[cfg(any(feature = "cuda", feature = "rocm"))]
fn rechnen(auftraege: &[Auftrag<'_>]) -> Option<Vec<Vec<Vec<i16>>>> {
    let jobs: Vec<Job<'_>> = auftraege.iter().map(Job::aus).collect::<Option<_>>()?;
    let v = verbund();
    let karten: Vec<&treiber::Karte> = v.karten.iter().filter(|k| k.darf_rechnen()).collect();
    if karten.is_empty() {
        return None;
    }
    let rechner: Vec<&dyn Rechner> = karten.iter().map(|k| *k as &dyn Rechner).collect();
    let (aus, ausfaelle) = mit_ersatz(&rechner, &jobs);
    for (i, grund) in ausfaelle {
        let k = karten[i];
        k.ausgefallen.store(true, Ordering::Relaxed);
        eprintln!(
            "⛔️ Grafikkarte: {} Karte {} ({}) faellt aus ({grund}); die uebrigen rechnen weiter",
            k.hersteller.rechenweg(),
            k.nummer,
            k.pci
        );
    }
    if aus.is_some() {
        GERECHNET.fetch_add(jobs.len(), Ordering::Relaxed);
    }
    aus
}

/// **Rechnet auf den Karten und ersetzt eine, die ausfaellt.**
///
/// ⚑ **Warum ein Ersatzlauf** (2026-10-14, vorbereitet fuer eine Maschine
/// mit sechs Karten): Faellt eine Karte mitten im Lauf aus (ein Fehler des
/// Treibers, voller Speicher), haette sonst jedes weitere Buendel die CPU
/// gerechnet, obwohl fuenf Karten bereit sind. Hier wird dasselbe Buendel
/// auf den uebrigen Karten neu verteilt, bis es gelingt oder keine mehr da
/// ist. Gibt das Ergebnis und die ausgefallenen Karten (Platz in `karten`)
/// zurueck; der Aufrufer nimmt sie dauerhaft heraus.
///
/// ⚠️ Ein Fehler, der keiner Karte gehoert (eine nicht gedeckte Zeile), ist
/// ein Fehler der Verteilung; dann rechnet die CPU, ohne dass eine Karte
/// dafuer haftet.
#[cfg_attr(not(any(feature = "cuda", feature = "rocm")), allow(dead_code))]
fn mit_ersatz(karten: &[&dyn Rechner], jobs: &[Job<'_>]) -> (Option<Buendelergebnis>, Vec<(usize, String)>) {
    let mut ausfaelle: Vec<(usize, String)> = Vec::new();
    loop {
        let uebrig: Vec<usize> = (0..karten.len()).filter(|i| !ausfaelle.iter().any(|(a, _)| a == i)).collect();
        if uebrig.is_empty() {
            return (None, ausfaelle);
        }
        let aktiv: Vec<&dyn Rechner> = uebrig.iter().map(|&i| karten[i]).collect();
        match auf_karten(&aktiv, jobs) {
            Ok(aus) => return (Some(aus), ausfaelle),
            Err((Some(i), grund)) => ausfaelle.push((uebrig[i], grund)),
            Err((None, grund)) => {
                eprintln!("⚠️ Grafikkarte: {grund}; diese Buendelung rechnet die CPU");
                return (None, ausfaelle);
            }
        }
    }
}

#[cfg(not(any(feature = "cuda", feature = "rocm")))]
fn rechnen(_: &[Auftrag<'_>]) -> Option<Vec<Vec<Vec<i16>>>> {
    None
}

/// Ist auf dieser Uebersetzung und dieser Maschine mindestens eine Karte
/// bereit, die rechnen darf (Hersteller an, Karte an)?
pub fn verfuegbar() -> bool {
    karten().iter().any(|k| hersteller_an(k.hersteller) && karte_an(&k.pci) && !k.ausgefallen)
}

/// **Hat dieser Hersteller eine Karte, die die Selbstpruefung bestanden
/// hat?** Unabhaengig vom Schalter, wie `metal::verfuegbar` unabhaengig von
/// der Schwelle ist: Gefragt ist, ob es den Rechenweg gibt.
pub fn verfuegbar_fuer(h: Hersteller) -> bool {
    #[cfg(any(feature = "cuda", feature = "rocm"))]
    {
        // ⛔️ Eine nachgebildete Karte zaehlt nie: Sie rechnet auf der CPU.
        uebersetzt_fuer(h) && verbund().karten.iter().any(|k| k.hersteller == h && !k.nachgebildet())
    }
    #[cfg(not(any(feature = "cuda", feature = "rocm")))]
    {
        let _ = h;
        false
    }
}

/// Ist dieser Hersteller in dieser Uebersetzung enthalten?
pub fn uebersetzt_fuer(h: Hersteller) -> bool {
    match h {
        Hersteller::Nvidia => cfg!(feature = "cuda"),
        Hersteller::Amd => cfg!(feature = "rocm"),
    }
}

/// **Warum keine Karte rechnet**, oder `None`, wenn mindestens eine darf.
/// Fuer Meldungen an Menschen.
pub fn nicht_verfuegbar_weil() -> Option<String> {
    if verfuegbar() {
        return None;
    }
    #[cfg(any(feature = "cuda", feature = "rocm"))]
    {
        let v = verbund();
        let mut g = v.gruende.clone();
        for h in [Hersteller::Nvidia, Hersteller::Amd] {
            if verfuegbar_fuer(h) && !hersteller_an(h) {
                g.push(format!("{} ist abgeschaltet", h.rechenweg()));
            }
        }
        for k in &v.karten {
            if !karte_an(&k.pci) {
                g.push(format!("{} Karte {} ({}) ist abgeschaltet", k.hersteller.rechenweg(), k.nummer, k.pci));
            }
            if k.ausgefallen.load(Ordering::Relaxed) {
                g.push(format!("{} Karte {} ({}) ist ausgefallen", k.hersteller.rechenweg(), k.nummer, k.pci));
            }
        }
        Some(if g.is_empty() { "keine Karte gefunden".into() } else { g.join("; ") })
    }
    #[cfg(not(any(feature = "cuda", feature = "rocm")))]
    {
        Some("ohne die Features `cuda` und `rocm` uebersetzt".into())
    }
}

/// Eine Karte, wie sie ein Mensch sehen soll.
#[derive(Debug, Clone)]
pub struct Kartenbeschreibung {
    pub hersteller: Hersteller,
    pub nummer: i32,
    pub name: String,
    /// Die PCI-Adresse, Schluessel fuer [`karte_setzen`].
    pub pci: String,
    pub architektur: String,
    pub gewicht: u32,
    /// Freier und ganzer Speicher beim Aufbau, in Byte.
    pub speicher: (usize, usize),
    /// ⚑ **Wie viele Teile und Zeilen diese Karte gerechnet hat**, seit
    /// der Prozess laeuft, ohne die Selbstpruefung. Der Beleg, dass jede
    /// Karte mitgerechnet hat; der Gesamtzaehler [`gerechnet`] saehe auch
    /// dann gut aus, wenn fuenf von sechs nie etwas bekommen.
    pub teile: usize,
    pub zeilen: usize,
    /// Mitten im Lauf ausgefallen und seitdem draussen.
    pub ausgefallen: bool,
    /// ⚠️ Eine nachgebildete Karte (`MYL_GPU_NACHBAU`): rechnet auf der CPU.
    pub nachgebildet: bool,
}

/// **Die Karten, die rechnen duerfen**, in der Reihenfolge der Verteilung.
pub fn karten() -> Vec<Kartenbeschreibung> {
    #[cfg(any(feature = "cuda", feature = "rocm"))]
    {
        verbund()
            .karten
            .iter()
            .map(|k| Kartenbeschreibung {
                hersteller: k.hersteller,
                nummer: k.nummer,
                name: k.name.clone(),
                pci: k.pci.clone(),
                architektur: k.architektur.clone(),
                gewicht: k.gewicht,
                speicher: k.speicher,
                teile: k.teile.load(Ordering::Relaxed),
                zeilen: k.zeilen.load(Ordering::Relaxed),
                ausgefallen: k.ausgefallen.load(Ordering::Relaxed),
                nachgebildet: k.nachgebildet(),
            })
            .collect()
    }
    #[cfg(not(any(feature = "cuda", feature = "rocm")))]
    {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Die Verteilung: ohne Karte uebersetzt, damit sie mit nachgebildeten Karten
// in jedem Bau geprueft wird.

/// **Was der Verteiler von einer Karte braucht.** Die echte Karte erfuellt
/// es, und in den Pruefungen eine nachgebildete, die auf der CPU rechnet.
trait Rechner: Sync {
    /// Der Anteil an der Arbeit, siehe [`verteiler`].
    fn gewicht(&self) -> u32;
    /// Rechnet die Teile; je Teil `stapel * (bis - von)` Werte, Eingabe
    /// fuer Eingabe.
    fn teile_rechnen(&self, teile: &[verteiler::Teil], jobs: &[Job<'_>]) -> Result<Vec<Vec<i16>>, String>;
}

/// Ein Auftrag, fertig fuer die Karten: die Abstaende ausgerechnet.
#[cfg_attr(not(any(feature = "cuda", feature = "rocm")), allow(dead_code))]
struct Job<'a> {
    xs: &'a [&'a [i16]],
    w: Gewichte<'a>,
    spalten: usize,
    abstaende: Vec<i8>,
}

#[cfg_attr(not(any(feature = "cuda", feature = "rocm")), allow(dead_code))]
impl<'a> Job<'a> {
    /// `None`, wenn die Form nicht passt; dann rechnet die CPU und meldet
    /// den Fehler selbst, mit ihren eigenen Pruefungen.
    fn aus(a: &Auftrag<'a>) -> Option<Self> {
        let zeilen = a.w_shifts.len();
        let spalten = a.in_features;
        if spalten == 0 || spalten > i32::MAX as usize || zeilen > i32::MAX as usize {
            return None;
        }
        let passt = match a.w {
            Gewichte::Int8(w) => w.len() == zeilen * spalten,
            Gewichte::Ternaer { muster, betraege } => {
                spalten.is_multiple_of(crate::ternaer::GRUPPE)
                    && muster.len() == zeilen * spalten / 4
                    && betraege.len() == zeilen * (spalten / crate::ternaer::GRUPPE)
            }
            Gewichte::Vierbit { codes, gitter } => {
                spalten.is_multiple_of(crate::vierbit::GRUPPE)
                    && codes.len() == zeilen * spalten / 2
                    && gitter.len() == zeilen * (spalten / crate::vierbit::GRUPPE) * crate::vierbit::GITTER_JE_GRUPPE
            }
        };
        if !passt || a.xs.iter().any(|x| x.len() != spalten) {
            return None;
        }
        Some(Job { xs: a.xs, w: a.w, spalten, abstaende: crate::ressourcen::metal::abstaende_fuer(a.w_shifts, a.act_frac_bits, a.ziel) })
    }

    fn zeilen(&self) -> usize {
        self.abstaende.len()
    }
}

/// Die Karten, einmal je Prozess gefunden und geprueft.
#[cfg(any(feature = "cuda", feature = "rocm"))]
struct Verbund {
    karten: Vec<treiber::Karte>,
    /// Warum ein Hersteller oder eine Karte fehlt.
    gruende: Vec<String>,
}

#[cfg(any(feature = "cuda", feature = "rocm"))]
fn verbund() -> &'static Verbund {
    static V: std::sync::OnceLock<Verbund> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        let mut karten = Vec::new();
        let mut gruende = Vec::new();
        let hersteller: Vec<Hersteller> = [Hersteller::Nvidia, Hersteller::Amd]
            .into_iter()
            .filter(|&h| uebersetzt_fuer(h))
            .collect();
        for h in hersteller {
            match treiber::karten(h) {
                Ok((gefunden, fehlend)) => {
                    gruende.extend(fehlend.into_iter().inspect(|g| eprintln!("⚠️ Grafikkarte: {g}")));
                    karten.extend(gefunden);
                }
                Err(grund) => gruende.push(format!("{}: {grund}", h.rechenweg())),
            }
        }
        // ⚑ **Nachgebildete Karten**, nur auf ausdrueckliche Bitte: zum
        // Probelauf der Verteilung ohne Karte (`nachbaukarte`). Sie rechnen
        // auf der CPU und zaehlen fuer keinen Rechenweg.
        let nachbau: usize = std::env::var("MYL_GPU_NACHBAU").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        if nachbau > 0 {
            let h = if cfg!(feature = "cuda") { Hersteller::Nvidia } else { Hersteller::Amd };
            eprintln!(
                "⚠️ Grafikkarte: MYL_GPU_NACHBAU={nachbau}: {nachbau} nachgebildete Karten; sie rechnen auf der CPU und zaehlen fuer keinen Rechenweg"
            );
            karten.extend(treiber::nachbaukarten(nachbau, h));
        }
        let auswahl = std::env::var("MYL_GPU_GERAETE").ok().filter(|s| !s.trim().is_empty());
        if let Some(a) = &auswahl {
            karten.retain(|k| gewaehlt(a, k.hersteller, k.nummer));
        }
        let mut geprueft = Vec::new();
        for k in karten {
            let wer = format!("{} Karte {} ({}, {})", k.hersteller.rechenweg(), k.nummer, k.name, k.architektur);
            match pruefung::selbstpruefung(&k) {
                Ok(()) => geprueft.push(k),
                Err(grund) => {
                    eprintln!("⛔️ Grafikkarte: {wer} rechnet anders als die CPU ({grund}); sie rechnet nicht mit");
                    gruende.push(format!("{wer}: Selbstpruefung gescheitert: {grund}"));
                }
            }
        }
        if let Ok(g) = std::env::var("MYL_GPU_GEWICHTE") {
            let zahlen: Vec<u32> = g.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if zahlen.len() == geprueft.len() {
                for (k, z) in geprueft.iter_mut().zip(zahlen) {
                    k.gewicht = z;
                }
            } else if !g.trim().is_empty() {
                eprintln!(
                    "⚠️ Grafikkarte: MYL_GPU_GEWICHTE nennt {} Zahlen fuer {} Karten und gilt nicht",
                    g.split(',').count(),
                    geprueft.len()
                );
            }
        }
        Verbund { karten: geprueft, gruende }
    })
}

/// Ob die Auswahl `MYL_GPU_GERAETE` diese Karte nennt: `2` fuer jede Karte
/// mit Nummer 2, `cuda:2` nur fuer die NVIDIA-Karte 2.
#[cfg_attr(not(any(feature = "cuda", feature = "rocm")), allow(dead_code))]
fn gewaehlt(auswahl: &str, h: Hersteller, nummer: i32) -> bool {
    auswahl.split(',').map(str::trim).any(|e| match e.split_once(':') {
        Some((weg, n)) => weg == h.rechenweg() && n.parse() == Ok(nummer),
        None => e.parse() == Ok(nummer),
    })
}

/// Das Ergebnis eines Buendels: je Auftrag, je Eingabe, je Zeile.
type Buendelergebnis = Vec<Vec<Vec<i16>>>;

/// Was eine Karte zurueckgibt: je Teil die Werte, oder warum nicht.
type Kartenergebnis = Result<Vec<Vec<i16>>, String>;

/// Ein Fehler beim Rechnen: welche Karte (Platz in der uebergebenen Liste),
/// oder keine, wenn die Verteilung selbst nicht aufging.
type Fehlschlag = (Option<usize>, String);

/// **Verteilt die Jobs auf die Karten und rechnet**, jede Karte in ihrem
/// eigenen Faden.
#[cfg_attr(not(any(feature = "cuda", feature = "rocm")), allow(dead_code))]
fn auf_karten(karten: &[&dyn Rechner], jobs: &[Job<'_>]) -> Result<Vec<Vec<Vec<i16>>>, Fehlschlag> {
    let lasten: Vec<verteiler::Last> = jobs
        .iter()
        .map(|j| verteiler::Last { zeilen: j.zeilen(), je_zeile: (j.spalten as u64) * (j.xs.len() as u64) })
        .collect();
    let gewichte: Vec<u32> = karten.iter().map(|k| k.gewicht()).collect();
    let plan = verteiler::verteilen(&lasten, &gewichte);
    let mut aus: Vec<Vec<Vec<i16>>> = jobs.iter().map(|j| vec![vec![0i16; j.zeilen()]; j.xs.len()]).collect();
    let ergebnisse: Vec<(usize, Kartenergebnis)> = std::thread::scope(|s| {
        let faeden: Vec<_> = karten
            .iter()
            .zip(&plan)
            .enumerate()
            .filter(|(_, (_, teile))| !teile.is_empty())
            .map(|(i, (k, teile))| (i, s.spawn(move || k.teile_rechnen(teile, jobs))))
            .collect();
        faeden
            .into_iter()
            .map(|(i, f)| (i, f.join().unwrap_or_else(|_| Err("ein Kartenfaden ist abgestuerzt".into()))))
            .collect()
    });
    let mut gedeckt: Vec<usize> = vec![0; jobs.len()];
    for (i, ergebnis) in ergebnisse {
        let teile = &plan[i];
        let flach = ergebnis.map_err(|g| (Some(i), g))?;
        if flach.len() != teile.len() {
            return Err((Some(i), format!("{} Ergebnisse fuer {} Teile", flach.len(), teile.len())));
        }
        for (t, werte) in teile.iter().zip(flach) {
            let n = t.bis - t.von;
            for (b, zeile) in aus[t.auftrag].iter_mut().enumerate() {
                zeile[t.von..t.bis].copy_from_slice(&werte[b * n..(b + 1) * n]);
            }
            gedeckt[t.auftrag] += n;
        }
    }
    // ⛔️ Jede Zeile genau einmal, sonst stuende eine Null als Ergebnis da.
    for (j, (&n, job)) in gedeckt.iter().zip(jobs).enumerate() {
        if n != job.zeilen() {
            return Err((None, format!("Auftrag {j}: {n} von {} Zeilen gerechnet", job.zeilen())));
        }
    }
    Ok(aus)
}

#[cfg(any(feature = "cuda", feature = "rocm"))]
impl Rechner for treiber::Karte {
    fn gewicht(&self) -> u32 {
        self.gewicht
    }

    fn teile_rechnen(&self, teile: &[verteiler::Teil], jobs: &[Job<'_>]) -> Result<Vec<Vec<i16>>, String> {
        let aus = auf_karte(self, teile, jobs)?;
        self.teile.fetch_add(teile.len(), Ordering::Relaxed);
        self.zeilen.fetch_add(teile.iter().map(|t| t.bis - t.von).sum::<usize>(), Ordering::Relaxed);
        Ok(aus)
    }
}

/// Rundet auf 256 Byte auf; jeder Abschnitt eines Puffers beginnt dort.
#[cfg(any(feature = "cuda", feature = "rocm"))]
fn ausgerichtet(n: usize) -> usize {
    n.div_ceil(256) * 256
}

/// **Rechnet Teile auf einer Karte**: alles hochladen, alle Kerne starten,
/// einmal warten, alles herunterladen. Je Teil `stapel * (bis - von)` Werte,
/// Eingabe fuer Eingabe.
#[cfg(any(feature = "cuda", feature = "rocm"))]
fn auf_karte(k: &treiber::Karte, teile: &[verteiler::Teil], jobs: &[Job<'_>]) -> Result<Vec<Vec<i16>>, String> {
    struct Lage {
        w: usize,
        m: usize,
        x: usize,
        a: usize,
        o: usize,
    }
    let mut lagen = Vec::with_capacity(teile.len());
    let (mut gw, mut gm, mut gx, mut ga, mut go) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for t in teile {
        let j = &jobs[t.auftrag];
        let n = t.bis - t.von;
        lagen.push(Lage { w: gw, m: gm, x: gx, a: ga, o: go });
        match j.w {
            Gewichte::Int8(_) => gw += ausgerichtet(n * j.spalten),
            Gewichte::Ternaer { .. } => {
                gw += ausgerichtet(n * j.spalten / 4);
                gm += ausgerichtet(n * (j.spalten / crate::ternaer::GRUPPE) * 2);
            }
            // Die Gitter liegen im Puffer der Betraege.
            Gewichte::Vierbit { .. } => {
                gw += ausgerichtet(n * j.spalten / 2);
                gm += ausgerichtet(n * (j.spalten / crate::vierbit::GRUPPE) * crate::vierbit::GITTER_JE_GRUPPE);
            }
        }
        gx += ausgerichtet(j.xs.len() * j.spalten * 2);
        ga += ausgerichtet(n);
        go += ausgerichtet(j.xs.len() * n * 2);
    }

    let mut vorrat = k.sperre.lock().unwrap_or_else(|e| e.into_inner());
    k.aktivieren()?;
    let v = &mut *vorrat;
    let pw = k.mindestens(&mut v.gewichte, gw)?;
    let pm = k.mindestens(&mut v.betraege, gm)?;
    let px = k.mindestens(&mut v.eingaben, gx)?;
    let pa = k.mindestens(&mut v.abstaende, ga)?;
    let po = k.mindestens(&mut v.ausgaben, go)?;

    let mut flach: Vec<i16> = Vec::new();
    for (t, lage) in teile.iter().zip(&lagen) {
        let j = &jobs[t.auftrag];
        let (n, s) = (t.bis - t.von, j.spalten);
        match j.w {
            Gewichte::Int8(w) => k.hoch(pw + lage.w as u64, bytes_i8(&w[t.von * s..t.bis * s]))?,
            Gewichte::Ternaer { muster, betraege } => {
                let g = s / crate::ternaer::GRUPPE;
                k.hoch(pw + lage.w as u64, &muster[t.von * s / 4..t.bis * s / 4])?;
                k.hoch(pm + lage.m as u64, bytes_i16(&betraege[t.von * g..t.bis * g]))?;
            }
            Gewichte::Vierbit { codes, gitter } => {
                let g = s / crate::vierbit::GRUPPE * crate::vierbit::GITTER_JE_GRUPPE;
                k.hoch(pw + lage.w as u64, &codes[t.von * s / 2..t.bis * s / 2])?;
                k.hoch(pm + lage.m as u64, &gitter[t.von * g..t.bis * g])?;
            }
        }
        flach.clear();
        for x in j.xs {
            flach.extend_from_slice(x);
        }
        k.hoch(px + lage.x as u64, bytes_i16(&flach))?;
        k.hoch(pa + lage.a as u64, bytes_i8(&j.abstaende[t.von..t.bis]))?;
        let (zeilen, stapel) = (n, j.xs.len());
        match j.w {
            Gewichte::Int8(_) => k.starten(
                treiber::Kern::W8a16,
                &[pw + lage.w as u64, px + lage.x as u64, pa + lage.a as u64, po + lage.o as u64],
                zeilen,
                s,
                stapel,
            )?,
            Gewichte::Ternaer { .. } => k.starten(
                treiber::Kern::Ternaer,
                &[pw + lage.w as u64, pm + lage.m as u64, px + lage.x as u64, pa + lage.a as u64, po + lage.o as u64],
                zeilen,
                s,
                stapel,
            )?,
            Gewichte::Vierbit { .. } => k.starten(
                treiber::Kern::Vierbit,
                &[pw + lage.w as u64, pm + lage.m as u64, px + lage.x as u64, pa + lage.a as u64, po + lage.o as u64],
                zeilen,
                s,
                stapel,
            )?,
        }
    }
    k.warten()?;
    let mut aus = Vec::with_capacity(teile.len());
    for (t, lage) in teile.iter().zip(&lagen) {
        let werte = jobs[t.auftrag].xs.len() * (t.bis - t.von);
        let mut ziel = vec![0i16; werte];
        k.runter(bytes_i16_mut(&mut ziel), po + lage.o as u64)?;
        aus.push(ziel);
    }
    Ok(aus)
}

#[cfg(any(feature = "cuda", feature = "rocm"))]
fn bytes_i8(v: &[i8]) -> &[u8] {
    // SAFETY: `i8` und `u8` haben dieselbe Groesse und Ausrichtung.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len()) }
}

#[cfg(any(feature = "cuda", feature = "rocm"))]
fn bytes_i16(v: &[i16]) -> &[u8] {
    // SAFETY: Jede Bytefolge ist als `u8` gueltig; die Laenge ist die des
    // Ausschnitts in Byte. Die Reihenfolge der Bytes ist die der Maschine,
    // und die Karte liest little-endian wie jede Maschine, auf der es CUDA
    // oder ROCm gibt.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

#[cfg(any(feature = "cuda", feature = "rocm"))]
fn bytes_i16_mut(v: &mut [i16]) -> &mut [u8] {
    // SAFETY: wie `bytes_i16`; jedes Bitmuster ist ein gueltiges `i16`.
    unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr() as *mut u8, std::mem::size_of_val(v)) }
}

#[cfg(test)]
mod proben {
    use super::*;

    /// ⚑ **Die Auswahl versteht beide Formen.**
    #[test]
    fn die_auswahl_nennt_karten() {
        assert!(gewaehlt("0,2", Hersteller::Nvidia, 2));
        assert!(gewaehlt("0, 2", Hersteller::Amd, 0));
        assert!(!gewaehlt("0,2", Hersteller::Nvidia, 1));
        assert!(gewaehlt("cuda:1", Hersteller::Nvidia, 1));
        assert!(!gewaehlt("cuda:1", Hersteller::Amd, 1));
        assert!(gewaehlt("rocm:0,cuda:3", Hersteller::Amd, 0));
    }

    /// ⚑ **Ohne Karte rechnet die CPU**, und die Meldung sagt warum. Auf der
    /// Entwicklungsmaschine gibt es keine Karte.
    #[test]
    fn ohne_karte_kommt_none_und_ein_grund() {
        if verfuegbar() {
            return;
        }
        let w = [1i8; 4];
        let x = [1i16; 2];
        let xs: Vec<&[i16]> = vec![&x; 32];
        assert!(stapel(&xs, &w, 2, &[0, 0], 0, &|_| 0).is_none());
        assert!(!nimmt(1_000));
        let grund = nicht_verfuegbar_weil().expect("ohne Karte ein Grund");
        assert!(!grund.is_empty());
    }

    /// ⛔️ **Auf einer Maschine mit Karte scheitert die Pruefung, wenn eine
    /// Karte die Selbstpruefung nicht bestanden hat.** Im Betrieb faellt
    /// sie nur heraus und die anderen rechnen weiter; eine Pruefung, die
    /// dabei gruen bliebe, liesse genau diesen Fund unbemerkt. Ohne Karte
    /// gibt es nichts zu pruefen, und das steht in der Ausgabe.
    #[cfg(any(feature = "cuda", feature = "rocm"))]
    #[test]
    fn jede_gefundene_karte_besteht_die_selbstpruefung() {
        let v = verbund();
        let gescheitert: Vec<&String> = v.gruende.iter().filter(|g| g.contains("Selbstpruefung")).collect();
        assert!(gescheitert.is_empty(), "{gescheitert:?}");
        if v.karten.is_empty() {
            eprintln!("ohne Karte nichts geprueft: {}", nicht_verfuegbar_weil().unwrap_or_default());
        }
        for k in &v.karten {
            eprintln!("bestanden: {} Karte {} ({}, {})", k.hersteller.rechenweg(), k.nummer, k.name, k.architektur);
        }
    }

    /// ⚑ **Jede Schreibweise einer PCI-Adresse wird dieselbe.**
    #[test]
    fn pci_adressen_werden_gleich_geschrieben() {
        for roh in ["0000:3B:00.0", "00000000:3b:00.0", "3b:00.0", " 0000:3b:00.0\n"] {
            assert_eq!(pci_normal(roh), "0000:3b:00.0", "{roh:?}");
        }
        assert_eq!(pci_normal("0001:C1:1F.7"), "0001:c1:1f.7");
        assert_eq!(pci_normal("Unsinn"), "unsinn");
    }

    /// ⚑ **Der Schalter je Karte merkt sich den Wunsch** in jeder
    /// Schreibweise der Adresse, und an hebt aus wieder auf.
    #[test]
    fn der_schalter_je_karte_kehrt_zurueck() {
        let adresse = "0000:7F:00.0";
        assert!(karte_an(adresse));
        karte_setzen(adresse, false);
        assert!(!karte_an("0000:7f:00.0"));
        karte_setzen("0000:7f:00.0", false);
        karte_setzen(adresse, true);
        assert!(karte_an(adresse), "zweimal aus, einmal an: muss an sein");
    }

    /// ⚑ **Ein Schalter je Hersteller**, an und wieder an.
    #[test]
    fn der_schalter_kehrt_zurueck() {
        for h in [Hersteller::Nvidia, Hersteller::Amd] {
            let vorher = hersteller_an(h);
            hersteller_setzen(h, false);
            assert!(!hersteller_an(h));
            hersteller_setzen(h, vorher);
            assert_eq!(hersteller_an(h), vorher);
        }
    }
}
