//! **FLAC lesen**, ohne fremdes Programm (CLIENT 8.1, 2026-11-12).
//!
//! Nach RFC 9639, neu geschrieben: Kopf mit `STREAMINFO`, Rahmen mit
//! CRC-8 und CRC-16, Unterrahmen `CONSTANT`, `VERBATIM`, `FIXED` und `LPC`,
//! Restwerte nach Rice, die drei Kanalpaare (links/Seite, Seite/rechts,
//! Mitte/Seite) und ungenutzte Bits.
//!
//! # ⚑ Bit fuer Bit geprueft
//!
//! FLAC ist verlustfrei, und `STREAMINFO` traegt die MD5-Summe der
//! unkomprimierten Proben. Steht dort eine (nicht null), rechnet dieser
//! Leser sie nach und weist die Datei ab, wenn sie nicht stimmt. Ein Leser,
//! der die Summe trifft, hat jede Probe richtig gelesen; deshalb pruefen die
//! Proben gegen Dateien zweier Kodierer nur diese Summe.
//!
//! ⚠️ **Eine Obergrenze fuer die Laenge**: Ein Rahmen aus `CONSTANT`
//! beschreibt 65 535 Proben in wenigen Bytes. Ohne Deckel liesse eine kleine
//! Datei den Speicher volllaufen.

use crate::ton::Ton;

/// Hoechstens so viele Proben je Kanal (rund 23 Minuten bei 48 kHz).
pub const PROBEN_HOECHSTENS: usize = 1 << 26;

/// Was eine FLAC-Datei enthaelt, je Kanal die Proben als ganze Zahlen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flac {
    pub rate: u32,
    pub bits: u32,
    pub kanaele: Vec<Vec<i32>>,
}

/// Beginnt die Datei wie FLAC (auch hinter einem ID3-Kopf)?
pub fn ist_flac(b: &[u8]) -> bool {
    b.get(id3_ueberspringen(b)..).is_some_and(|r| r.starts_with(b"fLaC"))
}

/// **Liest eine FLAC-Datei** und mischt alle Kanaele zu einem.
pub fn lesen(b: &[u8]) -> Result<Ton, String> {
    let f = dekodieren(b)?;
    let n = f.kanaele.first().map_or(0, Vec::len);
    let skala = (1u64 << (f.bits - 1)) as f32;
    let zahl = f.kanaele.len() as f32;
    let proben = (0..n).map(|i| f.kanaele.iter().map(|k| k[i] as f32).sum::<f32>() / zahl / skala).collect();
    Ok(Ton { rate: f.rate, proben })
}

/// ID3v2 vor dem Strom: zehn Bytes Kopf, die Groesse in je sieben Bit.
fn id3_ueberspringen(b: &[u8]) -> usize {
    if b.len() >= 10 && &b[..3] == b"ID3" {
        let g = b[6..10].iter().fold(0usize, |n, &x| n << 7 | usize::from(x & 0x7f));
        let fuss = if b[5] & 0x10 != 0 { 10 } else { 0 };
        10 + g + fuss
    } else {
        0
    }
}

