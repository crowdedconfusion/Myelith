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
//!    und minus eins, alle [`PERIODE`] Proben wiederholt, mit einem
//!    Vierundsechzigstel der oertlichen Lautstaerke dazugegeben (rund
//!    36 dB darunter). Erkannt wird es durch Korrelation; es uebersteht
//!    Kopieren, Pegelaenderungen, Abschneiden am Anfang und MP3 mit
//!    128 kbit/s.
//!
//! ⚠️ **Die Folge ist oeffentlich**, denn der Quelltext ist es. Wer sie
//! kennt, kann sie abziehen. Das Wasserzeichen erschwert das versehentliche
//! Verlieren der Kennzeichnung, es verhindert kein absichtliches Entfernen.
//!
//! ⚑ **Ganzzahlig**, wie alles in diesem Projekt: Gleitkommaproben werden
//! als Bitmuster gelesen und in 16 Bit umgerechnet, ohne dass eine
//! Gleitkommazahl entsteht.

use std::path::Path;

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

/// Das Verhaeltnis von Lautstaerke zu Wasserzeichen: ein Vierundsechzigstel,
/// rund 36 dB darunter.
///
/// ⚑ **Gemessen an echter Sprache** (2026-09-25, 14 Saetze aus dem
/// Sprechmodell, 1,7 bis 5,6 s): ohne Kennzeichnung hoechstens 2,5
/// Standardabweichungen; gekennzeichnet mindestens 11,9, vorne
/// abgeschnitten und halb so laut mindestens 11,8, nach MP3 mit 128 kbit/s
/// mindestens 10,3. Ein Zweiunddreissigstel (30 dB) gab rund das Doppelte
/// und ist lauter; der Abstand zur Schwelle reicht auch so.
pub const TEILER: i64 = 64;

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
    einpraegen(&mut proben, wav.kanaele as usize);
    std::fs::write(pfad, schreiben(wav.rate, wav.kanaele, &proben))
        .map_err(|f| format!("{}: {f}", pfad.display()))
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
    let wav = Wav::lesen(&roh).map_err(|f| format!("{}: {f}", pfad.display()))?;
    let (am_anfang, gesucht) = korrelation(&wav.proben, wav.kanaele as usize);
    Ok(Befund {
        metadaten: wav.xmp.as_deref().is_some_and(|x| x.contains(QUELLENTYP)),
        am_anfang,
        gesucht,
    })
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
    let c = folge();
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
        let staerke = (rms / TEILER).max(2);
        for r in anfang..ende {
            let chip = c[r % PERIODE] * staerke;
            for k in 0..kanaele {
                let i = r * kanaele + k;
                proben[i] = (proben[i] as i64 + chip).clamp(i16::MIN as i64, i16::MAX as i64) as i16;
            }
        }
        anfang = ende;
    }
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
fn korrelation(proben: &[i16], kanaele: usize) -> (i64, i64) {
    let c = folge();
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
            (1, 16) => daten.chunks_exact(2).map(|p| i16::from_le_bytes([p[0], p[1]])).collect(),
            (3, 32) => daten
                .chunks_exact(4)
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

    fn ablegen(name: &str, proben: &[i16]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("myl-kennz-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(name);
        // Ohne Metadaten, wie piper es schreibt.
        let mut b = schreiben(24_000, 1, proben);
        let i = b.windows(4).position(|w| w == b"LIST").unwrap();
        let j = b.windows(4).position(|w| w == b"data").unwrap();
        b.drain(i..j);
        let groesse = (b.len() - 8) as u32;
        b[4..8].copy_from_slice(&groesse.to_le_bytes());
        std::fs::write(&p, b).unwrap();
        p
    }

    /// ⚑ **Gekennzeichnet traegt beide Marken, ungekennzeichnet keine**,
    /// auch wenn vorne abgeschnitten und leiser gemacht wurde.
    #[test]
    fn beide_marken_und_keine_falschen() {
        for saat in [3u64, 17, 99] {
            let roh = signal(saat, 2);
            let p = ablegen(&format!("s{saat}.wav"), &roh);
            let vorher = pruefen(&p).unwrap();
            assert!(!vorher.metadaten && !vorher.wasserzeichen(), "{saat}: {vorher:?}");

            synthetisch_kennzeichnen(&p, Modalitaet::Sprache).unwrap();
            let nachher = pruefen(&p).unwrap();
            assert!(nachher.metadaten, "{saat}: keine Metadaten");
            assert!(nachher.wasserzeichen(), "{saat}: kein Wasserzeichen: {nachher:?}");

            // Vorne abgeschnitten und halb so laut.
            let wav = Wav::lesen(&std::fs::read(&p).unwrap()).unwrap();
            let verstuemmelt: Vec<i16> = wav.proben[1234..].iter().map(|x| x / 2).collect();
            let q = ablegen(&format!("v{saat}.wav"), &verstuemmelt);
            let b = pruefen(&q).unwrap();
            assert!(!b.metadaten);
            assert!(b.gesucht >= SCHWELLE_GESUCHT, "{saat}: abgeschnitten nicht erkannt: {b:?}");
        }
    }

    /// ⚑ **Die Stoerung bleibt klein**: rund 30 dB unter dem Signal, also
    /// weniger als ein Tausendstel seiner Energie, und in der Stille zwei
    /// Stufen.
    #[test]
    fn die_stoerung_bleibt_klein() {
        let roh = signal(5, 2);
        let p = ablegen("klein.wav", &roh);
        synthetisch_kennzeichnen(&p, Modalitaet::Sprache).unwrap();
        let neu = Wav::lesen(&std::fs::read(&p).unwrap()).unwrap().proben;
        let energie = |v: &[i64]| v.iter().map(|x| (x * x) as u128).sum::<u128>();
        let s: Vec<i64> = roh.iter().map(|&x| x as i64).collect();
        let d: Vec<i64> = roh.iter().zip(&neu).map(|(&a, &b)| b as i64 - a as i64).collect();
        assert!(energie(&d) * 900 < energie(&s), "Stoerung zu gross");
        assert!(d.iter().all(|x| x.abs() <= 2 + 9000 / TEILER), "Ausreisser in der Stoerung");
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
        let d = std::env::temp_dir().join(format!("myl-kennz-f-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("f.wav");
        std::fs::write(&p, &b).unwrap();

        synthetisch_kennzeichnen(&p, Modalitaet::Sprache).unwrap();
        let einmal = std::fs::read(&p).unwrap();
        assert!(pruefen(&p).unwrap().wasserzeichen());
        synthetisch_kennzeichnen(&p, Modalitaet::Sprache).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), einmal, "zweimal gekennzeichnet");
    }
}
