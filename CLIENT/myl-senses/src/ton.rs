//! **Der Ton**: WAV lesen und schreiben, Umtasten, ohne fremde Laufzeit.
//!
//! Baustein T aus CLIENT 8.0. Hoeren und Sprechen brauchen ihn, und die
//! Aufnahme spaeter auch; bis dahin lag ein WAV-Leser fuer 16 Bit und einen
//! Kanal privat im Sprechen, ein zweiter las die Dauer aus einem festen
//! Kopf von 44 Bytes. Jetzt liest diese eine Stelle.
//!
//! # ⚑ Die Bloecke werden gelesen, nicht angenommen
//!
//! Ein WAV ist eine Folge von Bloecken (`fmt `, `data`, oft ein `LIST`
//! dazwischen). ⚠️ ffmpeg schreibt gern einen `LIST`-Block vor die Daten;
//! wer 44 Bytes Kopf annimmt, liest ihn als Ton oder die Dauer falsch.
//!
//! # Das Umtasten
//!
//! Gefensterter Sinc bei rationalem Verhaeltnis (`nach/von` gekuerzt),
//! Grenzfrequenz bei 0,95 der kleineren Nyquistfrequenz, 32 Nulldurchgaenge
//! je Seite, Blackman-Fenster. Vorlage im Gedanken: die bandbegrenzte
//! Interpolation nach Smith (CCRMA), neu geschrieben.

/// Die Form der Proben im `data`-Block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probenart {
    /// Ganzzahlig mit Vorzeichen (8 Bit ohne), Bits je Probe.
    Ganzzahl(u16),
    /// Gleitkomma mit 32 Bit.
    Gleitkomma32,
}

/// Was der Kopf eines WAV sagt, und wo die Daten liegen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kopf {
    pub art: Probenart,
    pub kanaele: u16,
    pub rate: u32,
    /// Die Bytes des `data`-Blocks, auf ganze Rahmen gekuerzt.
    pub daten: std::ops::Range<usize>,
}

impl Kopf {
    /// Bytes je Rahmen (eine Probe je Kanal).
    pub fn rahmen(&self) -> usize {
        let bytes = match self.art {
            Probenart::Ganzzahl(bits) => usize::from(bits).div_ceil(8),
            Probenart::Gleitkomma32 => 4,
        };
        bytes * usize::from(self.kanaele)
    }

    /// Die Dauer in Millisekunden.
    pub fn dauer_ms(&self) -> u64 {
        let rahmen = (self.daten.len() / self.rahmen().max(1)) as u64;
        rahmen * 1000 / u64::from(self.rate.max(1))
    }
}

/// ⚑ **Abtastraten, die eine Tonaufnahme haben kann** (2026-11-12): von
/// 1 kHz bis 768 kHz. 📌 Ohne Grenze nahm der Kopf auch 1 Hz an, und
/// `umtasten` auf 16 kHz rechnete das 16 000-fache; ein Anhang von wenigen
/// Kilobyte hielt das Programm fest.
pub const RATEN: std::ops::RangeInclusive<u32> = 1_000..=768_000;

/// Warum ein WAV nicht lesbar ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tonfehler {
    KeinWav,
    KeinFormat,
    KeineDaten,
    Unbekannt { format: u16, bits: u16 },
    Rate(u32),
}

impl std::fmt::Display for Tonfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeinWav => write!(f, "keine WAV-Datei (RIFF/WAVE fehlt)"),
            Self::KeinFormat => write!(f, "WAV ohne Formatblock vor den Daten"),
            Self::KeineDaten => write!(f, "WAV ohne Datenblock"),
            Self::Unbekannt { format, bits } => write!(f, "WAV-Format {format} mit {bits} Bit wird nicht gelesen"),
            Self::Rate(r) => write!(f, "WAV mit {r} Hz, keine Abtastrate einer Tonaufnahme"),
        }
    }
}

impl std::error::Error for Tonfehler {}