/// **Dekodiert die ganze Datei.** Fehler sind Saetze, keine Panik.
pub fn dekodieren(b: &[u8]) -> Result<Flac, String> {
    let mut o = id3_ueberspringen(b);
    if b.get(o..o + 4) != Some(b"fLaC") {
        return Err("kein FLAC (fLaC fehlt)".into());
    }
    o += 4;
    let mut info = None;
    loop {
        let kopf = *b.get(o).ok_or("FLAC: Kopf endet vor dem letzten Block")?;
        let laenge = b.get(o + 1..o + 4).ok_or("FLAC: Kopf abgeschnitten")?.iter().fold(0usize, |n, &x| n << 8 | usize::from(x));
        let inhalt = b.get(o + 4..o + 4 + laenge).ok_or("FLAC: Block abgeschnitten")?;
        if kopf & 0x7f == 0 {
            if laenge != 34 {
                return Err("FLAC: STREAMINFO hat nicht 34 Bytes".into());
            }
            info = Some(Streaminfo::lesen(inhalt)?);
        }
        o += 4 + laenge;
        if kopf & 0x80 != 0 {
            break;
        }
    }
    let info = info.ok_or("FLAC: STREAMINFO fehlt")?;
    let mut kanaele = vec![Vec::new(); info.kanaele as usize];
    while o < b.len() {
        // ID3v1 am Ende: 128 Bytes, die mit `TAG` beginnen.
        if b.len() - o == 128 && b[o..].starts_with(b"TAG") {
            break;
        }
        let (n, rest) = rahmen(&b[o..], &info, &mut kanaele)?;
        o += n;
        if rest {
            break;
        }
        if kanaele[0].len() > PROBEN_HOECHSTENS {
            return Err("FLAC: laenger als erlaubt".into());
        }
    }
    if info.proben > 0 && kanaele[0].len() as u64 != info.proben {
        return Err(format!("FLAC: {} Proben statt {}", kanaele[0].len(), info.proben));
    }
    if info.md5 != [0; 16] && md5_der_proben(&kanaele, info.bits) != info.md5 {
        return Err("FLAC: die MD5-Summe der Proben stimmt nicht".into());
    }
    Ok(Flac { rate: info.rate, bits: info.bits, kanaele })
}

struct Streaminfo {
    rate: u32,
    kanaele: u32,
    bits: u32,
    proben: u64,
    md5: [u8; 16],
}

impl Streaminfo {
    fn lesen(s: &[u8]) -> Result<Self, String> {
        let mut r = Bits::neu(&s[10..18]);
        let rate = r.lies(20)? as u32;
        let kanaele = r.lies(3)? as u32 + 1;
        let bits = r.lies(5)? as u32 + 1;
        let proben = r.lies(36)?;
        if !crate::ton::RATEN.contains(&rate) || !(4..=32).contains(&bits) {
            return Err("FLAC: STREAMINFO unsinnig (Abtastrate oder Breite)".into());
        }
        let mut md5 = [0; 16];
        md5.copy_from_slice(&s[18..34]);
        Ok(Self { rate, kanaele, bits, proben, md5 })
    }
}

/// Liest Bits von vorn, das hoechste zuerst.
struct Bits<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    fn neu(b: &'a [u8]) -> Self {
        Self { b, pos: 0 }
    }

    fn lies(&mut self, n: u32) -> Result<u64, String> {
        let mut v = 0u64;
        for _ in 0..n {
            let byte = *self.b.get(self.pos / 8).ok_or("FLAC: Rahmen endet zu frueh")?;
            v = v << 1 | u64::from(byte >> (7 - self.pos % 8) & 1);
            self.pos += 1;
        }
        Ok(v)
    }

    fn mit_vorzeichen(&mut self, n: u32) -> Result<i64, String> {
        if n == 0 {
            return Ok(0);
        }
        let v = self.lies(n)?;
        Ok(((v << (64 - n)) as i64) >> (64 - n))
    }

    /// Nullen bis zur ersten Eins.
    fn unaer(&mut self) -> Result<u64, String> {
        let mut n = 0;
        while self.lies(1)? == 0 {
            n += 1;
        }
        Ok(n)
    }

    fn auf_byte(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }

    fn bytes(&self) -> usize {
        self.pos / 8
    }
}

