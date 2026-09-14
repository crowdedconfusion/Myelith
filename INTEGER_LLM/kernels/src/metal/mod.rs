//! Metal: gebuendelte W8A16-Matrizen auf der GPU von Apple-Silizium.
//!
//! # ⚑ Was hier gerechnet wird, und was nicht
//!
//! **Nur [`crate::linear::linear_w8a16_stapel`] und sein Zwilling mit
//! einer Ausgangsskala je Kanal**, also viele Eingaben auf denselben
//! Gewichten: die sieben Matrizen einer Ebene in der Vorbereitung eines
//! Prompts. Alles andere bleibt auf der CPU, und zwar gemessen:
//!
//! | je Matrix, `down_proj` des 4B | eine Eingabe | 169 Eingaben |
//! |---|---|---|
//! | `cpu-simd` | 0,16 ms | 14,3 ms |
//! | GPU ueber Metal Performance Primitives | 0,21 ms | **1,03 ms** |
//!
//! ⛔️ **Der Decode gewinnt nichts.** Eine Eingabe liest die 25 MB einer
//! Matrix einmal ganz, und das begrenzt die Speicherbandbreite, nicht die
//! Rechenleistung; die GPU liest langsamer als die CPU. Gebuendelt wird
//! dieselbe Matrix fuer alle Eingaben einmal gelesen, daher der Faktor.
//!
//! # ⚑ Die Zerlegung
//!
//! `matmul2d` rechnet `int8 · int8 → int32` und kennt `int8 · int16`
//! nicht. Jede Aktivierung wird deshalb in zwei vorzeichenbehaftete
//! Stellen zerlegt:
//!
//! ```text
//! h = x >> 8,   l = (x & 255) − 128,   also x = 256·h + l + 128
//! W·x = 256·(W·h) + W·l + 128·Zeilensumme(W)
//! ```
//!
//! **Exakt, und int32 genuegt je Teilprodukt:** `|h|, |l| ≤ 128` und
//! `|w| ≤ 128`, also `|W·h| ≤ in_features · 16 384`. Unter `2³¹` bleibt das
//! bis `in_features = 131 071`; darueber rechnet die CPU
//! ([`HOECHSTE_SPALTENZAHL`]). Zusammengesetzt wird in int64, gerundet
//! und geklemmt wie in `fixed_point.rs`.
//!
//! Die Zeilensummen kommen **aus derselben Matrixmultiplikation**: Die
//! letzte Zeile von X besteht aus Einsen. Damit gibt es keinen
//! Zwischenspeicher, der an einer Adresse haengt, und keine zweite
//! Rechnung, die auseinanderlaufen koennte.
//!
//! # ⚠️ Die Rechnung liegt in geschlossenem Systemcode
//!
//! `matmul2d` steht in einer Systembibliothek, deren Umsetzung nicht
//! einsehbar ist. **Die Bitgleichheit ist damit pruefbar, nicht lesbar**,
//! und ein Systemupdate kann sie aendern. Deshalb drei Schranken:
//!
//! 1. **Eine Selbstpruefung in jedem Prozess**, bevor die GPU zum ersten
//!    Mal rechnen darf: Extremwerte beider Stellen, Kachelraender, alle
//!    Faelle der Umskalierung, verglichen mit der CPU. Scheitert sie,
//!    rechnet der ganze Prozess auf der CPU und sagt es.
//! 2. **Der Konformitaetslauf mit erzwungener GPU** (`run.sh metal`), der
//!    abgelehnt wird, wenn die GPU dabei nicht gerechnet hat
//!    ([`gerechnet`]).
//! 3. **Die Pruefungen in diesem Modul**, ueber ungerade Formen und den
//!    vollen Wertebereich.
//!
//! # Die Stellschrauben
//!
//! - `MYL_METAL_AB`: ab wie vielen Eingaben die GPU rechnet. `0` schaltet
//!   sie ab, `1` erzwingt sie fuer jede Buendelung (Konformitaetslauf).
//!   Ohne Angabe gilt [`VORGABE_AB`]. Zur Laufzeit: [`schwelle_setzen`].

#[cfg(all(feature = "metal", target_os = "macos"))]
mod geraet;

