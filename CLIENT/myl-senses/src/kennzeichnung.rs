//! **Synthetische Ausgaben kennzeichnen**: maschinenlesbar und im Signal.
//!
//! # ⚑ Warum es das gibt
//!
//! Artikel 50 Absatz 2 der Verordnung (EU) 2024/1689 verlangt, dass
//! synthetisch erzeugte Audio-, Bild-, Video- und Textinhalte in einem
//! maschinenlesbaren Format gekennzeichnet und als kuenstlich erzeugt
//! erkennbar sind, wirksam und robust, soweit technisch machbar. Myelith
//! erzeugt synthetische **Sprache**; Bilder und Videos erzeugt es nicht.
//!
//! # ⚑ Eine Stelle fuer alles, was hinausgeht
//!
//! [`synthetisch_kennzeichnen`] ist der Pflichtweg fuer jede erzeugte
//! Tondatei, bevor sie abgespielt, abgelegt oder weitergegeben wird. Der
//! Vorleser und der Dauerlaeufer rufen sie an genau den Stellen, an denen
//! eine Datei entsteht; eine zweite Stelle, die selbst kennzeichnet, gaebe
//! es nur, bis sie vergessen wird.
//!
//! # ⚑ Zwei Marken, weil jede anders verloren geht
//!
//! 1. **Metadaten**: ein XMP-Block mit dem IPTC-Quellentyp
//!    `trainedAlgorithmicMedia` (dasselbe Vokabular benutzt C2PA) und ein
//!    RIFF-INFO-Kommentar. Maschinenlesbar und genormt, aber weg, sobald
//!    jemand die Datei neu kodiert.
//! 2. **Wasserzeichen im Signal**: eine feste Pseudozufallsfolge aus plus
//!    und minus eins, alle [`PERIODE`] Proben wiederholt, je Fenster von
//!    20 ms durch die Huellkurve der Stimme geformt ([`LPC_GAMMA`]) und mit
//!    einem Vierundsechzigstel der oertlichen Lautstaerke dazugegeben, so
//!    dass es unter der Stimme bleibt. Erkannt wird es durch Korrelation
//!    nach der umgekehrten Vorhersage; es uebersteht Kopieren,
//!    Pegelaenderungen, Abschneiden am Anfang und MP3 mit 128 kbit/s.
//!
//! ⚠️ **Die Folge ist oeffentlich**, denn der Quelltext ist es. Wer sie
//! kennt, kann sie abziehen. Das Wasserzeichen erschwert das versehentliche
//! Verlieren der Kennzeichnung, es verhindert kein absichtliches Entfernen.
//!
//! ⚑ **Ganzzahlig**, wie alles in diesem Projekt: Gleitkommaproben werden
//! als Bitmuster gelesen und in 16 Bit umgerechnet, ohne dass eine
//! Gleitkommazahl entsteht.

use std::path::Path;

mod c2pa;
pub use c2pa::Herkunft;

/// Was fuer ein Inhalt gekennzeichnet wird.
///
/// ⚑ **Nur, was Myelith erzeugt.** Eine Modalitaet fuer Bilder gaebe es
/// erst, wenn Myelith Bilder erzeugt; dann zwingt die vollstaendige
/// Unterscheidung in [`synthetisch_kennzeichnen`], auch sie zu behandeln.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modalitaet {
    /// Synthetische Sprache als WAV.
    Sprache,
}

/// Wie lang die Wasserzeichenfolge ist, bevor sie sich wiederholt.
pub const PERIODE: usize = 4096;

/// Die Saat der Folge: `MYELITH` und ein Nullbyte.
pub const SAAT: u64 = 0x4D59_454C_4954_4800;

/// Das Verhaeltnis von Lautstaerke zu Wasserzeichen: ein Achtzigstel, rund
/// 38 dB darunter, und das Wasserzeichen folgt der Stimme ([`LPC_GAMMA`]).
///
/// ⚑ **Ein Achtzigstel statt eines Vierundsechzigstels** (Wunsch des
/// Projektinhabers: „etwas leiser bitte aber weiterhin gut auslesbar“, nach
/// einem Hoertest, in dem er die rohe Fassung fuenfmal von fuenf vorzog): das
/// leiseste, bei dem Saetze abgeschnitten und nach MP3 das Anderthalbfache der
/// Schwelle halten (10,0 und 10,1); hoerbar unter 20 dB 0,39 statt 0,86
/// Prozent, fuenftes Perzentil in jedem Band mindestens 21,4 dB. Stuecke am
/// Anfang 6,3. ⚠️ Ein Stueck, das vorne abgeschnitten wird, liegt mit 5,5
/// unter der gesuchten Schwelle; abgelegt wird der ganze Satz.
///
/// ⚑ **Gemessen an echter Sprache** (2026-11-11, 15 Saetze aus Qwen3-TTS mit
/// geklonter Stimme, 2,6 bis 4,6 s, dazu je das erste Stueck von 0,96 s),
/// je Sprachfenster von 20 ms und Band: in 0,86 Prozent der Fenster und
/// Baender liegt das Wasserzeichen weniger als 20 dB unter der Stimme und
/// ist zugleich ueber 2 Stufen laut; das fuenfte Perzentil des Abstands ist
/// in jedem Band mindestens 19,6 dB. Erkannt ([`BEGRENZEN`]) ohne
/// Kennzeichnung hoechstens 4,0 Standardabweichungen (gesucht);
/// gekennzeichnet Saetze 13,5, vorne abgeschnitten und halb so laut 12,4,
/// nach MP3 mit 128 kbit/s 12,3; Stuecke 7,9, 7,1 und 7,0. (Mit einem
/// Vierundsechzigstel; zum jetzigen Teiler siehe oben.)
///
/// 📌 Der Weg dahin, unter demselben Mass gemessen: weiss mit einem
/// Vierundsechzigstel (bis 2026-11-11) in 46 Prozent hoerbar unter 20 dB,
/// ueber 8 kHz im fuenften Perzentil 6 dB **ueber** der Stimme
/// (Projektinhaber: „Rauschen und Artefakte im Hintergrund“); fest
/// binomisch geformt mit einem Zweiunddreissigstel (ein paar Stunden am
/// 2026-11-11) 34 Prozent, 2 bis 4 kHz nur 6,8 dB. Ein Mittel ueber ganze
/// Dateien hatte die feste Formung gut aussehen lassen.
pub const TEILER: i64 = 80;

/// **Wie eng das Wasserzeichen der Stimme folgt**, in Tausendsteln: Je
/// Fenster wird die Folge durch `1/A(z/γ)` der Stimme gefiltert, die
/// Rauschformung der CELP-Sprachcodecs, die das Rauschen unter die
/// Formanten legt. ⚑ 0,97 mit einem Rauschboden der Vorhersage von 2⁻¹⁶
/// ([`LPC_BODEN`]); 0,92 mit 2⁻⁹ bildete die steil fallenden Hoehen von
/// Qwen3-TTS nicht ab (8 bis 12 kHz im fuenften Perzentil 5,8 dB).
///
/// ⚠️ **CAM++ misst jedes Wasserzeichen**, auch ein unhoerbares (weiss mit
/// einem Hundertachtundzwanzigstel kostete 0,021 Aehnlichkeit): In den
/// Pausen nahezu digitaler Stille aendern schon zwei Stufen dessen
/// logarithmische Baender. Fuer die Hoerbarkeit zaehlen die Abstaende je
/// Fenster und Band.
pub const LPC_GAMMA: i64 = 970;