/// Ein Rahmen: wie viele Bytes er belegt, und ob danach nichts mehr kommt.
fn rahmen(b: &[u8], info: &Streaminfo, kanaele: &mut [Vec<i32>]) -> Result<(usize, bool), String> {
    if b.iter().all(|&x| x == 0) {
        return Ok((b.len(), true));
    }
    let mut r = Bits::neu(b);
    if r.lies(14)? != 0x3ffe {
        return Err("FLAC: kein Rahmenanfang".into());
    }
    r.lies(2)?;
    let groessencode = r.lies(4)?;
    let ratencode = r.lies(4)?;
    let anordnung = r.lies(4)?;
    let breitencode = r.lies(3)?;
    r.lies(1)?;
    // Die Nummer, UTF-8-artig: Zahl der fuehrenden Einsen ist die Laenge.
    let erstes = r.lies(8)?;
    let folge = (erstes as u8).leading_ones();
    if folge == 1 || folge > 7 {
        return Err("FLAC: Rahmennummer unlesbar".into());
    }
    for _ in 1..folge.max(1) {
        if r.lies(8)? >> 6 != 0b10 {
            return Err("FLAC: Rahmennummer unlesbar".into());
        }
    }
    let groesse = match groessencode {
        0 => return Err("FLAC: Blockgroesse reserviert".into()),
        1 => 192,
        2..=5 => 576 << (groessencode - 2),
        6 => r.lies(8)? as usize + 1,
        7 => r.lies(16)? as usize + 1,
        _ => 256 << (groessencode - 8),
    };
    match ratencode {
        12 => {
            r.lies(8)?;
        }
        13 | 14 => {
            r.lies(16)?;
        }
        15 => return Err("FLAC: Abtastrate ungueltig".into()),
        _ => {}
    }
    let bits = match breitencode {
        0 => info.bits,
        1 => 8,
        2 => 12,
        4 => 16,
        5 => 20,
        6 => 24,
        7 => 32,
        _ => return Err("FLAC: Probenbreite reserviert".into()),
    };
    // 📌 Gefunden vom Fuzzer am 2026-11-12: Ein Rahmen, dessen Kopf mehr Bit
    // nennt als `STREAMINFO`, ergab Proben ausserhalb der Breite, mit der
    // `lesen` skaliert, also ausserhalb von [-1, 1]. Die Norm verlangt
    // dieselbe Breite in jedem Rahmen.
    if bits != info.bits {
        return Err("FLAC: Rahmen mit anderer Probenbreite als STREAMINFO".into());
    }
    let zahl = match anordnung {
        0..=7 => anordnung as usize + 1,
        8..=10 => 2,
        _ => return Err("FLAC: Kanalanordnung reserviert".into()),
    };
    if zahl != kanaele.len() {
        return Err("FLAC: Rahmen mit anderer Kanalzahl".into());
    }
    let kopfende = r.bytes();
    if crc8(&b[..kopfende]) != r.lies(8)? as u8 {
        return Err("FLAC: Pruefsumme des Rahmenkopfs stimmt nicht".into());
    }
    let mut kan: Vec<Vec<i64>> = Vec::with_capacity(zahl);
    for k in 0..zahl {
        let seite = matches!((anordnung, k), (8, 1) | (9, 0) | (10, 1));
        kan.push(unterrahmen(&mut r, groesse, bits + u32::from(seite))?);
    }
    r.auf_byte();
    let ende = r.bytes();
    if crc16(&b[..ende]) != r.lies(16)? as u16 {
        return Err("FLAC: Pruefsumme des Rahmens stimmt nicht".into());
    }
    if anordnung >= 8 {
        let (a, s) = (&kan[0], &kan[1]);
        let (l, rr): (Vec<i64>, Vec<i64>) = match anordnung {
            8 => (a.clone(), a.iter().zip(s).map(|(l, s)| l - s).collect()),
            9 => (a.iter().zip(s).map(|(s, r)| s + r).collect(), s.clone()),
            _ => a
                .iter()
                .zip(s)
                .map(|(m, s)| {
                    let m = (m << 1) | (s & 1);
                    ((m + s) >> 1, (m - s) >> 1)
                })
                .unzip(),
        };
        kan = vec![l, rr];
    }
    let grenze = 1i64 << (bits - 1);
    for (ziel, quelle) in kanaele.iter_mut().zip(kan) {
        for v in quelle {
            if v < -grenze || v >= grenze {
                return Err("FLAC: Probe ausserhalb der Breite".into());
            }
            ziel.push(v as i32);
        }
    }
    Ok((ende + 2, false))
}