/// Ab wie vielen Eingaben die GPU rechnet, wenn nichts anderes gesetzt ist.
///
/// # 📌 Hier stand bis zum 2026-09-14 Mittag eine Vier, und sie war geraten
///
/// Beim 30B-Gemisch rechnete ein Prompt von sieben Token mit `metal` 2,0 s
/// statt 1,8 s: Die GPU kostet je Aufruf 0,2 bis 0,3 ms fest, und bei
/// wenigen Eingaben rechnet die CPU die Matrix schneller. Gemessen am
/// selben Tag, CPU gegen GPU in ms (`linear_w8a16_stapel`, M5 Pro):
///
/// | Form | 4 | 8 | 16 | 64 |
/// |---|---|---|---|---|
/// | 30B q, 4 096 x 2 048 | 0,20 / 0,30 | 0,32 / 0,32 | 0,67 / 0,37 | 2,56 / 1,01 |
/// | 4B down, 2 560 x 9 728 | 0,36 / 0,61 | 0,62 / 0,67 | 1,23 / 0,96 | 5,64 / 2,65 |
/// | Experte, 768 x 2 048 | 0,10 / 0,22 | 0,13 / 0,23 | 0,26 / 0,24 | 1,03 / 0,27 |
/// | Router, 128 x 2 048 | 0,04 / 0,20 | 0,07 / 0,20 | 0,09 / 0,21 | 0,44 / 0,22 |
///
/// ⚑ **Ab sechzehn ist die GPU nirgends nennenswert langsamer** und bei
/// den grossen Formen schon schneller; darunter ist sie ueberall
/// langsamer. Der Router bleibt bei sechzehn um 0,1 ms zurueck, das ist
/// der Preis einer einzigen Zahl fuer alle Formen.
pub const VORGABE_AB: usize = 16;

/// Wie viele Buendelungen die GPU in diesem Prozess gerechnet hat.
static GERECHNET: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// **Hat die GPU ueberhaupt gerechnet?**
///
/// 📌 **Die Lehre aus Fund 33:** Ein Konformitaetslauf, der unter dem
/// Namen eines Backends die Referenz rechnet, sieht genau aus wie ein
/// bestandener. Der Pruefstand liest diesen Zaehler und lehnt einen
/// Metal-Lauf ab, in dem er bei null steht.
pub fn gerechnet() -> usize {
    GERECHNET.load(std::sync::atomic::Ordering::Relaxed)
}

/// Bis zu welcher Spaltenzahl int32 je Teilprodukt sicher genuegt:
/// `131 071 · 16 384 < 2³¹`.
pub const HOECHSTE_SPALTENZAHL: usize = 131_071;

/// Noch nicht gelesen.
const UNGELESEN: usize = usize::MAX;

static AB: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(UNGELESEN);

/// **Ab wie vielen Eingaben die GPU rechnet.** `0` heisst: nie.
///
/// Beim ersten Lesen aus `MYL_METAL_AB`, sonst [`VORGABE_AB`]; danach aus
/// [`schwelle_setzen`].
pub fn schwelle() -> usize {
    use std::sync::atomic::Ordering;
    let wert = AB.load(Ordering::Relaxed);
    if wert != UNGELESEN {
        return wert;
    }
    let aus_umgebung = std::env::var("MYL_METAL_AB")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n: &usize| n != UNGELESEN)
        .unwrap_or(VORGABE_AB);
    // Wer zuerst setzt, gewinnt; ein gleichzeitiges `schwelle_setzen` geht
    // dem Umgebungswert vor.
    let _ = AB.compare_exchange(UNGELESEN, aus_umgebung, Ordering::Relaxed, Ordering::Relaxed);
    AB.load(Ordering::Relaxed)
}

/// **Setzt die Schwelle fuer den ganzen Prozess**: `0` schaltet die GPU ab,
/// `1` laesst sie jede Buendelung rechnen.
///
/// ⚑ Fuer den Testclient, der alle Rechenwege einer Maschine in einem
/// Lauf vergleicht (2026-09-14). **Keine Zahl haengt daran**, nur wo
/// gerechnet wird.
pub fn schwelle_setzen(n: usize) {
    AB.store(n.min(UNGELESEN - 1), std::sync::atomic::Ordering::Relaxed);
}