/// Der Rauschboden der Vorhersage: `2^-LPC_BODEN` der Energie.
pub const LPC_BODEN: u32 = 16;

/// **Die Erkennung begrenzt den Rest** je Fenster auf das Zweifache seines
/// Medianbetrags, bevor sie ihn auf gleiche Lautstaerke bringt und faltet.
/// ⚑ Nach der umgekehrten Vorhersage bleiben Stimmbandimpulse und Spruenge
/// als grosse Spitzen, die das Wasserzeichen ueberdecken: ein Saegezahn von
/// 0,2 s kam ohne Begrenzung auf 2,6 Standardabweichungen, mit 9,7; die
/// Stuecke echter Sprache von 6,4 auf 7,9 (abgeschnitten 7,1, nach MP3 7,0),
/// ohne Wasserzeichen hoechstens 4,0. Das Drei- und Fuenffache halfen
/// weniger.
pub const BEGRENZEN: i64 = 2;

/// Ab welchem Wert die Korrelation als erkannt gilt, in Hundertsteln der
/// Standardabweichung, wenn die Datei am Anfang beginnt.
///
/// ⚑ Vier Standardabweichungen: Ohne Wasserzeichen liegt der Wert dort
/// seltener als einmal in dreissigtausend Dateien.
pub const SCHWELLE_ANFANG: i64 = 400;

/// Dasselbe, wenn die Folge an jeder Stelle gesucht wird, weil die Datei
/// vorne abgeschnitten sein kann. Hoeher, weil unter 4096 Versuchen einer
/// zufaellig gross ausfaellt.
pub const SCHWELLE_GESUCHT: i64 = 650;

/// Der Quellentyp nach IPTC, der eine Datei als KI-erzeugt kennzeichnet.
pub const QUELLENTYP: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia";

/// Der Kommentar im RIFF-INFO-Block.
const KOMMENTAR: &str = "KI-generierte Sprache (Myelith) / AI-generated speech (Myelith)";

/// **Kennzeichnet eine erzeugte Datei**, an Ort und Stelle.
///
/// Eine schon gekennzeichnete Datei bleibt, wie sie ist; zweimal
/// gekennzeichnet hiesse zweimal Wasserzeichen und doppelte Stoerung.
pub fn synthetisch_kennzeichnen(pfad: &Path, art: Modalitaet) -> Result<(), String> {
    match art {
        Modalitaet::Sprache => sprache_kennzeichnen(pfad),
    }
}

fn sprache_kennzeichnen(pfad: &Path) -> Result<(), String> {
    let roh = std::fs::read(pfad).map_err(|f| format!("{}: {f}", pfad.display()))?;
    let wav = Wav::lesen(&roh).map_err(|f| format!("{}: {f}", pfad.display()))?;
    if wav.xmp.as_deref().is_some_and(|x| x.contains(QUELLENTYP)) {
        return Ok(());
    }
    let mut proben = wav.proben;
    // ⚑ Erst die Pausen beruhigen, dann einpraegen: Das Wasserzeichen
    //   richtet sich nach der Lautstaerke, die bleibt.
    pausen_beruhigen(&mut proben, wav.kanaele as usize, wav.rate);
    einpraegen(&mut proben, wav.kanaele as usize);
    // ⚑ Zuletzt signiert (C2PA), damit der Hash den fertigen Ton abdeckt.
    let signiert = c2pa::signieren(schreiben(wav.rate, wav.kanaele, &proben))?;
    std::fs::write(pfad, signiert).map_err(|f| format!("{}: {f}", pfad.display()))
}

/// Ab dieser Lautstaerke (RMS in Stufen von 32767, rund −42 dBFS) bleibt ein
/// Fenster, wie es ist.
pub const RUHE_AB: i64 = 260;

/// **Beruhigt die Pausen**: ein sanfter Abwaertsexpander gegen Rauschen und
/// Artefakte des Sprechmodells in leisen Stellen.
///
/// ⚑ Je 10 ms die Lautstaerke, davon das Lauteste aus ±20 ms (damit leise
/// Anlaute und Wortenden nicht beschnitten werden). Darunter wird im
/// Verhaeltnis 1:3 abgesenkt, hoechstens um 12 dB (−4 dB bei −44 dBFS,
/// −12 dB ab −48 dBFS); geoeffnet wird sofort, geschlossen ueber 120 ms, und
/// zwischen den Fenstern gleitet die Verstaerkung. Ganzzahlig, in 1024steln.
/// 📌 Anlass: In den Pausen lag das Rauschen je nach Satz zwischen −77 und
/// −44 dBFS; die Stimmprobe selbst lag bei −75 (2026-11-11).
fn pausen_beruhigen(proben: &mut [i16], kanaele: usize, rate: u32) {
    let kanaele = kanaele.max(1);
    let fenster = (rate as usize / 100).max(1);
    let rahmen = proben.len() / kanaele;
    let n = rahmen.div_ceil(fenster);
    if n == 0 {
        return;
    }
    let pegel: Vec<i64> = (0..n)
        .map(|i| {
            let (von, bis) = (i * fenster * kanaele, ((i + 1) * fenster * kanaele).min(proben.len()));
            let summe: u128 = proben[von..bis].iter().map(|&x| (x as i128 * x as i128) as u128).sum();
            wurzel(summe / ((bis - von) as u128).max(1)) as i64
        })
        .collect();
    let mut ziel = Vec::with_capacity(n);
    for i in 0..n {
        let e = pegel[i.saturating_sub(2)..(i + 3).min(n)].iter().copied().max().unwrap_or(0);
        ziel.push(if e >= RUHE_AB { 1024 } else { (1024 * e * e / (RUHE_AB * RUHE_AB)).max(256) });
    }
    let mut g = vec![0i64; n];
    for i in 0..n {
        let vorher = if i == 0 { ziel[0] } else { g[i - 1] };
        g[i] = if ziel[i] >= vorher { ziel[i] } else { ziel[i].max(vorher - 64) };
    }
    for r in 0..rahmen {
        let (i, j) = (r / fenster, (r % fenster) as i64);
        let vorher = if i == 0 { g[0] } else { g[i - 1] };
        let v = vorher + (g[i] - vorher) * j / fenster as i64;
        if v >= 1024 {
            continue;
        }
        for k in 0..kanaele {
            let x = &mut proben[r * kanaele + k];
            *x = ((*x as i64 * v) / 1024) as i16;
        }
    }
}

/// Was eine Pruefung ueber eine Datei sagt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Befund {
    /// Ob der XMP-Block den KI-Quellentyp traegt.
    pub metadaten: bool,
    /// Die Korrelation am Anfang, in Hundertsteln der Standardabweichung.
    pub am_anfang: i64,
    /// Die groesste Korrelation ueber alle Verschiebungen, ebenso.
    pub gesucht: i64,
    /// Das signierte Manifest nach C2PA.
    pub herkunft: Herkunft,
}

impl Befund {
    /// Ob das Wasserzeichen erkannt ist.
    pub fn wasserzeichen(&self) -> bool {
        self.am_anfang >= SCHWELLE_ANFANG || self.gesucht >= SCHWELLE_GESUCHT
    }
}

/// **Prueft eine Tondatei** auf beide Marken.
pub fn pruefen(pfad: &Path) -> Result<Befund, String> {
    let roh = std::fs::read(pfad).map_err(|f| format!("{}: {f}", pfad.display()))?;
    pruefen_daten(&roh).map_err(|f| format!("{}: {f}", pfad.display()))
}