/// **Liest den Kopf eines WAV** aus seinen Bloecken.
pub fn kopf(b: &[u8]) -> Result<Kopf, Tonfehler> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(Tonfehler::KeinWav);
    }
    let wort = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let halb = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    let mut o = 12usize;
    let mut format = None;
    while o + 8 <= b.len() {
        let groesse = wort(o + 4) as usize;
        let inhalt = o + 8;
        match &b[o..o + 4] {
            b"fmt " if inhalt + 16 <= b.len() => {
                let mut code = halb(inhalt);
                let bits = halb(inhalt + 14);
                // WAVE_FORMAT_EXTENSIBLE: die eigentliche Art steht in den
                // ersten zwei Bytes der Untertyp-Kennung.
                if code == 0xFFFE && inhalt + 26 <= b.len() {
                    code = halb(inhalt + 24);
                }
                let art = match (code, bits) {
                    (1, 8 | 16 | 24 | 32) => Probenart::Ganzzahl(bits),
                    (3, 32) => Probenart::Gleitkomma32,
                    _ => return Err(Tonfehler::Unbekannt { format: code, bits }),
                };
                format = Some((art, halb(inhalt + 2).max(1), wort(inhalt + 4)));
            }
            b"data" => {
                let (art, kanaele, rate) = format.ok_or(Tonfehler::KeinFormat)?;
                if !RATEN.contains(&rate) {
                    return Err(Tonfehler::Rate(rate));
                }
                let ende = (inhalt + groesse).min(b.len());
                let mut k = Kopf { art, kanaele, rate, daten: inhalt..inhalt };
                let rahmen = k.rahmen();
                k.daten = inhalt..inhalt + (ende - inhalt) / rahmen * rahmen;
                return Ok(k);
            }
            _ => {}
        }
        o = inhalt + groesse + (groesse & 1);
    }
    Err(Tonfehler::KeineDaten)
}

/// Ein Ton: einkanalig, Proben in [-1, 1].
#[derive(Debug, Clone, PartialEq)]
pub struct Ton {
    pub rate: u32,
    pub proben: Vec<f32>,
}

/// **Liest ein WAV** und mischt alle Kanaele zu einem.
pub fn lesen(b: &[u8]) -> Result<Ton, Tonfehler> {
    let k = kopf(b)?;
    let d = &b[k.daten.clone()];
    let kanaele = usize::from(k.kanaele);
    let probe = |r: &[u8]| -> f32 {
        match k.art {
            Probenart::Ganzzahl(8) => (f32::from(r[0]) - 128.0) / 128.0,
            Probenart::Ganzzahl(16) => f32::from(i16::from_le_bytes([r[0], r[1]])) / 32_768.0,
            Probenart::Ganzzahl(24) => (i32::from_le_bytes([0, r[0], r[1], r[2]]) >> 8) as f32 / 8_388_608.0,
            Probenart::Ganzzahl(_) => i32::from_le_bytes([r[0], r[1], r[2], r[3]]) as f32 / 2_147_483_648.0,
            Probenart::Gleitkomma32 => f32::from_le_bytes([r[0], r[1], r[2], r[3]]),
        }
    };
    let je = k.rahmen() / kanaele;
    let proben = d
        .chunks_exact(k.rahmen())
        .map(|rahmen| rahmen.chunks_exact(je).map(probe).sum::<f32>() / kanaele as f32)
        .collect();
    Ok(Ton { rate: k.rate, proben })
}