/// Rechnet diese Buendelung auf der GPU, wenn sie sich lohnt und eine
/// GPU da ist. `None` heisst: Die CPU rechnet, und das Ergebnis ist
/// dasselbe.
///
/// `ziel(z)` ist die Ausgangsskala der Zeile `z`, wie in
/// [`crate::linear::linear_w8a16_pc_stapel`].
pub fn stapel(
    xs: &[&[i16]],
    w: &[i8],
    in_features: usize,
    w_shifts: &[u8],
    act_frac_bits: u8,
    ziel: &dyn Fn(usize) -> u8,
) -> Option<Vec<Vec<i16>>> {
    let ab = schwelle();
    if ab == 0 || xs.len() < ab || in_features == 0 || in_features > HOECHSTE_SPALTENZAHL {
        return None;
    }
    rechnen(xs, w, in_features, &abstaende_fuer(w_shifts, act_frac_bits, ziel))
}

/// Ein Stapel fuer [`stapel_viele`], mit denselben Angaben wie [`stapel`].
pub struct Auftrag<'a> {
    pub xs: &'a [&'a [i16]],
    pub w: &'a [i8],
    pub in_features: usize,
    pub w_shifts: &'a [u8],
    pub act_frac_bits: u8,
    pub ziel: &'a (dyn Fn(usize) -> u8 + Sync),
}