/// [`pruefen`] auf den Bytes einer Datei; ⚑ fremde Eingabe, also endet
/// jede mit einem Befund oder einem Fehler.
pub fn pruefen_daten(roh: &[u8]) -> Result<Befund, String> {
    let wav = Wav::lesen(roh)?;
    let (am_anfang, gesucht) = korrelation(&wav.proben, wav.kanaele as usize);
    Ok(Befund {
        metadaten: wav.xmp.as_deref().is_some_and(|x| x.contains(QUELLENTYP)),
        am_anfang,
        gesucht,
        herkunft: c2pa::pruefen(roh),
    })
}

/// Nur das signierte Manifest nach C2PA, ohne Wasserzeichen und Metadaten.
pub fn herkunft(roh: &[u8]) -> Herkunft {
    c2pa::pruefen(roh)
}

// ---------------------------------------------------------------- Text

/// Ab so vielen Zeichen wird ein Dokument gekennzeichnet: rund 200 Token
/// (der Kodex vom 2026-06-10 nimmt sehr kurzen Text aus), bei rund vier
/// Zeichen je Token in Deutsch und Englisch.
pub const TEXT_AB_ZEICHEN: usize = 800;

/// **Soll ein Dokument, das der Agent schreibt, gekennzeichnet werden?**
/// Nur Markdown und Text, nie Quelltext oder Konfiguration (Leitlinien,
/// Rn. 68), und nur ab [`TEXT_AB_ZEICHEN`].
pub fn text_kennzeichnen_soll(pfad: &Path, text: &str) -> bool {
    let endung = pfad.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    matches!(endung.as_deref(), Some("md" | "markdown" | "txt" | "text")) && ohne_textkennzeichen(text).chars().count() >= TEXT_AB_ZEICHEN
}

/// **Kennzeichnet einen erzeugten Text** mit einem signierten Manifest nach
/// C2PA 2.4, Anhang A.8: unsichtbare Variantenzeichen am Ende, der Text
/// davor in NFC. Eine Huelle, die schon dasteht, wird ersetzt; so bleibt es
/// nach einer Aenderung bei genau einer, die zum neuen Text passt.
///
/// ⚑ Festlegung des Projektinhabers (2026-11-13): nur signierte Metadaten
/// und nur, was der Nutzer nicht sieht; abschaltbar mit
/// `agent.text_kennzeichnen`.
pub fn text_kennzeichnen(text: &str) -> Result<String, String> {
    c2pa::text_signieren(&c2pa::text_ohne_huelle(text))
}

/// Das Manifest eines Texts nach Anhang A.8.
pub fn text_herkunft(text: &str) -> Herkunft {
    c2pa::text_pruefen(text)
}