/// **Schreibt einen Ton als WAV** mit 16 Bit und einem Kanal.
pub fn schreiben(ton: &Ton) -> Vec<u8> {
    let laenge = (ton.proben.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + laenge as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + laenge).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&ton.rate.to_le_bytes());
    b.extend_from_slice(&(ton.rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&laenge.to_le_bytes());
    for p in &ton.proben {
        // ×32768 wie beim Lesen, damit ein gelesenes PCM16 bitgleich
        // zurueckkommt; +1 wird zu 32767 gekappt.
        let v = (p * 32_768.0).round().clamp(-32_768.0, 32_767.0) as i16;
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

/// Nulldurchgaenge des Sinc je Seite.
pub const NULLDURCHGAENGE: usize = 32;

fn ggt(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        ggt(b, a % b)
    }
}

/// **Tastet um** auf `nach` Proben je Sekunde.
pub fn umtasten(ton: &Ton, nach: u32) -> Ton {
    if ton.rate == nach || ton.proben.is_empty() || nach == 0 || ton.rate == 0 {
        return Ton { rate: if nach == 0 { ton.rate } else { nach }, proben: ton.proben.clone() };
    }
    let (von, zu) = (u64::from(ton.rate), u64::from(nach));
    let g = ggt(von, zu);
    let (l, m) = (zu / g, von / g);
    // Grenze relativ zur Eingangsrate, unter der kleineren Nyquistfrequenz.
    let grenze = 0.95 * (zu.min(von) as f64) / (von as f64);
    let halbe = NULLDURCHGAENGE as f64 / grenze;
    let n_aus = (ton.proben.len() as u64 * l).div_ceil(m) as usize;
    let pi = std::f64::consts::PI;
    let mut aus = Vec::with_capacity(n_aus);
    for j in 0..n_aus {
        // Zeitpunkt in Eingangsproben: j·m/l, als Bruch genau.
        let mitte = (j as u64 * m) as f64 / l as f64;
        let ab = (mitte - halbe).ceil().max(0.0) as usize;
        let bis = ((mitte + halbe).floor() as usize).min(ton.proben.len() - 1);
        let mut s = 0.0f64;
        for (i, p) in ton.proben.iter().enumerate().take(bis + 1).skip(ab) {
            let t = i as f64 - mitte;
            let sinc = if t == 0.0 { grenze } else { (pi * grenze * t).sin() / (pi * t) };
            let w = 0.42 + 0.5 * (pi * t / halbe).cos() + 0.08 * (2.0 * pi * t / halbe).cos();
            s += f64::from(*p) * sinc * w;
        }
        aus.push(s as f32);
    }
    Ton { rate: nach, proben: aus }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sinus(rate: u32, hz: f64, n: usize) -> Vec<f32> {
        (0..n).map(|i| (0.5 * (2.0 * std::f64::consts::PI * hz * i as f64 / f64::from(rate)).sin()) as f32).collect()
    }

    /// PCM16 kommt bitgleich zurueck.
    #[test]
    fn pcm16_kommt_bitgleich_zurueck() {
        let ton = Ton { rate: 16_000, proben: (0..1000).map(|i| ((i % 200) as f32 - 100.0) / 128.0).collect() };
        let einmal = schreiben(&ton);
        let zurueck = lesen(&einmal).expect("lesbar");
        assert_eq!(schreiben(&zurueck), einmal);
        assert_eq!(kopf(&einmal).expect("Kopf").dauer_ms(), 62);
    }

    /// ⚠️ Ein `LIST`-Block vor den Daten wird uebersprungen, nicht gelesen.
    #[test]
    fn ein_list_block_vor_den_daten_wird_uebersprungen() {
        let roh = schreiben(&Ton { rate: 8_000, proben: vec![0.25; 80] });
        let mut mit = roh[..36].to_vec();
        mit.extend_from_slice(b"LIST");
        mit.extend_from_slice(&5u32.to_le_bytes());
        mit.extend_from_slice(b"INFO\0\0"); // ungerade Groesse, ein Fuellbyte
        mit.extend_from_slice(&roh[36..]);
        let t = lesen(&mit).expect("lesbar");
        assert_eq!(t.proben.len(), 80);
        assert!(t.proben.iter().all(|p| (p - 0.25).abs() < 1e-4));
    }

    /// ⛔️ **Eine erfundene Abtastrate wird abgewiesen**: Mit 1 Hz rechnete
    /// `umtasten` auf 16 kHz das 16 000-fache, und eine kleine Datei hielt
    /// das Programm fest. Echte Raten von 8 kHz bis 384 kHz gehen durch.
    #[test]
    fn eine_erfundene_rate_wird_abgewiesen() {
        for (rate, gut) in [(1, false), (999, false), (8_000, true), (44_100, true), (384_000, true), (2_000_000, false)] {
            let b = schreiben(&Ton { rate, proben: vec![0.0; 4] });
            assert_eq!(kopf(&b).is_ok(), gut, "{rate} Hz");
        }
    }

    /// Zwei Kanaele werden gemischt, Gleitkomma und 24 Bit gelesen, ein
    /// unbekanntes Format abgewiesen.
    #[test]
    fn kanaele_und_formate() {
        let mut b = schreiben(&Ton { rate: 16_000, proben: vec![] });
        // Stereo, 32 Bit Gleitkomma: Format 3, zwei Kanaele.
        b[20..22].copy_from_slice(&3u16.to_le_bytes());
        b[22..24].copy_from_slice(&2u16.to_le_bytes());
        b[34..36].copy_from_slice(&32u16.to_le_bytes());
        for v in [0.5f32, -0.25, 1.0, 0.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b[40..44].copy_from_slice(&16u32.to_le_bytes());
        let t = lesen(&b).expect("lesbar");
        assert_eq!(t.proben, vec![0.125, 0.5]);
        let mut d = b.clone();
        d[20..22].copy_from_slice(&1u16.to_le_bytes());
        d[22..24].copy_from_slice(&1u16.to_le_bytes());
        d[34..36].copy_from_slice(&24u16.to_le_bytes());
        d.truncate(44);
        d.extend_from_slice(&[0x00, 0x00, 0x40, 0x00, 0x00, 0xC0]); // +0,5 und -0,5
        d[40..44].copy_from_slice(&6u32.to_le_bytes());
        assert_eq!(lesen(&d).expect("lesbar").proben, vec![0.5, -0.5]);
        let mut u = b.clone();
        u[20..22].copy_from_slice(&2u16.to_le_bytes()); // ADPCM
        assert!(matches!(lesen(&u), Err(Tonfehler::Unbekannt { format: 2, .. })));
        assert_eq!(lesen(b"keine Datei, nur Text"), Err(Tonfehler::KeinWav));
    }

    /// 48 kHz auf 16 kHz: ein Ton von 1 kHz bleibt, mit weniger als
    /// −60 dB Fehler gegen den genauen Sinus.
    #[test]
    fn umtasten_haelt_einen_ton_im_band() {
        let ein = Ton { rate: 48_000, proben: sinus(48_000, 1_000.0, 48_000) };
        let aus = umtasten(&ein, 16_000);
        assert_eq!(aus.rate, 16_000);
        assert_eq!(aus.proben.len(), 16_000);
        let soll = sinus(16_000, 1_000.0, 16_000);
        // Ohne die Raender, wo das Filter ueber das Ende reicht.
        let (mut fehler, mut leistung) = (0.0f64, 0.0f64);
        for (a, s) in aus.proben[100..15_900].iter().zip(&soll[100..15_900]) {
            fehler += f64::from(a - s).powi(2);
            leistung += f64::from(*s).powi(2);
        }
        let db = 10.0 * (fehler / leistung).log10();
        assert!(db < -60.0, "Fehler {db:.1} dB");
        // Auch nicht ganzzahlig: 16 kHz auf 24 kHz.
        let hoch = umtasten(&Ton { rate: 16_000, proben: soll.clone() }, 24_000);
        assert_eq!(hoch.proben.len(), 24_000);
    }

    /// ⛔️ Gegenprobe: Ein Ton ueber der neuen Nyquistfrequenz faltet nicht
    /// zurueck, er wird um mehr als 60 dB gedaempft.
    #[test]
    fn ueber_der_neuen_grenze_wird_gedaempft() {
        let ein = Ton { rate: 48_000, proben: sinus(48_000, 11_000.0, 48_000) };
        let aus = umtasten(&ein, 16_000);
        let rest: f64 = aus.proben[100..15_900].iter().map(|p| f64::from(*p).powi(2)).sum::<f64>() / 15_800.0;
        let db = 10.0 * (rest / 0.125).log10();
        assert!(db < -60.0, "der Ton faltet mit {db:.1} dB zurueck");
    }
}