/// Ein Unterrahmen mit `bits` Bit je Probe.
fn unterrahmen(r: &mut Bits, groesse: usize, bits: u32) -> Result<Vec<i64>, String> {
    if r.lies(1)? != 0 {
        return Err("FLAC: Unterrahmen ohne Nullbit".into());
    }
    let art = r.lies(6)?;
    let ungenutzt = if r.lies(1)? == 1 { r.unaer()? as u32 + 1 } else { 0 };
    if ungenutzt >= bits {
        return Err("FLAC: mehr ungenutzte Bits als Bits".into());
    }
    let b = bits - ungenutzt;
    let mut s = match art {
        0 => vec![r.mit_vorzeichen(b)?; groesse],
        1 => (0..groesse).map(|_| r.mit_vorzeichen(b)).collect::<Result<_, _>>()?,
        8..=12 => vorhersagen(r, groesse, b, (art - 8) as usize, None)?,
        32..=63 => vorhersagen(r, groesse, b, (art - 31) as usize, Some(()))?,
        _ => return Err("FLAC: Unterrahmen reserviert".into()),
    };
    if ungenutzt > 0 {
        for v in &mut s {
            *v <<= ungenutzt;
        }
    }
    Ok(s)
}

/// `FIXED` (ohne `lpc`) oder `LPC` der Ordnung `ordnung`.
fn vorhersagen(r: &mut Bits, groesse: usize, bits: u32, ordnung: usize, lpc: Option<()>) -> Result<Vec<i64>, String> {
    if ordnung > groesse {
        return Err("FLAC: Ordnung groesser als der Block".into());
    }
    let mut s: Vec<i64> = (0..ordnung).map(|_| r.mit_vorzeichen(bits)).collect::<Result<_, _>>()?;
    let (koeff, schiebe): (Vec<i64>, u32) = match lpc {
        None => (
            match ordnung {
                0 => vec![],
                1 => vec![1],
                2 => vec![2, -1],
                3 => vec![3, -3, 1],
                _ => vec![4, -6, 4, -1],
            },
            0,
        ),
        Some(()) => {
            let genau = r.lies(4)? as u32 + 1;
            if genau == 16 {
                return Err("FLAC: Genauigkeit ungueltig".into());
            }
            let schiebe = r.mit_vorzeichen(5)?;
            if schiebe < 0 {
                return Err("FLAC: negative Verschiebung".into());
            }
            ((0..ordnung).map(|_| r.mit_vorzeichen(genau)).collect::<Result<_, _>>()?, schiebe as u32)
        }
    };
    let rest = restwerte(r, groesse, ordnung)?;
    s.reserve(groesse - ordnung);
    for (i, e) in rest.into_iter().enumerate() {
        let n = ordnung + i;
        let summe = koeff.iter().enumerate().fold(0i64, |a, (j, c)| a.wrapping_add(c.wrapping_mul(s[n - 1 - j])));
        s.push((summe >> schiebe).wrapping_add(e));
    }
    Ok(s)
}

/// Die Restwerte nach Rice, in `2^ordnung` Teilen.
fn restwerte(r: &mut Bits, groesse: usize, ordnung: usize) -> Result<Vec<i64>, String> {
    let methode = r.lies(2)?;
    let (breite, flucht) = match methode {
        0 => (4, 15),
        1 => (5, 31),
        _ => return Err("FLAC: Restkodierung reserviert".into()),
    };
    let teile = r.lies(4)? as u32;
    let je = groesse >> teile;
    if je << teile != groesse || je < ordnung && teile > 0 || (teile == 0 && groesse < ordnung) {
        return Err("FLAC: Teilung passt nicht zum Block".into());
    }
    let mut aus = Vec::with_capacity(groesse - ordnung);
    for t in 0..(1usize << teile) {
        let n = if t == 0 { je - ordnung } else { je };
        let k = r.lies(breite)? as u32;
        if k == flucht {
            let roh = r.lies(5)? as u32;
            for _ in 0..n {
                aus.push(r.mit_vorzeichen(roh)?);
            }
        } else {
            for _ in 0..n {
                let q = r.unaer()?;
                if q > 1 << 32 {
                    return Err("FLAC: Restwert unsinnig gross".into());
                }
                let v = (q << k) | r.lies(k)?;
                aus.push(((v >> 1) as i64) ^ -((v & 1) as i64));
            }
        }
    }
    Ok(aus)
}