/// **Der Text ohne Kennzeichen**, fuer alles, was ihn liest: Sonst saehe ein
/// Modell, das das Dokument wieder oeffnet, tausende unsichtbare Zeichen.
pub fn ohne_textkennzeichen(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains('\u{FEFF}') {
        std::borrow::Cow::Owned(c2pa::text_ohne_huelle(text))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

/// **Die Folge, geformt**: zyklisch gefaltet mit den Binomialzahlen der
/// Ordnung `ordnung` (`1 2 1`, `1 4 6 4 1`, ...), ein Tiefpass ohne
/// Gleitkomma. Ordnung null ist die weisse Folge. Dazu der Teiler, der die
/// Energie wieder auf die der weissen bringt, in 256steln: `√(Σ b²) · 256`.
fn folge_geformt(ordnung: usize) -> (Vec<i64>, i64) {
    let c = folge();
    let mut b = vec![1i64];
    for _ in 0..ordnung {
        b = (0..=b.len()).map(|j| b.get(j).copied().unwrap_or(0) + if j > 0 { b[j - 1] } else { 0 }).collect();
    }
    let g = (0..PERIODE).map(|k| b.iter().enumerate().map(|(j, bj)| bj * c[(k + PERIODE - j % PERIODE) % PERIODE]).sum()).collect();
    let quadrate: u128 = b.iter().map(|x| (x * x) as u128).sum();
    (g, wurzel(quadrate * 65_536) as i64)
}

/// Die Folge aus plus und minus eins, aus einem Xorshift mit fester Saat.
fn folge() -> Vec<i64> {
    let mut z = SAAT;
    (0..PERIODE)
        .map(|_| {
            z ^= z << 13;
            z ^= z >> 7;
            z ^= z << 17;
            if z & 1 == 1 { 1 } else { -1 }
        })
        .collect()
}

/// Ganzzahlige Quadratwurzel, abgerundet.
fn wurzel(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Wie viele Proben ein Fenster fuer die oertliche Lautstaerke hat.
const FENSTER: usize = 480;

/// Praegt die Folge ein, mit einer Staerke nach der Lautstaerke je Fenster.
fn einpraegen(proben: &mut [i16], kanaele: usize) {
    einpraegen_lpc_mit(proben, kanaele, TEILER, (LPC_GAMMA << LPC_Q) / 1000, LPC_BODEN, 0);
}

/// Die Folge mit fester binomischer Formung und gesagtem Teiler einpraegen;
/// Ordnung null ist das weisse Wasserzeichen bis 2026-11-11. Nur noch fuer
/// die Proben (alte Dateien nachstellen, Messung).
#[cfg(test)]
fn einpraegen_mit(proben: &mut [i16], kanaele: usize, ordnung: usize, teiler: i64) {
    let (c, norm) = folge_geformt(ordnung);
    let kanaele = kanaele.max(1);
    let rahmen = proben.len() / kanaele;
    let mut anfang = 0;
    while anfang < rahmen {
        let ende = (anfang + FENSTER).min(rahmen);
        let mut summe: u128 = 0;
        for r in anfang..ende {
            for k in 0..kanaele {
                let x = proben[r * kanaele + k] as i128;
                summe += (x * x) as u128;
            }
        }
        let anzahl = ((ende - anfang) * kanaele) as u128;
        let rms = wurzel(summe / anzahl.max(1)) as i64;
        // ⚑ Mindestens zwei Stufen: In der Stille bliebe sonst nichts,
        //   und zwei Stufen von 32767 hoert niemand.
        let staerke = (rms / teiler).max(2);
        for r in anfang..ende {
            // Gerundet: `g · staerke / norm`, `norm` in 256steln.
            let z = c[r % PERIODE] * staerke * 256;
            let chip = (z + z.signum() * norm / 2) / norm;
            for k in 0..kanaele {
                let i = r * kanaele + k;
                proben[i] = (proben[i] as i64 + chip).clamp(i16::MIN as i64, i16::MAX as i64) as i16;
            }
        }
        anfang = ende;
    }
}

/// Ordnung der linearen Vorhersage fuer das Wasserzeichen, das der Stimme folgt.
const LPC_ORDNUNG: usize = 16;
/// Nachkommabits der Vorhersagekoeffizienten.
const LPC_Q: u32 = 30;
/// Wie viele Proben vor dem Fenster die Vorhersage mitsieht.
const LPC_VORLAUF: usize = 240;

/// **Die lineare Vorhersage eines Abschnitts**, ganzzahlig nach
/// Levinson-Durbin: `a[1..=p]` in `Q30`, sodass `e[n] = x[n] + Σ a[j]·x[n−j]`
/// der Vorhersagefehler ist. Mit einem Rauschboden von `2^-boden` der
/// Energie (stabil auch in Stille), und abgebrochen, sobald ein
/// Reflexionskoeffizient den Betrag 0,999 erreicht.
fn lpc_mit(x: &[i64], boden: u32) -> [i64; LPC_ORDNUNG] {
    let p = LPC_ORDNUNG;
    let mut r = [0i128; LPC_ORDNUNG + 1];
    for (k, rk) in r.iter_mut().enumerate() {
        *rk = (k..x.len()).map(|n| x[n] as i128 * x[n - k] as i128).sum();
    }
    let mut aus = [0i64; LPC_ORDNUNG];
    if r[0] <= 0 {
        return aus;
    }
    r[0] += (r[0] >> boden).max(1);
    let eins = 1i128 << LPC_Q;
    let mut a = vec![0i128; p + 1];
    a[0] = eins;
    let mut fehler = r[0];
    for i in 1..=p {
        let acc: i128 = (0..i).map(|j| a[j] * r[i - j]).sum();
        let k = -acc / fehler.max(1);
        if k.abs() >= eins * 999 / 1000 {
            break;
        }
        let alt = a.clone();
        for j in 1..i {
            a[j] = alt[j] + ((k * alt[i - j]) >> LPC_Q);
        }
        a[i] = k;
        fehler = (fehler * (eins * eins - k * k)) >> (2 * LPC_Q);
        if fehler <= 0 {
            break;
        }
    }
    for j in 1..=p {
        aus[j - 1] = a[j] as i64;
    }
    aus
}

/// `a[j] · γʲ`, `γ` in `Q30`: die Hüllkurve etwas weiter, wie die
/// Rauschformung der CELP-Sprachcodecs `1/A(z/γ)`.
fn bandbreite(a: &[i64; LPC_ORDNUNG], gamma: i64) -> [i64; LPC_ORDNUNG] {
    let mut aus = [0i64; LPC_ORDNUNG];
    let mut g = gamma as i128;
    for j in 0..LPC_ORDNUNG {
        aus[j] = ((a[j] as i128 * g) >> LPC_Q) as i64;
        g = (g * gamma as i128) >> LPC_Q;
    }
    aus
}

/// Die Proben gemittelt ueber die Kanaele.
fn einkanalig(proben: &[i16], kanaele: usize) -> Vec<i64> {
    let k = kanaele.max(1);
    proben.chunks(k).map(|r| r.iter().map(|&x| x as i64).sum::<i64>() / k as i64).collect()
}

/// **Praegt das Wasserzeichen ein, das der Stimme folgt**: je Fenster die
/// Folge durch `1/A(z/γ)` der Stimme in diesem Fenster (der Zustand des
/// Filters laeuft ueber die Fenster weiter), auf `max(RMS/teiler, 2)`
/// gebracht, die Verstaerkung zwischen den Fenstern gleitend.
/// **Praegt das Wasserzeichen ein, das der Stimme folgt**, mit Rauschboden der Vorhersage und einem binomischen
/// Tiefpass der Ordnung `vorfilter` auf der Folge davor.
fn einpraegen_lpc_mit(proben: &mut [i16], kanaele: usize, teiler: i64, gamma: i64, boden: u32, vorfilter: usize) {
    let (c, norm) = folge_geformt(vorfilter);
    let k = kanaele.max(1);
    let x = einkanalig(proben, k);
    let rahmen = x.len();
    let mut zustand = [0i64; LPC_ORDNUNG];
    let mut einheit = vec![0i64; rahmen];
    let mut verstaerkung = Vec::new();
    let mut anfang = 0;
    while anfang < rahmen {
        let ende = (anfang + FENSTER).min(rahmen);
        let ag = bandbreite(&lpc_mit(&x[anfang.saturating_sub(LPC_VORLAUF)..ende], boden), gamma);
        for r in anfang..ende {
            let rueck: i128 = ag.iter().zip(&zustand).map(|(&a, &w)| a as i128 * w as i128).sum();
            let w = ((c[r % PERIODE] << 20) / norm) - (rueck >> LPC_Q) as i64;
            zustand.copy_within(0..LPC_ORDNUNG - 1, 1);
            zustand[0] = w;
            einheit[r] = w;
        }
        let quadrat = |v: &[i64]| wurzel(v.iter().map(|&a| (a as i128 * a as i128) as u128).sum::<u128>() / (v.len() as u128).max(1)) as i64;
        let ziel = (quadrat(&x[anfang..ende]) / teiler).max(2);
        // In 2^16steln: aus einer Einheitsfolge mit RMS `quadrat(einheit)` wird eine mit RMS `ziel`.
        verstaerkung.push(((ziel as i128) << 16) / quadrat(&einheit[anfang..ende]).max(1) as i128);
        anfang = ende;
    }
    for r in 0..rahmen {
        let (i, j) = (r / FENSTER, (r % FENSTER) as i128);
        let vorher = if i == 0 { verstaerkung[0] } else { verstaerkung[i - 1] };
        let v = vorher + (verstaerkung[i] - vorher) * j / FENSTER as i128;
        let z = einheit[r] as i128 * v;
        let beitrag = ((z + z.signum() * (1 << 15)) >> 16) as i64;
        for ch in 0..k {
            let p = &mut proben[r * k + ch];
            *p = (*p as i64 + beitrag).clamp(i16::MIN as i64, i16::MAX as i64) as i16;
        }
    }
}

/// [`korrelation`] fuer das Wasserzeichen, das der Stimme folgt: je Fenster
/// die Vorhersage des empfangenen Signals, damit vorab gefiltert
/// (`A(z/γ)`, mit `gamma` gleich `1 << 30` das volle `A(z)`), dann gefaltet
/// und mit der Folge verglichen.
/// Die Korrelation nach der umgekehrten Vorhersage, mit Rauschboden der Vorhersage und Vorfilter der Folge.
#[cfg(test)]
fn korrelation_lpc_mit(proben: &[i16], kanaele: usize, gamma: i64, boden: u32, vorfilter: usize) -> (i64, i64) {
    korrelation_lpc_voll(proben, kanaele, gamma, boden, vorfilter, false)
}

/// [`korrelation_lpc_mit`], mit `normieren` je Fenster der Rest auf gleiche
/// Lautstaerke gebracht, bevor gefaltet wird.
#[cfg(test)]
fn korrelation_lpc_voll(proben: &[i16], kanaele: usize, gamma: i64, boden: u32, vorfilter: usize, normieren: bool) -> (i64, i64) {
    korrelation_lpc_robust(proben, kanaele, gamma, boden, vorfilter, normieren, 0)
}

/// [`korrelation_lpc_voll`], mit `begrenzen > 0` der Rest je Fenster auf das
/// `begrenzen`-fache seines Medianbetrags begrenzt.
fn korrelation_lpc_robust(proben: &[i16], kanaele: usize, gamma: i64, boden: u32, vorfilter: usize, normieren: bool, begrenzen: i64) -> (i64, i64) {
    let (c, _) = folge_geformt(vorfilter);
    let x = einkanalig(proben, kanaele);
    let mut gefaltet = vec![0i64; PERIODE];
    let mut anfang = 0;
    while anfang < x.len() {
        let ende = (anfang + FENSTER).min(x.len());
        let ag = bandbreite(&lpc_mit(&x[anfang.saturating_sub(LPC_VORLAUF)..ende], boden), gamma);
        let rest: Vec<i64> = (anfang..ende)
            .map(|n| {
                let vor: i128 = (0..LPC_ORDNUNG).filter(|&j| n > j).map(|j| ag[j] as i128 * x[n - j - 1] as i128).sum();
                x[n] + (vor >> LPC_Q) as i64
            })
            .collect();
        // Impulse im Rest (Stimmbandimpulse, Spruenge) auf ein Vielfaches
        // der mittleren absoluten Abweichung begrenzt, dann je Fenster auf
        // gleiche Lautstaerke gebracht.
        let mut betraege: Vec<i64> = rest.iter().map(|e| e.abs()).collect();
        betraege.sort_unstable();
        let grenze = if begrenzen > 0 { (betraege[betraege.len() / 2] * begrenzen).max(1) } else { i64::MAX };
        let rest: Vec<i64> = rest.into_iter().map(|e| e.clamp(-grenze, grenze)).collect();
        let teiler = if normieren { wurzel(rest.iter().map(|&e| (e as i128 * e as i128) as u128).sum::<u128>() / rest.len() as u128).max(1) as i64 } else { 1024 };
        for (n, e) in (anfang..ende).zip(rest) {
            gefaltet[n % PERIODE] += e * 1024 / teiler;
        }
        anfang = ende;
    }
    // Ohne Wasserzeichen hat die Summe die Varianz `Σ f(k)² c(k)²`.
    let energie: u128 = gefaltet.iter().zip(&c).map(|(&f, &w)| (f as i128 * f as i128 * (w * w) as i128) as u128).sum();
    let norm = wurzel(energie).max(1) as i128;
    let bei = |s: usize| -> i64 {
        let summe: i128 = (0..PERIODE).map(|k| gefaltet[(k + s) % PERIODE] as i128 * c[k] as i128).sum();
        (summe * 100 / norm) as i64
    };
    let am_anfang = bei(0);
    let gesucht = (0..PERIODE).map(bei).max().unwrap_or(0);
    (am_anfang, gesucht)
}

/// (am Anfang, gesucht), beide in Hundertsteln der Standardabweichung.
///
/// ⚑ Gefaltet: Alle Proben mit demselben Rest nach [`PERIODE`] werden
/// addiert, dann wird die gefaltete Reihe mit der Folge in jeder
/// Verschiebung verglichen. So kostet die Suche ueber alle Verschiebungen
/// nicht mehr als eine Datei von zwei Perioden.
///
/// ⚑ **Aufgehellt, bevor gefaltet wird**: verglichen werden die
/// Differenzen benachbarter Proben mit den Differenzen der Folge. Sprache
/// traegt ihre Energie in den tiefen Frequenzen, und die Differenz daempft
/// genau die; das Wasserzeichen ist weiss und bleibt. 📌 Ohne diesen Schritt
/// reichten zwei Sekunden nicht, um eine vorne abgeschnittene Datei zu
/// erkennen (4,25 statt der noetigen 6,5 Standardabweichungen).
///
/// ⚑ **Der Stimme folgend und weiss**: Dateien, die vor dem 2026-11-11
/// gekennzeichnet wurden, tragen die weisse Folge und bleiben erkannt. Je Wert das
/// groessere; zwei Folgen statt einer verdoppeln hoechstens die Rate
/// falscher Funde, unter den Schwellen weiter selten.
fn korrelation(proben: &[i16], kanaele: usize) -> (i64, i64) {
    let (a, b) = (korrelation_lpc_robust(proben, kanaele, 1 << LPC_Q, LPC_BODEN, 0, true, BEGRENZEN), korrelation_mit(proben, kanaele, 0));
    (a.0.max(b.0), a.1.max(b.1))
}

/// [`korrelation`] gegen die Folge der gesagten Formung.
fn korrelation_mit(proben: &[i16], kanaele: usize, ordnung: usize) -> (i64, i64) {
    let (c, _) = folge_geformt(ordnung);
    let e: Vec<i64> = (0..PERIODE).map(|k| c[k] - c[(k + PERIODE - 1) % PERIODE]).collect();
    let kanaele = kanaele.max(1);
    let mut gefaltet = vec![0i64; PERIODE];
    let mut vorige: Option<i64> = None;
    for (r, rahmen) in proben.chunks(kanaele).enumerate() {
        let mittel: i64 = rahmen.iter().map(|&x| x as i64).sum::<i64>() / kanaele as i64;
        if let Some(v) = vorige {
            gefaltet[r % PERIODE] += mittel - v;
        }
        vorige = Some(mittel);
    }
    // ⚑ Die Norm unter der Annahme ohne Wasserzeichen: Die Summe ueber
    //   f(k) mal e(k) hat dann die Varianz der Summe ueber f(k)^2 e(k)^2.
    let energie: u128 = gefaltet
        .iter()
        .zip(&e)
        .map(|(&f, &w)| (f as i128 * f as i128 * (w * w) as i128) as u128)
        .sum();
    let norm = wurzel(energie).max(1) as i128;
    let bei = |s: usize| -> i64 {
        let summe: i128 = (0..PERIODE).map(|k| gefaltet[(k + s) % PERIODE] as i128 * e[k] as i128).sum();
        (summe * 100 / norm) as i64
    };
    let am_anfang = bei(0);
    let gesucht = (0..PERIODE).map(bei).max().unwrap_or(0);
    (am_anfang, gesucht)
}

/// Eine gelesene Tondatei, auf 16 Bit gebracht.
struct Wav {
    rate: u32,
    kanaele: u16,
    proben: Vec<i16>,
    xmp: Option<String>,
}

impl Wav {
    fn lesen(b: &[u8]) -> Result<Self, String> {
        if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
            return Err("kein WAV".into());
        }
        let wort = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let halb = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        let (mut o, mut format, mut xmp, mut proben) = (12usize, None, None, None);
        while o + 8 <= b.len() {
            let groesse = wort(o + 4) as usize;
            let inhalt = o + 8;
            let ende = (inhalt + groesse).min(b.len());
            match &b[o..o + 4] {
                b"fmt " if inhalt + 16 <= b.len() => {
                    format = Some((halb(inhalt), halb(inhalt + 2), wort(inhalt + 4), halb(inhalt + 14)));
                }
                b"_PMX" => xmp = Some(String::from_utf8_lossy(&b[inhalt..ende]).into_owned()),
                b"data" => proben = Some(inhalt..ende),
                _ => {}
            }
            o = inhalt + groesse + (groesse & 1);
        }
        let (art, kanaele, rate, bits) = format.ok_or("kein fmt-Block")?;
        let daten = &b[proben.ok_or("kein data-Block")?];
        // ⚑ Formatkennung 0xFFFE (erweitert) traegt die eigentliche Art
        //   weiter hinten; die Laeufer schreiben sie nicht, und dann ist
        //   eine ehrliche Ablehnung besser als ein Raten.
        let proben = match (art, bits) {
            (1, 16) => daten.as_chunks::<2>().0.iter().map(|p| i16::from_le_bytes([p[0], p[1]])).collect(),
            (3, 32) => daten
                .as_chunks::<4>().0.iter()
                .map(|p| gleitkomma_zu_16(u32::from_le_bytes([p[0], p[1], p[2], p[3]])))
                .collect(),
            _ => return Err(format!("Format {art} mit {bits} Bit wird nicht gekennzeichnet")),
        };
        Ok(Self { rate, kanaele, proben, xmp })
    }
}

/// **Eine 32-Bit-Gleitkommaprobe als 16-Bit-Ganzzahl**, aus dem
/// Bitmuster gerechnet: `x * 32767`, gerundet zur naechsten geraden Zahl
/// und auf den Bereich begrenzt. Es entsteht keine Gleitkommazahl.
fn gleitkomma_zu_16(bits: u32) -> i16 {
    let negativ = bits >> 31 == 1;
    let exponent = ((bits >> 23) & 0xFF) as i32;
    if exponent == 0 {
        return 0; // null oder subnormal: weit unter einer Stufe
    }
    if exponent == 0xFF {
        return if negativ { i16::MIN } else { i16::MAX };
    }
    let mantisse = ((bits & 0x7F_FFFF) | 0x80_0000) as i128; // 1.m mal 2^23
    // Wert = mantisse * 2^(exponent - 127 - 23); mal 32767.
    let produkt = mantisse * 32767;
    let schub = exponent - 150;
    let betrag: i128 = if schub >= 0 {
        if schub > 40 { i128::MAX / 4 } else { produkt << schub }
    } else {
        let s = (-schub) as u32;
        if s > 100 {
            0
        } else {
            // ⚑ Rechtsshift mit Rundung zur naechsten geraden Zahl.
            let ganz = produkt >> s;
            let rest = produkt - (ganz << s);
            let halb = 1i128 << (s - 1);
            if rest > halb || (rest == halb && ganz & 1 == 1) { ganz + 1 } else { ganz }
        }
    };
    let begrenzt = betrag.min(32767) as i16;
    if negativ { begrenzt.saturating_neg() } else { begrenzt }
}

/// Schreibt ein gekennzeichnetes WAV: fmt, LIST/INFO, XMP, data.
fn schreiben(rate: u32, kanaele: u16, proben: &[i16]) -> Vec<u8> {
    fn block(aus: &mut Vec<u8>, id: &[u8; 4], inhalt: &[u8]) {
        aus.extend_from_slice(id);
        aus.extend_from_slice(&(inhalt.len() as u32).to_le_bytes());
        aus.extend_from_slice(inhalt);
        if inhalt.len() % 2 == 1 {
            aus.push(0);
        }
    }
    let mut fmt = Vec::with_capacity(16);
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&kanaele.to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * kanaele as u32 * 2).to_le_bytes());
    fmt.extend_from_slice(&(kanaele * 2).to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());

    let mut info = b"INFO".to_vec();
    for (id, text) in [(b"ISFT", "Myelith"), (b"ICMT", KOMMENTAR)] {
        let mut t = text.as_bytes().to_vec();
        t.push(0);
        block(&mut info, id, &t);
    }
    let xmp = format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
         <rdf:Description rdf:about=\"\" \
         xmlns:Iptc4xmpExt=\"http://iptc.org/std/Iptc4xmpExt/2008-02-29/\" \
         xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
         Iptc4xmpExt:DigitalSourceType=\"{QUELLENTYP}\" xmp:CreatorTool=\"Myelith\">\
         <dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">{KOMMENTAR}</rdf:li></rdf:Alt></dc:description>\
         </rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end=\"w\"?>"
    );
    let daten: Vec<u8> = proben.iter().flat_map(|p| p.to_le_bytes()).collect();

    let mut rumpf = b"WAVE".to_vec();
    block(&mut rumpf, b"fmt ", &fmt);
    block(&mut rumpf, b"LIST", &info);
    block(&mut rumpf, b"_PMX", xmp.as_bytes());
    block(&mut rumpf, b"data", &daten);
    let mut aus = b"RIFF".to_vec();
    aus.extend_from_slice(&(rumpf.len() as u32).to_le_bytes());
    aus.extend_from_slice(&rumpf);
    aus
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Ein sprachaehnliches Signal: Toene mit wechselnder Lautstaerke,
    /// dazu etwas Rauschen und Pausen, ganzzahlig erzeugt.
    fn signal(saat: u64, sekunden: usize) -> Vec<i16> {
        let mut z = saat | 1;
        let mut zufall = move || {
            z ^= z << 13;
            z ^= z >> 7;
            z ^= z << 17;
            z
        };
        let n = 24_000 * sekunden;
        let mut phase: i64 = 0;
        (0..n)
            .map(|i| {
                let silbe = (i / 3000) % 5;
                let pegel: i64 = if silbe == 4 { 0 } else { 3000 + (silbe as i64) * 1500 };
                phase = (phase + 37 + silbe as i64 * 11) % 1000;
                let dreieck = if phase < 500 { phase - 250 } else { 750 - phase };
                let rauschen = (zufall() % 401) as i64 - 200;
                ((dreieck * pegel) / 250 + rauschen).clamp(-32768, 32767) as i16
            })
            .collect()
    }

    /// Der Ordner verschwindet mit der Probe (Fund 638); bis hierher blieb
    /// je Lauf ein `myl-kennz-<pid>` im Temp-Ordner liegen.
    fn ablegen(name: &str, proben: &[i16]) -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::Builder::new().prefix("myl-kennz-").tempdir().unwrap();
        let p = d.path().join(name);
        // Ohne Metadaten, wie piper es schreibt.
        let mut b = schreiben(24_000, 1, proben);
        let i = b.windows(4).position(|w| w == b"LIST").unwrap();
        let j = b.windows(4).position(|w| w == b"data").unwrap();
        b.drain(i..j);
        let groesse = (b.len() - 8) as u32;
        b[4..8].copy_from_slice(&groesse.to_le_bytes());
        std::fs::write(&p, b).unwrap();
        (d, p)
    }

    /// ⚑ **Gekennzeichnet traegt beide Marken, ungekennzeichnet keine**,
    /// auch wenn vorne abgeschnitten und leiser gemacht wurde.
    #[test]
    fn beide_marken_und_keine_falschen() {
        for saat in [3u64, 17, 99] {
            let roh = signal(saat, 2);
            let (_d, p) = ablegen(&format!("s{saat}.wav"), &roh);
            let vorher = pruefen(&p).unwrap();
            assert!(!vorher.metadaten && !vorher.wasserzeichen(), "{saat}: {vorher:?}");

            synthetisch_kennzeichnen(&p, Modalitaet::Sprache).unwrap();
            let nachher = pruefen(&p).unwrap();
            assert!(nachher.metadaten, "{saat}: keine Metadaten");
            assert!(nachher.wasserzeichen(), "{saat}: kein Wasserzeichen: {nachher:?}");

            // Vorne abgeschnitten und halb so laut.
            let wav = Wav::lesen(&std::fs::read(&p).unwrap()).unwrap();
            let verstuemmelt: Vec<i16> = wav.proben[1234..].iter().map(|x| x / 2).collect();
            let (_e, q) = ablegen(&format!("v{saat}.wav"), &verstuemmelt);
            let b = pruefen(&q).unwrap();
            assert!(!b.metadaten);
            assert!(b.gesucht >= SCHWELLE_GESUCHT, "{saat}: abgeschnitten nicht erkannt: {b:?}");
        }
    }

    /// ⚑ **Die Stoerung bleibt klein**: im Ganzen rund 38 dB unter dem
    /// Signal (ein Achtzigstel), und keine Probe weicht weiter ab als
    /// ein Vielfaches der Fensterstaerke.
    #[test]
    fn die_stoerung_bleibt_klein() {
        let roh = signal(5, 2);
        let (_d, p) = ablegen("klein.wav", &roh);
        synthetisch_kennzeichnen(&p, Modalitaet::Sprache).unwrap();
        let neu = Wav::lesen(&std::fs::read(&p).unwrap()).unwrap().proben;
        let energie = |v: &[i64]| v.iter().map(|x| (x * x) as u128).sum::<u128>();
        let s: Vec<i64> = roh.iter().map(|&x| x as i64).collect();
        let d: Vec<i64> = roh.iter().zip(&neu).map(|(&a, &b)| b as i64 - a as i64).collect();
        assert!(energie(&d) * 2_000 < energie(&s), "Stoerung zu gross");
        let grenze = (2 + 9000 / TEILER) * 2107 / 1000 + 1;
        assert!(d.iter().all(|x| x.abs() <= grenze), "Ausreisser in der Stoerung");
    }

    /// ⚑ **Das Wasserzeichen folgt der Stimme**: Unter einem Signal, das
    /// fast nur tiefe Toene traegt, liegt ueber 4 kHz weniger als ein
    /// Hundertstel seiner Energie (weiss die Haelfte), und es bleibt in jedem
    /// Fenster unter dem Signal. Gegenprobe: die weisse Folge in derselben
    /// Rechnung.
    #[test]
    fn das_wasserzeichen_folgt_der_stimme() {
        // Dreieck um 300 Hz mit wechselnder Lautstaerke, ganzzahlig.
        let roh: Vec<i16> = (0..1 << 15).map(|i| { let p = (i % 80) as i64; let d = if p < 40 { p - 20 } else { 60 - p }; (d * (300 + (i / 4000 % 4) as i64 * 400)) as i16 }).collect();
        let anteil = |weiss: bool| -> (f64, bool) {
            let mut x = roh.clone();
            if weiss { einpraegen_mit(&mut x, 1, 0, TEILER) } else { einpraegen(&mut x, 1) }
            let d: Vec<i64> = x.iter().zip(&roh).map(|(&a, &b)| a as i64 - b as i64).collect();
            let k: Vec<crate::spektrum::Komplex> = d.iter().map(|&v| crate::spektrum::Komplex { re: v as f64, im: 0.0 }).collect();
            let s = crate::spektrum::fft(&k);
            let e = |von: usize, bis: usize| (von..bis).map(|i| s[i].re * s[i].re + s[i].im * s[i].im).sum::<f64>();
            let n = x.len();
            let unter = d.chunks(FENSTER).zip(roh.chunks(FENSTER)).all(|(w, r)| w.iter().map(|v| v * v).sum::<i64>() * 100 < r.iter().map(|&v| v as i64 * v as i64).sum::<i64>());
            (e(n / 6, n / 2) / e(1, n / 2), unter)
        };
        let (geformt, unter) = anteil(false);
        assert!(geformt < 0.01, "ueber 4 kHz: {geformt}");
        assert!(unter, "ein Fenster mit dem Wasserzeichen nicht 20 dB unter dem Signal");
        assert!(anteil(true).0 > 0.4, "die weisse Folge waere auch frei: die Rechnung saehe nichts");
    }

    /// ⚑ **Weiss gekennzeichnete Dateien (bis 2026-11-11) bleiben erkannt.**
    #[test]
    fn die_weisse_folge_wird_weiter_erkannt() {
        let mut x = signal(41, 2);
        einpraegen_mit(&mut x, 1, 0, 64);
        let (am_anfang, _) = korrelation(&x, 1);
        assert!(am_anfang >= SCHWELLE_ANFANG, "weiss nicht mehr erkannt: {am_anfang}");
    }

    /// ⚑ **Zweimal kennzeichnen aendert nichts mehr**, und Gleitkomma wird
    /// ganzzahlig gelesen.
    #[test]
    fn einmal_und_aus_gleitkomma() {
        // 0,5 und -0,25 als IEEE-754, dazu die Grenzen.
        assert_eq!(gleitkomma_zu_16(0x3F00_0000), 16384); // 16383,5 -> gerade 16384
        assert_eq!(gleitkomma_zu_16(0xBE80_0000), -8192); // -8191,75 -> -8192
        assert_eq!(gleitkomma_zu_16(0x3F80_0000), 32767);
        assert_eq!(gleitkomma_zu_16(0x4000_0000), 32767); // 2,0 begrenzt
        assert_eq!(gleitkomma_zu_16(0x0000_0000), 0);

        let roh = signal(8, 2);
        let mut b = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        b.extend_from_slice(&16u32.to_le_bytes());
        for h in [3u16, 1] {
            b.extend_from_slice(&h.to_le_bytes());
        }
        b.extend_from_slice(&24_000u32.to_le_bytes());
        b.extend_from_slice(&96_000u32.to_le_bytes());
        for h in [4u16, 32] {
            b.extend_from_slice(&h.to_le_bytes());
        }
        b.extend_from_slice(b"data");
        b.extend_from_slice(&((roh.len() * 4) as u32).to_le_bytes());
        for &x in &roh {
            // x / 32768 als Gleitkomma-Bitmuster: exakt fuer Zweierpotenzen,
            // hier genuegt der Weg ueber die Standardbibliothek im Test.
            b.extend_from_slice(&(x as f32 / 32768.0).to_bits().to_le_bytes());
        }
        let groesse = (b.len() - 8) as u32;
        b[4..8].copy_from_slice(&groesse.to_le_bytes());
        let d = tempfile::Builder::new().prefix("myl-kennz-f-").tempdir().unwrap();
        let p = d.path().join("f.wav");
        std::fs::write(&p, &b).unwrap();

        synthetisch_kennzeichnen(&p, Modalitaet::Sprache).unwrap();
        let einmal = std::fs::read(&p).unwrap();
        assert!(pruefen(&p).unwrap().wasserzeichen());
        synthetisch_kennzeichnen(&p, Modalitaet::Sprache).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), einmal, "zweimal gekennzeichnet");
    }

    /// **Messung, welcher Filter die Stimme veraendert**, nicht im Lauf der
    /// Proben: `MYL_KENNZ_FILTER` zeigt auf einen Ordner ungekennzeichneter
    /// WAVs; daneben entstehen `-nur-pausen`, `-nur-geformt`, `-weiss` und
    /// `-beides` mit denselben Dateien.
    #[test]
    #[ignore = "Messung: braucht MYL_KENNZ_FILTER"]
    fn filter_einzeln_ablegen() {
        let Ok(ordner) = std::env::var("MYL_KENNZ_FILTER") else { return };
        let ordner = std::path::PathBuf::from(ordner);
        let varianten: Vec<(String, bool, Option<usize>, i64)> = match std::env::var("MYL_KENNZ_VARIANTEN") {
            // `ordnung:teiler,...`, je ohne Pausenfilter.
            Ok(v) => v.split(',').filter_map(|t| t.split_once(':')).map(|(o, t)| (format!("o{o}-t{t}"), false, o.parse().ok(), t.parse().unwrap_or(64))).collect(),
            // `Some(usize::MAX)`: das Wasserzeichen des Pflichtwegs.
            Err(_) => vec![("nur-pausen".into(), true, None, 0), ("nur-wasserzeichen".into(), false, Some(usize::MAX), 0), ("weiss".into(), false, Some(0), 64), ("beides".into(), true, Some(usize::MAX), 0)],
        };
        for (name, pausen, ordnung, teiler) in varianten {
            let ziel = ordner.with_file_name(format!("{}-{name}", ordner.file_name().unwrap().to_string_lossy()));
            std::fs::create_dir_all(&ziel).unwrap();
            for e in std::fs::read_dir(&ordner).unwrap() {
                let p = e.unwrap().path();
                let wav = Wav::lesen(&std::fs::read(&p).unwrap()).unwrap();
                let mut x = wav.proben;
                if pausen {
                    pausen_beruhigen(&mut x, 1, wav.rate);
                }
                match ordnung {
                    Some(usize::MAX) => einpraegen(&mut x, 1),
                    Some(o) => einpraegen_mit(&mut x, 1, o, teiler),
                    None => {}
                }
                std::fs::write(ziel.join(p.file_name().unwrap()), schreiben(wav.rate, 1, &x)).unwrap();
            }
        }
    }

    /// **Messung aller Wasserzeichen unter einem Mass**, nicht im Lauf der
    /// Proben (`MYL_KENNZ_MESSEN`, Varianten in `MYL_KENNZ_ALLE` als
    /// `art:teiler:gamma:boden:vorfilter`, `art` `weiss`, `binom` oder `lpc`).
    /// Erkennung wie [`messen_geformt`]; je Sprachfenster (20 ms, ueber
    /// −45 dBFS) und Band der Abstand Sprache zu Wasserzeichen als fuenftes
    /// Perzentil, dazu der Anteil der Fenster und Baender, in denen das
    /// Wasserzeichen weniger als 20 dB unter der Sprache **und** hoerbar laut
    /// liegt (Effektivwert im Band ueber 2 Stufen).
    #[test]
    #[ignore = "Messung: braucht MYL_KENNZ_MESSEN und ffmpeg"]
    fn messen_alle() {
        let Ok(ordner) = std::env::var("MYL_KENNZ_MESSEN") else { return };
        let mut dateien: Vec<_> = std::fs::read_dir(&ordner).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "wav")).collect();
        dateien.sort();
        let roh: Vec<Vec<i16>> = dateien.iter().map(|p| Wav::lesen(&std::fs::read(p).unwrap()).unwrap().proben).collect();
        let d = tempfile::tempdir().unwrap();
        let mp3 = |x: &[i16]| -> Vec<i16> {
            let (w, m, z) = (d.path().join("a.wav"), d.path().join("a.mp3"), d.path().join("b.wav"));
            std::fs::write(&w, schreiben(24_000, 1, x)).unwrap();
            for args in [vec![w.as_os_str(), "-b:a".as_ref(), "128k".as_ref(), m.as_os_str()], vec![m.as_os_str(), "-ar".as_ref(), "24000".as_ref(), "-ac".as_ref(), "1".as_ref(), z.as_os_str()]] {
                assert!(std::process::Command::new("ffmpeg").args(["-loglevel", "error", "-y", "-i"]).args(args).status().unwrap().success());
            }
            Wav::lesen(&std::fs::read(&z).unwrap()).unwrap().proben
        };
        // Energie je Band eines Fensters (FFT 512) und die Zahl der Stellen je Band.
        let grenzen = [0.0, 2_000.0, 4_000.0, 8_000.0, 12_000.0];
        let baender = |x: &[i64]| -> [(f64, f64); 4] {
            let mut k: Vec<crate::spektrum::Komplex> = x.iter().map(|&v| crate::spektrum::Komplex { re: v as f64, im: 0.0 }).collect();
            k.resize(512, crate::spektrum::Komplex { re: 0.0, im: 0.0 });
            let s = crate::spektrum::fft(&k);
            let mut aus = [(0.0, 0.0); 4];
            for (b, w) in grenzen.windows(2).enumerate() {
                let idx: Vec<usize> = (0..256).filter(|&i| { let f = i as f64 * 24_000.0 / 512.0; f >= w[0] && f < w[1] }).collect();
                aus[b] = (idx.iter().map(|&i| s[i].re * s[i].re + s[i].im * s[i].im).sum::<f64>().max(1e-6), idx.len() as f64);
            }
            aus
        };
        let alle = std::env::var("MYL_KENNZ_ALLE").unwrap_or("weiss:64:0:0:0,binom:32:0:0:6,lpc:32:920:9:0".into());
        for v in alle.split(',') {
            let t: Vec<&str> = v.split(':').collect();
            let (art, teiler, gamma, boden, vorfilter) = (t[0], t[1].parse::<i64>().unwrap(), (t[2].parse::<i64>().unwrap() << 30) / 1000, t[3].parse::<u32>().unwrap(), t[4].parse::<usize>().unwrap());
            let praegen = |m: &mut Vec<i16>| match art {
                "weiss" => einpraegen_mit(m, 1, 0, teiler),
                "binom" => einpraegen_mit(m, 1, vorfilter, teiler),
                _ => einpraegen_lpc_mit(m, 1, teiler, gamma, boden, vorfilter),
            };
            let erkennen = |m: &[i16]| match art {
                "weiss" => korrelation_mit(m, 1, 0),
                "binom" => korrelation_mit(m, 1, vorfilter),
                "lpcn" => korrelation_lpc_voll(m, 1, 1 << 30, boden, vorfilter, true),
                b if b.starts_with("lpcb") => korrelation_lpc_robust(m, 1, 1 << 30, boden, vorfilter, true, b[4..].parse().unwrap_or(3)),
                _ => korrelation_lpc_mit(m, 1, 1 << 30, boden, vorfilter),
            };
            let mut schlecht = [i64::MAX; 6];
            let mut abstand: Vec<Vec<f64>> = vec![Vec::new(); 4];
            let (mut verstoss, mut gezaehlt) = (0usize, 0usize);
            let mut ohne = 0i64;
            for x in &roh {
                ohne = ohne.max(erkennen(x).1);
                for (teil, stueck) in [(&x[..], false), (&x[..x.len().min(23_040)], true)] {
                    let mut m = teil.to_vec();
                    praegen(&mut m);
                    let o = if stueck { 3 } else { 0 };
                    schlecht[o] = schlecht[o].min(erkennen(&m).0);
                    let verst: Vec<i16> = m[1234.min(m.len())..].iter().map(|v| v / 2).collect();
                    schlecht[o + 1] = schlecht[o + 1].min(erkennen(&verst).1);
                    schlecht[o + 2] = schlecht[o + 2].min(erkennen(&mp3(&m)).1);
                    if stueck {
                        continue;
                    }
                    for (f, (a, b)) in teil.chunks(FENSTER).zip(m.chunks(FENSTER)).enumerate() {
                        let sp: Vec<i64> = a.iter().map(|&v| v as i64).collect();
                        if f == 0 || wurzel(sp.iter().map(|&v| (v * v) as u128).sum::<u128>() / sp.len() as u128) < 184 {
                            continue;
                        }
                        let wz: Vec<i64> = b.iter().zip(a).map(|(&p, &q)| p as i64 - q as i64).collect();
                        let (bs, bw) = (baender(&sp), baender(&wz));
                        for i in 0..4 {
                            let db = 10.0 * (bs[i].0 / bw[i].0).log10();
                            abstand[i].push(db);
                            // Effektivwert des Wasserzeichens im Band, in Stufen (Parseval, 512 Stellen, 480 Proben).
                            let rms = (2.0 * bw[i].0 / (512.0 * 480.0)).sqrt();
                            gezaehlt += 1;
                            if db < 20.0 && rms > 2.0 {
                                verstoss += 1;
                            }
                        }
                    }
                }
            }
            let mut zeile = format!(
                "[alle] {v}: ohne {:.1}; Satz {:.1} / {:.1} / {:.1}; Stueck {:.1} / {:.1} / {:.1}; hoerbar unter 20 dB {:.2} %",
                ohne as f64 / 100.0,
                schlecht[0] as f64 / 100.0, schlecht[1] as f64 / 100.0, schlecht[2] as f64 / 100.0,
                schlecht[3] as f64 / 100.0, schlecht[4] as f64 / 100.0, schlecht[5] as f64 / 100.0,
                100.0 * verstoss as f64 / gezaehlt.max(1) as f64
            );
            for (i, band) in ["0-2", "2-4", "4-8", "8-12"].iter().enumerate() {
                let w = &mut abstand[i];
                w.sort_by(|a, b| a.total_cmp(b));
                zeile += &format!("; {band} P5 {:.1}", w[w.len() / 20]);
            }
            eprintln!("{zeile}");
        }
    }
}