/// **Viele Stapel in einem Befehlspuffer**, oder `None`, wenn die CPU
/// rechnet.
///
/// ⚑ Die Schwelle gilt fuer die **Summe** der Eingaben: Die festen Kosten
/// hat der Befehlspuffer, nicht der einzelne Auftrag.
pub fn stapel_viele(auftraege: &[Auftrag<'_>]) -> Option<Vec<Vec<Vec<i16>>>> {
    let ab = schwelle();
    let eingaben: usize = auftraege.iter().map(|a| a.xs.len()).sum();
    if ab == 0
        || eingaben < ab
        || auftraege.iter().any(|a| a.in_features == 0 || a.in_features > HOECHSTE_SPALTENZAHL)
    {
        return None;
    }
    let abstaende: Vec<Vec<i8>> = auftraege
        .iter()
        .map(|a| abstaende_fuer(a.w_shifts, a.act_frac_bits, a.ziel))
        .collect();
    rechnen_viele(auftraege, &abstaende)
}

#[cfg(all(feature = "metal", target_os = "macos"))]
fn rechnen_viele(auftraege: &[Auftrag<'_>], abstaende: &[Vec<i8>]) -> Option<Vec<Vec<Vec<i16>>>> {
    let jobs: Vec<geraet::Job<'_>> = auftraege
        .iter()
        .zip(abstaende)
        .map(|(a, ab)| geraet::Job { xs: a.xs, w: a.w, spalten: a.in_features, abstaende: ab })
        .collect();
    geraet::stapel_viele(&jobs)
}

#[cfg(not(all(feature = "metal", target_os = "macos")))]
fn rechnen_viele(_: &[Auftrag<'_>], _: &[Vec<i8>]) -> Option<Vec<Vec<Vec<i16>>>> {
    None
}

/// ⚑ **Die Abstaende auf der CPU, mit derselben Arithmetik wie
/// `rescale_i64`**: `w_shifts[z] + act_frac_bits` als u8, der Abstand als
/// Differenz. Die Zusicherungen aus `fixed_point.rs` gelten hier genauso,
/// damit ein Fehler auf beiden Wegen gleich auffaellt.
fn abstaende_fuer(w_shifts: &[u8], act_frac_bits: u8, ziel: &dyn Fn(usize) -> u8) -> Vec<i8> {
    (0..w_shifts.len())
        .map(|z| {
            let ein = w_shifts[z] + act_frac_bits;
            let aus = ziel(z);
            debug_assert!(
                ein <= 127 && aus <= 127,
                "rescale_i64: in_frac {} / out_frac {} ab 128 dreht die Differenz das Vorzeichen (Fund 75)",
                ein,
                aus
            );
            let abstand = ein as i16 - aus as i16;
            debug_assert!(
                (-63..=62).contains(&abstand),
                "rescale_i64: Abstand {} ausserhalb -63..=62 (Fund 75)",
                abstand
            );
            ein as i8 - aus as i8
        })
        .collect()
}

#[cfg(all(feature = "metal", target_os = "macos"))]
fn rechnen(xs: &[&[i16]], w: &[i8], in_features: usize, abstaende: &[i8]) -> Option<Vec<Vec<i16>>> {
    geraet::stapel(xs, w, in_features, abstaende)
}

#[cfg(not(all(feature = "metal", target_os = "macos")))]
fn rechnen(_: &[&[i16]], _: &[i8], _: usize, _: &[i8]) -> Option<Vec<Vec<i16>>> {
    None
}

/// **Warum die GPU nicht rechnet**, oder `None`, wenn sie es darf.
///
/// Fuer Meldungen an Menschen: Ein Testlauf, der einen Rechenweg
/// auslaesst, sagt, warum.
pub fn nicht_verfuegbar_weil() -> Option<String> {
    #[cfg(all(feature = "metal", target_os = "macos"))]
    {
        geraet::grund()
    }
    #[cfg(not(all(feature = "metal", target_os = "macos")))]
    {
        if cfg!(feature = "metal") {
            Some("das Feature `metal` wirkt nur auf macOS".into())
        } else {
            Some("ohne das Feature `metal` uebersetzt".into())
        }
    }
}

/// Ist auf dieser Uebersetzung und dieser Maschine eine GPU nutzbar, deren
/// Shader sich uebersetzen liessen?
pub fn verfuegbar() -> bool {
    #[cfg(all(feature = "metal", target_os = "macos"))]
    {
        geraet::verfuegbar()
    }
    #[cfg(not(all(feature = "metal", target_os = "macos")))]
    {
        false
    }
}

/// Zerlegt eine Aktivierung in ihre beiden Stellen, siehe Modulkopf.
#[inline]
pub fn zerlegen(x: i16) -> (i8, i8) {
    let hoch = (x >> 8) as i8;
    let niedrig = ((x & 255) - 128) as i8;
    (hoch, niedrig)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Ueberspringen nur ohne GPU.** Eine GPU, deren Aufbau oder
    /// Selbstpruefung scheitert, laesst die Pruefung scheitern; sonst
    /// saehe eine GPU, die anders rechnet, aus wie eine Maschine ohne.
    #[cfg(all(feature = "metal", target_os = "macos"))]
    fn gpu_bereit() -> bool {
        match geraet::bereit() {
            Ok(()) => true,
            Err(geraet::Ohne::KeinGeraet) => {
                eprintln!("keine Metal-GPU auf dieser Maschine, nichts zu vergleichen");
                false
            }
            Err(anders) => panic!("Metal ist nicht bereit: {anders}"),
        }
    }

    #[cfg(all(feature = "metal", target_os = "macos"))]
    fn xorshift(saat: u64) -> impl FnMut() -> u64 {
        let mut s = saat | 1;
        move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        }
    }

    /// **Die GPU rechnet Bit fuer Bit wie die CPU**, ueber Formen, die das
    /// Kachelmass an jeder Kante verfehlen, und ueber den vollen
    /// Wertebereich beider Seiten.
    ///
    /// Ohne GPU (andere Plattform, Feature aus, virtuelle Maschine ohne
    /// Beschleuniger) gibt es nichts zu vergleichen; die Pruefung sagt das
    /// und endet.
    #[cfg(all(feature = "metal", target_os = "macos"))]
    #[test]
    fn die_gpu_rechnet_wie_die_cpu() {
        if !gpu_bereit() {
            return;
        }
        let mut zufall = xorshift(0x5eed);
        let formen = [
            (1usize, 1usize, 1usize),
            (1, 7, 3),
            (63, 64, 15),
            (64, 1, 16),
            (65, 33, 17),
            (129, 250, 40),
            (2560, 9728, 5),
            (512, 1024, 600),
        ];
        for (zeilen, spalten, stapel) in formen {
            let w: Vec<i8> = (0..zeilen * spalten).map(|_| zufall() as i8).collect();
            let eingaben: Vec<Vec<i16>> =
                (0..stapel).map(|_| (0..spalten).map(|_| zufall() as i16).collect()).collect();
            let xs: Vec<&[i16]> = eingaben.iter().map(|e| e.as_slice()).collect();
            let w_shifts: Vec<u8> = (0..zeilen).map(|_| (zufall() % 8) as u8).collect();
            let ziele: Vec<u8> = (0..zeilen).map(|_| (zufall() % 24) as u8).collect();
            let act = 9u8;
            let abstaende: Vec<i8> =
                (0..zeilen).map(|z| (w_shifts[z] + act) as i8 - ziele[z] as i8).collect();

            let gpu = geraet::direkt(&xs, &w, spalten, &abstaende)
                .expect("verfuegbar, also ein Kontext")
                .expect("die GPU hat gerechnet");
            let cpu = crate::linear::linear_w8a16_pc_stapel_cpu(&xs, &w, spalten, &w_shifts, act, &ziele);
            assert_eq!(gpu, cpu, "{zeilen} x {spalten} bei {stapel} Eingaben");
        }
    }

    /// **Die Weiche in `linear.rs` fuehrt wirklich auf die GPU**, und das
    /// Ergebnis ist dasselbe wie auf der CPU.
    /// Haelt die Pruefungen auseinander, die an der prozessweiten
    /// Schwelle drehen oder von ihr abhaengen.
    #[cfg(all(feature = "metal", target_os = "macos"))]
    static SCHWELLE_IN_BENUTZUNG: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// **Die Schwelle laesst sich zur Laufzeit setzen**: `0` schaltet ab,
    /// `1` rechnet schon eine einzige Eingabe auf der GPU.
    #[cfg(all(feature = "metal", target_os = "macos"))]
    #[test]
    fn die_schwelle_laesst_sich_zur_laufzeit_setzen() {
        let _allein = SCHWELLE_IN_BENUTZUNG.lock().unwrap_or_else(|e| e.into_inner());
        if !gpu_bereit() {
            return;
        }
        let vorher = schwelle();
        let w = vec![3i8; 16 * 40];
        let x = vec![1000i16; 40];
        let xs = [x.as_slice()];
        let shifts = vec![2u8; 16];

        schwelle_setzen(0);
        assert!(stapel(&xs, &w, 40, &shifts, 6, &|_| 5).is_none(), "abgeschaltet und doch gerechnet");

        schwelle_setzen(1);
        let gezaehlt = gerechnet();
        let gpu = stapel(&xs, &w, 40, &shifts, 6, &|_| 5).expect("bei Schwelle 1 rechnet die GPU");
        assert!(gerechnet() > gezaehlt);
        let cpu = crate::linear::linear_w8a16_pc_stapel_cpu(&xs, &w, 40, &shifts, 6, &[5u8; 16]);
        assert_eq!(gpu, cpu);

        schwelle_setzen(vorher);
        assert_eq!(schwelle(), vorher);
    }

    /// **Viele Auftraege in einem Befehlspuffer rechnen wie die CPU**, ueber
    /// die oeffentliche Weiche in `linear.rs`, mit erzwungener GPU.
    #[cfg(all(feature = "metal", target_os = "macos"))]
    #[test]
    fn viele_auftraege_rechnen_wie_die_cpu() {
        use crate::linear::{linear_w8a16_stapel_viele, linear_w8a16_stapel_viele_cpu, Ausgangsskala, Stapelauftrag};
        let _allein = SCHWELLE_IN_BENUTZUNG.lock().unwrap_or_else(|e| e.into_inner());
        if !gpu_bereit() {
            return;
        }
        let mut zufall = xorshift(0x5eed_0371);
        let formen = [(768usize, 2048usize, 23usize), (2048, 768, 5), (65, 17, 1), (130, 1003, 40), (9, 9, 0)];
        let w: Vec<Vec<i8>> = formen.iter().map(|(z, s, _)| (0..z * s).map(|_| zufall() as i8).collect()).collect();
        let e: Vec<Vec<Vec<i16>>> = formen
            .iter()
            .map(|(_, s, b)| (0..*b).map(|_| (0..*s).map(|_| zufall() as i16).collect()).collect())
            .collect();
        let xs: Vec<Vec<&[i16]>> = e.iter().map(|v| v.iter().map(|x| x.as_slice()).collect()).collect();
        let shifts: Vec<Vec<u8>> = formen.iter().map(|(z, _, _)| (0..*z).map(|_| (zufall() % 8) as u8).collect()).collect();
        let ziele: Vec<Vec<u8>> = formen.iter().map(|(z, _, _)| (0..*z).map(|_| (zufall() % 20) as u8).collect()).collect();
        let auftraege: Vec<Stapelauftrag<'_>> = (0..formen.len())
            .map(|i| Stapelauftrag {
                xs: &xs[i],
                w: &w[i],
                in_features: formen[i].1,
                w_shifts: &shifts[i],
                act_frac_bits: 9,
                aus: if i % 2 == 0 { Ausgangsskala::JeZeile(&ziele[i]) } else { Ausgangsskala::Eine(12) },
            })
            .collect();
        let vorher_schwelle = schwelle();
        schwelle_setzen(1);
        let gezaehlt = gerechnet();
        let gpu = linear_w8a16_stapel_viele(&auftraege);
        let hat_gerechnet = gerechnet() >= gezaehlt + formen.len();
        schwelle_setzen(vorher_schwelle);
        assert!(hat_gerechnet, "die GPU hat die Auftraege nicht gerechnet");
        assert_eq!(gpu, linear_w8a16_stapel_viele_cpu(&auftraege));
    }

    #[cfg(all(feature = "metal", target_os = "macos"))]
    #[test]
    fn die_weiche_in_linear_nimmt_die_gpu() {
        let _allein = SCHWELLE_IN_BENUTZUNG.lock().unwrap_or_else(|e| e.into_inner());
        if !gpu_bereit() || schwelle() == 0 {
            return;
        }
        let mut zufall = xorshift(0xabcd);
        let (zeilen, spalten, stapel) = (96usize, 200usize, schwelle().max(1) + 3);
        let w: Vec<i8> = (0..zeilen * spalten).map(|_| zufall() as i8).collect();
        let eingaben: Vec<Vec<i16>> =
            (0..stapel).map(|_| (0..spalten).map(|_| zufall() as i16).collect()).collect();
        let xs: Vec<&[i16]> = eingaben.iter().map(|e| e.as_slice()).collect();
        let w_shifts = vec![6u8; zeilen];
        let ziele: Vec<u8> = (0..zeilen).map(|z| 4 + (z % 5) as u8).collect();

        let vorher = gerechnet();
        let skalar = crate::linear::linear_w8a16_stapel(&xs, &w, spalten, &w_shifts, 8, 5);
        let je_kanal = crate::linear::linear_w8a16_pc_stapel(&xs, &w, spalten, &w_shifts, 8, &ziele);
        assert!(gerechnet() >= vorher + 2, "die Weiche hat die GPU nicht genommen");

        let cpu_skalar = crate::linear::linear_w8a16_pc_stapel_cpu(&xs, &w, spalten, &w_shifts, 8, &vec![5u8; zeilen]);
        let cpu_je_kanal = crate::linear::linear_w8a16_pc_stapel_cpu(&xs, &w, spalten, &w_shifts, 8, &ziele);
        assert_eq!(skalar, cpu_skalar);
        assert_eq!(je_kanal, cpu_je_kanal);
    }

    /// **Die Zerlegung ist exakt, fuer jeden der 65 536 Werte.**
    #[test]
    fn die_zerlegung_trifft_jeden_wert() {
        for x in i16::MIN..=i16::MAX {
            let (h, l) = zerlegen(x);
            assert_eq!(256 * h as i32 + l as i32 + 128, x as i32, "x = {x}");
        }
    }

    /// **Ohne GPU nennt das Modul einen Grund**, mit GPU keinen.
    #[test]
    fn ohne_gpu_gibt_es_einen_grund() {
        assert_eq!(verfuegbar(), nicht_verfuegbar_weil().is_none());
    }

    /// **Die Grenze fuer int32 haelt an der schaerfsten Stelle.**
    #[test]
    fn int32_genuegt_bis_zur_hoechsten_spaltenzahl() {
        let schlimmstes = HOECHSTE_SPALTENZAHL as i64 * 128 * 128;
        assert!(schlimmstes <= i32::MAX as i64, "{schlimmstes}");
        assert!((HOECHSTE_SPALTENZAHL as i64 + 1) * 128 * 128 > i32::MAX as i64);
    }
}