fn crc8(b: &[u8]) -> u8 {
    b.iter().fold(0u8, |mut c, &x| {
        c ^= x;
        for _ in 0..8 {
            c = if c & 0x80 != 0 { (c << 1) ^ 0x07 } else { c << 1 };
        }
        c
    })
}

fn crc16(b: &[u8]) -> u16 {
    b.iter().fold(0u16, |mut c, &x| {
        c ^= u16::from(x) << 8;
        for _ in 0..8 {
            c = if c & 0x8000 != 0 { (c << 1) ^ 0x8005 } else { c << 1 };
        }
        c
    })
}

/// Die MD5-Summe, wie `STREAMINFO` sie nennt: die Proben verschraenkt,
/// je Probe `ceil(bits/8)` Bytes, das niedrigste zuerst.
fn md5_der_proben(kanaele: &[Vec<i32>], bits: u32) -> [u8; 16] {
    let byte = bits.div_ceil(8) as usize;
    let n = kanaele.first().map_or(0, Vec::len);
    let mut roh = Vec::with_capacity(n * kanaele.len() * byte);
    for i in 0..n {
        for k in kanaele {
            roh.extend_from_slice(&k[i].to_le_bytes()[..byte]);
        }
    }
    md5(&roh)
}

/// MD5 nach RFC 1321, nur zum Nachrechnen der Summe in `STREAMINFO`.
fn md5(nachricht: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16,
        23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    // K[i] = floor(|sin(i + 1)| · 2³²), als Tabelle aus der Norm.
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af,
        0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8,
        0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244, 0x432aff97,
        0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];
    let mut m = nachricht.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&((nachricht.len() as u64).wrapping_mul(8)).to_le_bytes());
    let mut h: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];
    for block in m.as_chunks::<64>().0 {
        let w: Vec<u32> = block.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect();
        let [mut a, mut b, mut c, mut d] = h;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let neu = b.wrapping_add(a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(w[g]).rotate_left(S[i]));
            (a, d, c, b) = (d, c, b, neu);
        }
        h = [h[0].wrapping_add(a), h[1].wrapping_add(b), h[2].wrapping_add(c), h[3].wrapping_add(d)];
    }
    let mut aus = [0u8; 16];
    for (i, x) in h.iter().enumerate() {
        aus[i * 4..i * 4 + 4].copy_from_slice(&x.to_le_bytes());
    }
    aus
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// Die Pruefvektoren aus RFC 1321, Anhang A.5.
    #[test]
    fn md5_wie_in_der_norm() {
        assert_eq!(hex(&md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(&md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hex(&md5(b"message digest")), "f96b697d7cb7938d525a2f31aaf161d0");
        let lang = b"12345678901234567890123456789012345678901234567890123456789012345678901234567890";
        assert_eq!(hex(&md5(lang)), "57edf4a22be3c955ac49da2e2107b67a");
    }

    /// Die Pruefsummen des Rahmens: CRC-8 mit 0x07, CRC-16 mit 0x8005, je
    /// ohne Spiegelung und mit null begonnen ("123456789": 0xF4 und 0xFEE8).
    #[test]
    fn die_pruefsummen_der_rahmen() {
        assert_eq!(crc8(b"123456789"), 0xf4);
        assert_eq!(crc16(b"123456789"), 0xfee8);
    }

    #[test]
    fn ein_kopf_ohne_flac_wird_abgewiesen() {
        assert!(dekodieren(b"RIFF1234WAVE").is_err());
        assert!(dekodieren(b"fLaC").is_err());
        assert!(!ist_flac(b"OggS"));
        assert!(ist_flac(b"fLaC\0\0\0\x22"));
    }

    fn datei(name: &str) -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/daten/flac").join(name)).expect(name)
    }

    /// ⛔️ **Jede Datei trifft ihre MD5-Summe**, also jede Probe: fest, ohne
    /// Vorhersage und LPC in Mono, die vier Kanalanordnungen in Stereo, und
    /// 24 Bit. Erzeugt mit ffmpeg aus einem festen Signal (Sinus und Rauschen),
    /// Rahmen zu 256 und 576 Proben.
    #[test]
    fn jede_pruefdatei_trifft_ihre_summe() {
        for (name, rate, kanaele, bits, n) in [
            ("mono16-ohne.flac", 16_000, 1, 16, 800),
            ("mono16-fest.flac", 16_000, 1, 16, 800),
            ("mono16-lpc.flac", 16_000, 1, 16, 800),
            ("stereo16-indep.flac", 44_100, 2, 16, 2205),
            ("stereo16-left_side.flac", 44_100, 2, 16, 2205),
            ("stereo16-right_side.flac", 44_100, 2, 16, 2205),
            ("stereo16-mid_side.flac", 44_100, 2, 16, 2205),
            ("stereo24.flac", 48_000, 2, 24, 2400),
        ] {
            let f = dekodieren(&datei(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!((f.rate, f.kanaele.len(), f.bits, f.kanaele[0].len()), (rate, kanaele, bits, n), "{name}");
        }
    }

    /// Dasselbe Signal, vier Kanalanordnungen: dieselben Proben.
    #[test]
    fn die_kanalanordnungen_ergeben_dieselben_proben() {
        let grund = dekodieren(&datei("stereo16-indep.flac")).expect("indep");
        for name in ["stereo16-left_side.flac", "stereo16-right_side.flac", "stereo16-mid_side.flac"] {
            assert_eq!(dekodieren(&datei(name)).expect(name), grund, "{name}");
        }
        let mono = dekodieren(&datei("mono16-ohne.flac")).expect("ohne");
        for name in ["mono16-fest.flac", "mono16-lpc.flac"] {
            assert_eq!(dekodieren(&datei(name)).expect(name), mono, "{name}");
        }
    }

    /// ⛔️ Ein verfaelschtes Byte faellt auf, ohne Panik: im Rahmen ueber die
    /// CRC, in der Summe von `STREAMINFO` ueber MD5.
    #[test]
    fn eine_verfaelschung_wird_abgewiesen() {
        let gut = datei("stereo16-mid_side.flac");
        let mut rahmen = gut.clone();
        let i = gut.len() - 300;
        rahmen[i] ^= 0x10;
        assert!(dekodieren(&rahmen).is_err(), "Rahmen verfaelscht");
        let mut summe = gut.clone();
        summe[8 + 18] ^= 1;
        assert_eq!(dekodieren(&summe).expect_err("Summe verfaelscht"), "FLAC: die MD5-Summe der Proben stimmt nicht");
        for ende in [9, 42, 100, gut.len() - 1] {
            assert!(dekodieren(&gut[..ende]).is_err(), "abgeschnitten bei {ende}");
        }
    }

    /// ⛔️ Eine erfundene Abtastrate in `STREAMINFO` wird abgewiesen (wie
    /// bei WAV, [`crate::ton::RATEN`]).
    #[test]
    fn eine_erfundene_rate_wird_abgewiesen() {
        let mut b = datei("mono16-lpc.flac");
        // Die Rate steht in den ersten 20 Bit ab Byte 10 von STREAMINFO.
        b[8 + 10] = 0;
        b[8 + 11] = 0;
        b[8 + 12] = (1 << 4) | (b[8 + 12] & 0x0f);
        assert!(dekodieren(&b).expect_err("1 Hz").contains("Abtastrate"));
    }

    /// Gemischt und skaliert wie ein WAV: Proben in [-1, 1].
    #[test]
    fn lesen_mischt_zu_einem_kanal() {
        let t = lesen(&datei("stereo24.flac")).expect("lesbar");
        assert_eq!((t.rate, t.proben.len()), (48_000, 2400));
        assert!(t.proben.iter().all(|p| p.abs() <= 1.0));
        assert!(t.proben.iter().any(|p| p.abs() > 0.1), "Pegel da");
    }

    /// ⛔️ **Verstuemmelt endet es sauber**: 3 000 Abwandlungen der
    /// Pruefdateien (gekippte Bits, auch im Kopf, und Kuerzungen) ergeben ein
    /// Ergebnis oder einen Fehler, nie eine Panik und im Debug-Bau keinen
    /// Ueberlauf.
    #[test]
    fn verstuemmelt_endet_es_sauber() {
        let dateien: Vec<Vec<u8>> = ["mono16-lpc.flac", "stereo16-mid_side.flac", "stereo24.flac"].iter().map(|n| datei(n)).collect();
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let mut zufall = |n: usize| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % n as u64) as usize
        };
        let (mut gelesen, mut abgewiesen) = (0, 0);
        for runde in 0..3_000 {
            let mut b = dateien[runde % dateien.len()].clone();
            for _ in 0..1 + zufall(3) {
                let i = if zufall(4) == 0 { 4 + zufall(60) } else { zufall(b.len()) };
                b[i] ^= 1 << zufall(8);
            }
            if zufall(5) == 0 {
                let ende = zufall(b.len());
                b.truncate(ende);
            }
            match dekodieren(&b) {
                Ok(_) => gelesen += 1,
                Err(_) => abgewiesen += 1,
            }
        }
        assert!(abgewiesen > 2_500, "gelesen {gelesen}, abgewiesen {abgewiesen}");
    }

    /// Schreibt Bits von vorn, das hoechste zuerst.
    struct Schreiber(Vec<u8>, usize);

    impl Schreiber {
        fn bits(&mut self, v: u64, n: u32) {
            for k in (0..n).rev() {
                if self.1.is_multiple_of(8) {
                    self.0.push(0);
                }
                let letztes = self.0.len() - 1;
                self.0[letztes] |= ((v >> k & 1) as u8) << (7 - self.1 % 8);
                self.1 += 1;
            }
        }
    }

    /// Eine Datei mit einem Rahmen zu 16 Bit, Mono, Blockgroesse 192, dessen
    /// einziger Unterrahmen `CONSTANT` ist und `ungenutzt` Bits als ungenutzt
    /// meldet. Kopf und Rahmen mit gueltiger Pruefsumme.
    fn mit_ungenutzten_bits(ungenutzt: u32) -> Vec<u8> {
        let mut info = Schreiber(Vec::new(), 0);
        info.bits(192, 16);
        info.bits(192, 16);
        info.bits(0, 24);
        info.bits(0, 24);
        info.bits(16_000, 20);
        info.bits(0, 3);
        info.bits(15, 5);
        info.bits(192, 36);
        info.bits(0, 64);
        info.bits(0, 64);
        let mut b = b"fLaC".to_vec();
        b.extend_from_slice(&[0x80, 0, 0, 34]);
        b.extend_from_slice(&info.0);
        let mut r = Schreiber(Vec::new(), 0);
        r.bits(0x3ffe, 14);
        r.bits(0, 2);
        r.bits(1, 4);
        r.bits(0, 4);
        r.bits(0, 4);
        r.bits(4, 3);
        r.bits(0, 1);
        r.bits(0, 8);
        let k = crc8(&r.0);
        r.bits(u64::from(k), 8);
        r.bits(0, 1);
        r.bits(0, 6);
        r.bits(1, 1);
        r.bits(0, ungenutzt - 1);
        r.bits(1, 1);
        r.bits(5, 16u32.saturating_sub(ungenutzt).max(1));
        while !r.1.is_multiple_of(8) {
            r.bits(0, 1);
        }
        let k = crc16(&r.0);
        r.bits(u64::from(k), 16);
        b.extend_from_slice(&r.0);
        b
    }

    /// ⛔️ Mehr ungenutzte Bits, als die Probe hat, ist ein Fehler und kein
    /// Unterlauf; mit wenigen ungenutzten Bits liest sich derselbe Rahmen.
    #[test]
    fn ungenutzte_bits_ueber_der_breite_werden_abgewiesen() {
        let f = dekodieren(&mit_ungenutzten_bits(3)).expect("drei ungenutzte Bits");
        assert_eq!(f.kanaele[0], vec![5 << 3; 192]);
        assert_eq!(dekodieren(&mit_ungenutzten_bits(16)).expect_err("16 von 16"), "FLAC: mehr ungenutzte Bits als Bits");
        assert!(dekodieren(&mit_ungenutzten_bits(20)).is_err());
    }
}
