//! Ternaere Gewichte: je Gewicht einer der Werte -1, 0, +1, dazu ein
//! ganzzahliger Betrag je Gruppe von [`GRUPPE`] Eingaengen.
//!
//! # Wozu
//!
//! Ein Modell, dessen Matrizen auf {-s, 0, +s} je 128er-Gruppe
//! nachtrainiert sind, laeuft auch als int8 durch den Rechenpfad: Jede
//! Gruppe ist dann {-m, 0, +m} mit einem ganzen `m`. Gelesen werden dabei
//! aber acht Bit je Gewicht, wo zwei genuegen. Gemessen am 2026-09-28 an
//! einem ternaeren Qwen3-8B im int8-Pfad: 8,8 GB Artefakt, dieselbe
//! Qualitaet wie das int8-Original. Hier liegt also nur Platz und Zeit,
//! keine Genauigkeit.
//!
//! # Das Format
//!
//! Je Gewicht ein 2-Bit-Code `c = t + 1`, also 0 fuer -1, 1 fuer 0 und 2
//! fuer +1. Je Gruppe [`BYTES_JE_GRUPPE`] Byte in zwei Bloecken zu 16
//! Byte fuer je 64 Gewichte; **Byte `j` eines Blocks traegt in den Bits
//! `2s` und `2s+1` den Code des Gewichts `16s + j`** dieses Blocks. Dazu
//! ein `i16` je Gruppe, der Betrag. Zeile fuer Zeile, Gruppe fuer Gruppe.
//!
//! Der Wert eines Gewichts ist `betrag * (c - 1)`. ⚑ **Code 3 heisst +2.**
//! [`packen`] erzeugt ihn nie; festgelegt ist er trotzdem, damit jede
//! Umsetzung dieselbe Zahl liefert, auch fuer eine Datei, die nicht aus
//! dem Packer kommt.
//!
//! ⚑ **Warum verschraenkt nach Position.** Ein Vektor aus 16 Bytes ergibt
//! so mit einem Schieben und einem UND die Codes von 16 aufeinander
//! folgenden Gewichten, je einer in seiner Lane, ohne Tabellenabfrage.
//!
//! 📌 **Die erste Fassung (2026-09-28) speicherte zwei Bitmasken** (plus
//! und minus) je Gruppe. Gleich gross, aber teuer zu entpacken: rund 16,
//! nach einem Umbau 9 Vektorbefehle je 16 Gewichte, gegen 6 im int8-Weg.
//! Gemessen an einer Matrix 12 288 x 4 096 auf allen Kernen: int8 176 G
//! Gewichte/s (am Speicher), die Masken 201 G/s (am Rechnen, bei 53 GB/s
//! gelesen), und im ganzen 8B **kein** Gewinn im Decode. Ein Format, das
//! weniger liest, aber mehr rechnet, hat nur die Grenze verschoben.
//!
//! # ⚑ Warum das bitgleich zum int8-Weg ist, per Konstruktion
//!
//! Der int8-Weg rechnet `sum_i w_i * x_i` exakt in i64. Hier ist
//! `w_i = m_g * t_i` mit `t_i` aus {-1, 0, +1}, also
//! `sum_g m_g * (sum_{i in g} t_i * x_i)`: **dieselbe ganze Zahl**, nur
//! anders geklammert. Ganzzahlige Addition und Multiplikation sind
//! exakt, also kann die Klammerung nichts aendern. Ein Artefakt, das aus
//! einem int8-Artefakt umgepackt ist, liefert deshalb dieselben Logits,
//! Bit fuer Bit, und was am int8-Artefakt gemessen wurde, gilt fuer das
//! gepackte.
//!
//! Dasselbe gilt fuer einen int16-Kopf: Seine Gruppen sind {-M, 0, +M}
//! mit `M` bis 32 767. Deshalb ist der Betrag ein `i16` und kein `i8`.
//!
//! # Wertebereich
//!
//! Eine Gruppensumme hat hoechstens 128 Summanden vom Betrag bis 2^16
//! (Code 3), liegt also unter 2^23 und sicher in i32. Mal dem Betrag
//! (unter 2^15) bleibt sie unter 2^38, und ueber alle Gruppen einer Zeile
//! wird in i64 summiert.

/// Gewichte je Gruppe, also je Betrag.
pub const GRUPPE: usize = 128;

/// Bytes je Gruppe: zwei Bit je Gewicht.
pub const BYTES_JE_GRUPPE: usize = GRUPPE / 4;

/// Gewichte je Block von 16 Bytes.
const BLOCK: usize = 64;

/// Eine Aktivierung, aufbereitet fuer ternaere Zeilen.
///
/// ⚑ **Warum aufbereiten.** Die vektorisierte Fassung multipliziert die
/// Codes als 8-Bit-Lanes, 16 Gewichte je Befehlspaar. Dafuer wird jedes
/// `x` exakt in zwei Bytes zerlegt, `x = 256 * hoch + tief` mit
/// `hoch = x >> 8` (vorzeichenbehaftet) und `tief = x & 0xFF` (ohne
/// Vorzeichen). Mit `t = c - 1` wird daraus
///
/// `sum t*x = 256 * sum c*hoch + sum c*tief - sum x`,
///
/// und `sum x` je Gruppe haengt nur an der Eingabe, nicht an der Zeile.
/// Keine Rundung, nur eine andere Klammerung derselben ganzen Zahl.
///
/// ⚑ **Einmal je Eingabe, nicht je Zeile.** Eine Matrix mit 12 288 Zeilen
/// liest dieselbe Aktivierung 12 288-mal; zerlegt und summiert wird sie
/// einmal. Ohne vektorisierte Fassung ([`crate::dot::VEKTORISIERT`]) wird
/// nichts vorbereitet, denn die skalare Fassung rechnet mit `x`.
pub struct Eingabe<'a> {
    x: &'a [i16],
    hoch: Vec<i8>,
    tief: Vec<u8>,
    /// `sum x` je Gruppe von 128.
    summen: Vec<i64>,
}

impl<'a> Eingabe<'a> {
    /// Fuer eine ternaere Matrix.
    pub fn neu(x: &'a [i16]) -> Self {
        if !crate::dot::VEKTORISIERT {
            return Self::roh(x);
        }
        let hoch = x.iter().map(|&v| (v >> 8) as i8).collect();
        let tief = x.iter().map(|&v| (v & 0xFF) as u8).collect();
        let summen = x.chunks(GRUPPE).map(|g| g.iter().map(|&v| i64::from(v)).sum()).collect();
        Eingabe { x, hoch, tief, summen }
    }

    /// Ohne Vorbereitung, fuer eine int8-Matrix, die nur `x` liest.
    pub fn roh(x: &'a [i16]) -> Self {
        Eingabe { x, hoch: Vec::new(), tief: Vec::new(), summen: Vec::new() }
    }

    pub fn x(&self) -> &'a [i16] {
        self.x
    }
}

/// Eine ternaere Matrix, gelesen aus Codes und Betraegen, die jemand
/// anderes haelt (ein Abbild der Artefaktdatei oder ein Test).
#[derive(Clone, Copy, Debug)]
pub struct Ternaermatrix<'a> {
    muster: &'a [u8],
    betraege: &'a [i16],
    spalten: usize,
}

impl<'a> Ternaermatrix<'a> {
    /// Prueft die Laengen gegen die Form. ⚑ **Hier und nur hier**: Die
    /// vektorisierte Fassung liest ohne Grenzpruefung und verlaesst sich
    /// darauf.
    pub fn neu(muster: &'a [u8], betraege: &'a [i16], zeilen: usize, spalten: usize) -> Result<Self, String> {
        if spalten == 0 || spalten % GRUPPE != 0 {
            return Err(format!("ternaere Matrix: {spalten} Spalten sind kein Vielfaches von {GRUPPE}"));
        }
        let gruppen = zeilen
            .checked_mul(spalten / GRUPPE)
            .ok_or_else(|| format!("ternaere Matrix: {zeilen} x {spalten} ist zu gross"))?;
        if betraege.len() != gruppen {
            return Err(format!(
                "ternaere Matrix: {} Betraege, aber {zeilen} x {spalten} braucht {gruppen}",
                betraege.len()
            ));
        }
        if muster.len() != gruppen * BYTES_JE_GRUPPE {
            return Err(format!(
                "ternaere Matrix: {} Musterbytes, aber {zeilen} x {spalten} braucht {}",
                muster.len(),
                gruppen * BYTES_JE_GRUPPE
            ));
        }
        Ok(Ternaermatrix { muster, betraege, spalten })
    }

    pub fn zeilen(&self) -> usize {
        self.betraege.len() / (self.spalten / GRUPPE)
    }

    pub fn spalten(&self) -> usize {
        self.spalten
    }

    /// `sum_i w[z][i] * x[i]`, exakt, dieselbe Zahl wie `dot_i8_i16` ueber
    /// die entpackte Zeile; `e` aus [`Eingabe::neu`].
    ///
    /// ⚠️ **`x` muss mindestens eine Zeile lang sein**, sonst Panik. Der
    /// int8-Weg rechnet still ueber das kuerzere Ende; hier soll eine
    /// falsche Laenge auffallen und nicht eine schlechte Zahl werden.
    #[inline]
    pub fn zeile_mal_mit(&self, z: usize, e: &Eingabe<'_>) -> i64 {
        assert!(
            e.x.len() >= self.spalten,
            "ternaere Zeile: Eingabe mit {} Werten, die Zeile hat {}",
            e.x.len(),
            self.spalten
        );
        let n = self.spalten;
        let g = n / GRUPPE;
        let vorbereitet = !e.hoch.is_empty();
        crate::dot::ternaer_zeile(
            &self.muster[z * g * BYTES_JE_GRUPPE..(z + 1) * g * BYTES_JE_GRUPPE],
            &self.betraege[z * g..(z + 1) * g],
            &e.x[..n],
            if vorbereitet { &e.hoch[..n] } else { &[] },
            if vorbereitet { &e.tief[..n] } else { &[] },
            if vorbereitet { &e.summen[..g] } else { &[] },
        )
    }

    /// Wie [`Self::zeile_mal_mit`], mit einer Vorbereitung je Aufruf. Fuer
    /// Pruefungen; im Rechenpfad wird einmal je Eingabe vorbereitet.
    pub fn zeile_mal(&self, z: usize, x: &[i16]) -> i64 {
        self.zeile_mal_mit(z, &Eingabe::neu(x))
    }

    /// Die Zeile `z` als ganze Zahlen, `betrag * (c - 1)`. Fuer
    /// Pruefungen und Werkzeuge, nicht fuer den Rechenpfad.
    pub fn zeile_entpacken(&self, z: usize) -> Vec<i32> {
        let g = self.spalten / GRUPPE;
        let mut aus = Vec::with_capacity(self.spalten);
        for gruppe in z * g..(z + 1) * g {
            let m = &self.muster[gruppe * BYTES_JE_GRUPPE..(gruppe + 1) * BYTES_JE_GRUPPE];
            let betrag = i32::from(self.betraege[gruppe]);
            for i in 0..GRUPPE {
                aus.push(betrag * trit(m, i));
            }
        }
        aus
    }
}

/// Wo der Code des Gewichts `i` einer Gruppe steht: Byte und Bitversatz.
#[inline]
fn stelle(i: usize) -> (usize, usize) {
    let (block, rest) = (i / BLOCK, i % BLOCK);
    (block * 16 + rest % 16, 2 * (rest / 16))
}

/// Der Wert `c - 1` des Gewichts `i` einer Gruppe.
#[inline]
fn trit(m: &[u8], i: usize) -> i32 {
    let (byte, versatz) = stelle(i);
    i32::from((m[byte] >> versatz) & 3) - 1
}

/// Skalare Referenzfassung, der numerische Vertrag.
///
/// Vorausgesetzt (geprueft in [`crate::dot::ternaer_zeile`]):
/// `muster.len() == betraege.len() * BYTES_JE_GRUPPE` und
/// `x.len() == betraege.len() * GRUPPE`.
pub fn zeile_skalar(muster: &[u8], betraege: &[i16], x: &[i16]) -> i64 {
    let mut gesamt: i64 = 0;
    for (g, &betrag) in betraege.iter().enumerate() {
        let m = &muster[g * BYTES_JE_GRUPPE..(g + 1) * BYTES_JE_GRUPPE];
        let xs = &x[g * GRUPPE..(g + 1) * GRUPPE];
        let mut summe: i64 = 0;
        for (i, &xi) in xs.iter().enumerate() {
            summe += i64::from(trit(m, i)) * i64::from(xi);
        }
        gesamt += summe * i64::from(betrag);
    }
    gesamt
}

#[cfg(all(feature = "cpu-simd", target_arch = "aarch64"))]
pub(crate) mod neon {
    use super::{BYTES_JE_GRUPPE, GRUPPE};
    use std::arch::aarch64::*;

    /// Je Block von 16 Bytes vier Vektoren zu 16 Codes: `B & 3`,
    /// `(B >> 2) & 3`, `(B >> 4) & 3`, `B >> 6`. Jeder gegen `hoch`
    /// (`vmlal_s8`, der Code ist 0 bis 3 und damit auch als `i8` richtig)
    /// und gegen `tief` (`vmlal_u8`). Je Gruppe dann
    /// `256 * sum c*hoch + sum c*tief - sum x`, siehe [`super::Eingabe`].
    ///
    /// **Wertebereich:** Jede Lane der vier Akkumulatoren bekommt je
    /// Gruppe acht Produkte. Mit `hoch` hoechstens 3 * 128 = 384, also
    /// 3072 in `i16`; mit `tief` hoechstens 3 * 255 = 765, also 6120 in
    /// `u16`. Zwei Akkumulatoren zusammen: 6144 und 12 240, beide passen.
    ///
    /// # Sicherheit
    ///
    /// Der Aufrufer sichert `muster.len() == betraege.len() * 32`,
    /// `hoch.len() == tief.len() == betraege.len() * 128` und
    /// `summen.len() == betraege.len()` zu.
    #[inline]
    pub unsafe fn zeile(muster: &[u8], betraege: &[i16], hoch: &[i8], tief: &[u8], summen: &[i64]) -> i64 {
        let drei = vdupq_n_u8(3);
        let mut gesamt: i64 = 0;
        for (g, &betrag) in betraege.iter().enumerate() {
            let m = muster.as_ptr().add(g * BYTES_JE_GRUPPE);
            let h = hoch.as_ptr().add(g * GRUPPE);
            let t = tief.as_ptr().add(g * GRUPPE);
            let mut h0 = vdupq_n_s16(0);
            let mut h1 = vdupq_n_s16(0);
            let mut t0 = vdupq_n_u16(0);
            let mut t1 = vdupq_n_u16(0);
            for block in 0..2 {
                let b = vld1q_u8(m.add(16 * block));
                let codes = [
                    vandq_u8(b, drei),
                    vandq_u8(vshrq_n_u8::<2>(b), drei),
                    vandq_u8(vshrq_n_u8::<4>(b), drei),
                    vshrq_n_u8::<6>(b),
                ];
                for (s, c) in codes.iter().enumerate() {
                    let aus = 64 * block + 16 * s;
                    let hv = vld1q_s8(h.add(aus));
                    let tv = vld1q_u8(t.add(aus));
                    let cs = vreinterpretq_s8_u8(*c);
                    h0 = vmlal_s8(h0, vget_low_s8(cs), vget_low_s8(hv));
                    h1 = vmlal_high_s8(h1, cs, hv);
                    t0 = vmlal_u8(t0, vget_low_u8(*c), vget_low_u8(tv));
                    t1 = vmlal_high_u8(t1, *c, tv);
                }
            }
            let s_hoch = i64::from(vaddlvq_s16(vaddq_s16(h0, h1)));
            let s_tief = i64::from(vaddlvq_u16(vaddq_u16(t0, t1)));
            gesamt += (256 * s_hoch + s_tief - *summen.get_unchecked(g)) * i64::from(betrag);
        }
        gesamt
    }
    /// **Dieselbe Rechnung mit `sdot` und `udot`**: je 16 Gewichte zwei
    /// Befehle statt vier `vmlal`, und die Summen laufen gleich in 32 Bit.
    ///
    /// ⚑ **Ueber `asm!`**, weil `vdotq_s32` in stabilem Rust noch nicht
    /// freigegeben ist (2026-09-28, rustc 1.97). Die Befehle gehoeren zur
    /// Erweiterung `dotprod`; gewaehlt wird diese Fassung nur, wenn die
    /// Maschine sie hat ([`mit_dot`]). Jeder Apple-Prozessor hat sie,
    /// aeltere ARM-Kerne unter Linux nicht, und dort bleibt [`zeile`].
    ///
    /// **Wertebereich:** Eine Lane bekommt je Gruppe 32 Produkte, mit
    /// `hoch` hoechstens 3 * 128, mit `tief` hoechstens 3 * 255; beides
    /// weit innerhalb von 32 Bit.
    ///
    /// # Sicherheit
    ///
    /// Wie [`zeile`], und die Maschine muss `dotprod` haben.
    #[target_feature(enable = "dotprod")]
    pub unsafe fn zeile_dot(muster: &[u8], betraege: &[i16], hoch: &[i8], tief: &[u8], summen: &[i64]) -> i64 {
        let drei = vdupq_n_u8(3);
        let mut gesamt: i64 = 0;
        for (g, &betrag) in betraege.iter().enumerate() {
            let m = muster.as_ptr().add(g * BYTES_JE_GRUPPE);
            let h = hoch.as_ptr().add(g * GRUPPE);
            let t = tief.as_ptr().add(g * GRUPPE);
            let mut sh0 = vdupq_n_s32(0);
            let mut sh1 = vdupq_n_s32(0);
            let mut st0 = vdupq_n_u32(0);
            let mut st1 = vdupq_n_u32(0);
            for block in 0..2 {
                let b = vld1q_u8(m.add(16 * block));
                let codes = [
                    vandq_u8(b, drei),
                    vandq_u8(vshrq_n_u8::<2>(b), drei),
                    vandq_u8(vshrq_n_u8::<4>(b), drei),
                    vshrq_n_u8::<6>(b),
                ];
                for (s, c) in codes.iter().enumerate() {
                    let aus = 64 * block + 16 * s;
                    let hv = vld1q_s8(h.add(aus));
                    let tv = vld1q_u8(t.add(aus));
                    // Zwei Ketten je Summe, damit ein Befehl nicht auf den
                    // vorigen wartet.
                    if s % 2 == 0 {
                        sh0 = sdot(sh0, vreinterpretq_s8_u8(*c), hv);
                        st0 = udot(st0, *c, tv);
                    } else {
                        sh1 = sdot(sh1, vreinterpretq_s8_u8(*c), hv);
                        st1 = udot(st1, *c, tv);
                    }
                }
            }
            let s_hoch = i64::from(vaddvq_s32(vaddq_s32(sh0, sh1)));
            let s_tief = i64::from(vaddvq_u32(vaddq_u32(st0, st1)));
            gesamt += (256 * s_hoch + s_tief - *summen.get_unchecked(g)) * i64::from(betrag);
        }
        gesamt
    }

    #[inline]
    #[target_feature(enable = "dotprod")]
    unsafe fn sdot(acc: int32x4_t, a: int8x16_t, b: int8x16_t) -> int32x4_t {
        let mut r = acc;
        std::arch::asm!(
            "sdot {r:v}.4s, {a:v}.16b, {b:v}.16b",
            r = inout(vreg) r,
            a = in(vreg) a,
            b = in(vreg) b,
            options(pure, nomem, nostack, preserves_flags)
        );
        r
    }

    #[inline]
    #[target_feature(enable = "dotprod")]
    unsafe fn udot(acc: uint32x4_t, a: uint8x16_t, b: uint8x16_t) -> uint32x4_t {
        let mut r = acc;
        std::arch::asm!(
            "udot {r:v}.4s, {a:v}.16b, {b:v}.16b",
            r = inout(vreg) r,
            a = in(vreg) a,
            b = in(vreg) b,
            options(pure, nomem, nostack, preserves_flags)
        );
        r
    }

    /// Hat diese Maschine `sdot`/`udot`? Die Standardbibliothek fragt
    /// einmal und merkt es sich.
    #[inline]
    pub fn mit_dot() -> bool {
        std::arch::is_aarch64_feature_detected!("dotprod")
    }

}

/// Ergebnis von [`packen`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gepackt {
    pub muster: Vec<u8>,
    pub betraege: Vec<i16>,
}

/// Packt eine flache Matrix (Zeile fuer Zeile, `spalten` je Zeile), wenn
/// **jede** Gruppe ternaer ist: alle Werte ungleich null haben denselben
/// Betrag, und der passt in `i16`.
///
/// ⚑ **Kein Runden, kein Naehern.** Eine Gruppe, die nicht ternaer ist,
/// ist ein Fehler mit Zeile und Gruppe, denn das Versprechen dieses
/// Formats ist Bitgleichheit mit der Quelle. Eine Gruppe aus lauter
/// Nullen bekommt den Betrag null.
pub fn packen<T: Copy + Into<i32>>(werte: &[T], spalten: usize) -> Result<Gepackt, String> {
    if spalten == 0 || spalten % GRUPPE != 0 {
        return Err(format!("packen: {spalten} Spalten sind kein Vielfaches von {GRUPPE}"));
    }
    if werte.len() % spalten != 0 {
        return Err(format!("packen: {} Werte sind keine ganzen Zeilen zu {spalten}", werte.len()));
    }
    let je_zeile = spalten / GRUPPE;
    let gruppen = werte.len() / GRUPPE;
    let mut muster = vec![0u8; gruppen * BYTES_JE_GRUPPE];
    let mut betraege = Vec::with_capacity(gruppen);
    for (g, gruppe) in werte.chunks_exact(GRUPPE).enumerate() {
        let mut betrag: i32 = 0;
        for &w in gruppe {
            let v: i32 = w.into();
            if v == 0 {
                continue;
            }
            let a = v.abs();
            if betrag == 0 {
                betrag = a;
            } else if a != betrag {
                return Err(format!(
                    "packen: Zeile {}, Gruppe {}: Betraege {betrag} und {a}, die Gruppe ist nicht ternaer",
                    g / je_zeile,
                    g % je_zeile
                ));
            }
        }
        if betrag > i32::from(i16::MAX) {
            return Err(format!(
                "packen: Zeile {}, Gruppe {}: Betrag {betrag} passt nicht in i16",
                g / je_zeile,
                g % je_zeile
            ));
        }
        let m = &mut muster[g * BYTES_JE_GRUPPE..(g + 1) * BYTES_JE_GRUPPE];
        for (i, &w) in gruppe.iter().enumerate() {
            let v: i32 = w.into();
            let code: u8 = match v.signum() {
                1 => 2,
                -1 => 0,
                _ => 1,
            };
            let (byte, versatz) = stelle(i);
            m[byte] |= code << versatz;
        }
        betraege.push(betrag as i16);
    }
    Ok(Gepackt { muster, betraege })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dot::dot_i8_i16;

    // ⚠️ Kein `skalar_erzwingen` in diesen Tests: Der Schalter gilt fuer
    // den ganzen Prozess, und ein Test in `dot.rs` liest ihn, waehrend
    // diese parallel laufen. Die skalare Fassung wird direkt gerufen.

    fn zufall(n: usize, saat: u64) -> Vec<u64> {
        let mut x = saat | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x
            })
            .collect()
    }

    /// Eine ternaere int8-Matrix mit einem Betrag je Gruppe aus `betraege`
    /// und rund einem Drittel Nullen.
    fn ternaere_matrix(zeilen: usize, spalten: usize, saat: u64, betraege: &[i32]) -> Vec<i32> {
        let r = zufall(zeilen * spalten + zeilen * spalten / GRUPPE, saat);
        let mut w = Vec::with_capacity(zeilen * spalten);
        for g in 0..zeilen * spalten / GRUPPE {
            let betrag = betraege[(r[zeilen * spalten + g] % betraege.len() as u64) as usize];
            for i in 0..GRUPPE {
                let t = (r[g * GRUPPE + i] % 3) as i32 - 1;
                w.push(betrag * t);
            }
        }
        w
    }

    fn aktivierung(n: usize, saat: u64) -> Vec<i16> {
        zufall(n, saat).into_iter().map(|v| v as u16 as i16).collect()
    }

    /// **Gepackt ist dieselbe Zahl wie int8, auf jedem Weg.** Mit den
    /// Raendern: Aktivierungen ueber den ganzen i16-Bereich (auch -32768,
    /// wo ein Vorzeichenwechsel in i16 ueberliefe), Betraege 1 und 127.
    #[test]
    fn gepackt_ist_dieselbe_zahl_wie_int8() {
        for (zeilen, spalten) in [(1usize, 128usize), (3, 256), (7, 1024), (2, 4096)] {
            let w = ternaere_matrix(zeilen, spalten, 5 + spalten as u64, &[1, 23, 64, 127]);
            let w8: Vec<i8> = w.iter().map(|&v| v as i8).collect();
            let p = packen(&w8, spalten).expect("ternaer");
            let t = Ternaermatrix::neu(&p.muster, &p.betraege, zeilen, spalten).unwrap();
            let mut x = aktivierung(spalten, 99);
            x[0] = i16::MIN;
            x[spalten - 1] = i16::MAX;
            for z in 0..zeilen {
                let soll = dot_i8_i16(&w8[z * spalten..(z + 1) * spalten], &x);
                assert_eq!(t.zeile_mal(z, &x), soll, "{zeilen}x{spalten}, Zeile {z}");
                assert_eq!(
                    zeile_skalar(
                        &p.muster[z * spalten / 4..(z + 1) * spalten / 4],
                        &p.betraege[z * spalten / GRUPPE..(z + 1) * spalten / GRUPPE],
                        &x
                    ),
                    soll
                );
                let entpackt: Vec<i32> = t.zeile_entpacken(z);
                assert_eq!(entpackt, w[z * spalten..(z + 1) * spalten].to_vec());
            }
        }
    }

    /// **Der schlimmste Fall des Wertebereichs:** jede Aktivierung
    /// -32768, jedes Gewicht derselben Seite, Betrag 32767. Eine Gruppe
    /// ist dann 2^22 gross, mal Betrag knapp 2^37.
    #[test]
    fn der_groesste_wert_laeuft_nicht_ueber() {
        let spalten = 1024;
        for vorzeichen in [1i32, -1] {
            let w: Vec<i16> = vec![(vorzeichen * 32767) as i16; spalten];
            let p = packen(&w, spalten).unwrap();
            let t = Ternaermatrix::neu(&p.muster, &p.betraege, 1, spalten).unwrap();
            let x = vec![i16::MIN; spalten];
            let soll: i64 = w.iter().map(|&v| i64::from(v) * i64::from(i16::MIN)).sum();
            assert_eq!(t.zeile_mal(0, &x), soll);
            assert_eq!(zeile_skalar(&p.muster, &p.betraege, &x), soll);
        }
    }

    /// **Ein int16-Kopf passt hinein**, mit Betraegen bis 32767.
    #[test]
    fn ein_int16_kopf_ist_ebenso_bitgleich() {
        let (zeilen, spalten) = (5usize, 512usize);
        let w = ternaere_matrix(zeilen, spalten, 31, &[300, 4097, 32767]);
        let w16: Vec<i16> = w.iter().map(|&v| v as i16).collect();
        let p = packen(&w16, spalten).unwrap();
        let t = Ternaermatrix::neu(&p.muster, &p.betraege, zeilen, spalten).unwrap();
        let x = aktivierung(spalten, 7);
        for z in 0..zeilen {
            let soll: i64 = w16[z * spalten..(z + 1) * spalten]
                .iter()
                .zip(&x)
                .map(|(&a, &b)| i64::from(a) * i64::from(b))
                .sum();
            assert_eq!(t.zeile_mal(z, &x), soll);
        }
    }

    /// **Was nicht ternaer ist, wird nicht gepackt**, und die Meldung
    /// nennt Zeile und Gruppe.
    #[test]
    fn eine_gemischte_gruppe_wird_abgewiesen() {
        let mut w = vec![0i8; 2 * 256];
        w[256 + 128 + 3] = 5;
        w[256 + 128 + 9] = -6;
        let fehler = packen(&w, 256).unwrap_err();
        assert!(fehler.contains("Zeile 1, Gruppe 1"), "{fehler}");
        let breit = vec![i16::MIN; 128];
        assert!(packen(&breit, 128).unwrap_err().contains("passt nicht in i16"));
        assert!(packen(&[1i8; 100], 100).is_err());
    }

    /// **Jeder lineare Kern rechnet ternaer dasselbe wie int8**: einzeln,
    /// mit einer Skala je Kanal, gestapelt (ueber die Kachelgrenze von
    /// acht Eingaben) und gebuendelt, dort gemischt mit int8-Teilen.
    #[test]
    fn jeder_lineare_kern_rechnet_dasselbe() {
        use crate::linear::*;
        let (zeilen, spalten) = (37usize, 384usize);
        let w = ternaere_matrix(zeilen, spalten, 17, &[2, 50, 127]);
        let w8: Vec<i8> = w.iter().map(|&v| v as i8).collect();
        let p = packen(&w8, spalten).unwrap();
        let t: Gewichtsmatrix<'_> = Ternaermatrix::neu(&p.muster, &p.betraege, zeilen, spalten).unwrap().into();
        let shifts: Vec<u8> = (0..zeilen).map(|z| 6 + (z % 3) as u8).collect();
        let je_kanal: Vec<u8> = (0..zeilen).map(|z| 4 + (z % 5) as u8).collect();
        let eingaben: Vec<Vec<i16>> = (0..11).map(|i| aktivierung(spalten, 40 + i)).collect();
        let scheiben: Vec<&[i16]> = eingaben.iter().map(|v| v.as_slice()).collect();

        for x in &eingaben {
            let soll = linear_w8a16(x, &w8, spalten, &shifts, 3, 5);
            assert!(soll.iter().any(|&v| v != 0 && v != i16::MAX && v != i16::MIN), "nur Nullen oder Saettigung");
            assert_eq!(linear_matrix(x, t, spalten, &shifts, 3, 5), soll);
            assert_eq!(
                linear_matrix_pc(x, t, spalten, &shifts, 3, &je_kanal),
                linear_w8a16_pc(x, &w8, spalten, &shifts, 3, &je_kanal)
            );
        }
        assert_eq!(
            linear_matrix_stapel(&scheiben, t, spalten, &shifts, 3, 5),
            linear_w8a16_stapel(&scheiben, &w8, spalten, &shifts, 3, 5)
        );
        assert_eq!(
            linear_matrix_pc_stapel(&scheiben, t, spalten, &shifts, 3, &je_kanal),
            linear_w8a16_pc_stapel(&scheiben, &w8, spalten, &shifts, 3, &je_kanal)
        );
        fn teil<'a>(w: Gewichtsmatrix<'a>, x: &'a [i16], shifts: &'a [u8], aus: Ausgangsskala<'a>) -> Buendelteil<'a> {
            Buendelteil { w, x, in_features: x.len(), w_shifts: shifts, act_frac_bits: 3, aus }
        }
        let int8 = Gewichtsmatrix::Int8(&w8);
        let gemischt = linear_w8a16_buendel(&[
            teil(t, &eingaben[0], &shifts, Ausgangsskala::Eine(5)),
            teil(int8, &eingaben[1], &shifts, Ausgangsskala::JeZeile(&je_kanal)),
            teil(t, &eingaben[2], &shifts, Ausgangsskala::JeZeile(&je_kanal)),
        ]);
        let nur_int8 = linear_w8a16_buendel(&[
            teil(int8, &eingaben[0], &shifts, Ausgangsskala::Eine(5)),
            teil(int8, &eingaben[1], &shifts, Ausgangsskala::JeZeile(&je_kanal)),
            teil(int8, &eingaben[2], &shifts, Ausgangsskala::JeZeile(&je_kanal)),
        ]);
        assert_eq!(gemischt, nur_int8);
    }

    /// **Beide vektorisierten Fassungen rechnen dieselbe Zahl wie die
    /// skalare**, jede ausdruecklich gerufen: Welche der Rechenpfad nimmt,
    /// haengt an der Maschine, und die andere bliebe sonst ungeprueft.
    #[cfg(all(feature = "cpu-simd", target_arch = "aarch64"))]
    #[test]
    fn beide_vektorfassungen_sind_die_skalare() {
        let (zeilen, spalten) = (9usize, 1024usize);
        let w = ternaere_matrix(zeilen, spalten, 77, &[1, 99, 127]);
        let mut w16: Vec<i16> = w.iter().map(|&v| v as i16).collect();
        // Ein paar Gruppen mit grossem Betrag, wie in einem int16-Kopf.
        for v in w16.iter_mut().take(256) {
            *v = v.signum() * 32767;
        }
        let p = packen(&w16, spalten).unwrap();
        let mut x = aktivierung(spalten, 5);
        x[3] = i16::MIN;
        x[700] = i16::MAX;
        let e = Eingabe::neu(&x);
        let g = spalten / GRUPPE;
        for z in 0..zeilen {
            let m = &p.muster[z * g * BYTES_JE_GRUPPE..(z + 1) * g * BYTES_JE_GRUPPE];
            let b = &p.betraege[z * g..(z + 1) * g];
            let soll = zeile_skalar(m, b, &x);
            assert_eq!(unsafe { neon::zeile(m, b, &e.hoch, &e.tief, &e.summen) }, soll, "vmlal, Zeile {z}");
            if neon::mit_dot() {
                assert_eq!(unsafe { neon::zeile_dot(m, b, &e.hoch, &e.tief, &e.summen) }, soll, "sdot, Zeile {z}");
            }
        }
        assert!(neon::mit_dot() || !cfg!(target_os = "macos"), "jeder Apple-Prozessor hat dotprod");
    }

    /// **Code 3 heisst +2**, skalar wie vektorisiert; der Packer erzeugt
    /// ihn nie, aber jede Umsetzung muss ihn gleich lesen.
    #[test]
    fn code_drei_ist_plus_zwei() {
        // Gewicht 0: Code 3; Gewicht 1: Code 1 (null); der Rest Code 1.
        let mut muster = vec![0b0101_0101u8; BYTES_JE_GRUPPE];
        muster[0] = 0b0101_0111;
        muster[1] = 0b0101_0101;
        let t = Ternaermatrix::neu(&muster, &[10], 1, GRUPPE).unwrap();
        let mut x = vec![0i16; GRUPPE];
        x[0] = 7;
        x[1] = 1000;
        assert_eq!(t.zeile_entpacken(0)[..2], [20, 0]);
        assert_eq!(t.zeile_mal(0, &x), 140);
        assert_eq!(zeile_skalar(&muster, &[10], &x), 140);
        let ganz = vec![0xFFu8; BYTES_JE_GRUPPE];
        let y = vec![i16::MIN; GRUPPE];
        let soll = 32767i64 * 2 * 128 * i64::from(i16::MIN);
        let t = Ternaermatrix::neu(&ganz, &[32767], 1, GRUPPE).unwrap();
        assert_eq!(t.zeile_mal(0, &y), soll);
        assert_eq!(zeile_skalar(&ganz, &[32767], &y), soll);
    }

    /// **Das Format, an einer Stelle festgehalten:** Gewicht `16s + j`
    /// eines Blocks steht in Byte `j`, Bits `2s`.
    #[test]
    fn das_format_ist_verschraenkt_nach_position() {
        let mut w = vec![0i8; GRUPPE];
        w[17] = 5; // Block 0, s = 1, j = 1: Byte 1, Bits 2..3, Code 2
        w[64 + 48 + 15] = -5; // Block 1, s = 3, j = 15: Byte 31, Bits 6..7, Code 0
        let p = packen(&w, GRUPPE).unwrap();
        assert_eq!(p.muster[1], 0b0101_1001);
        assert_eq!(p.muster[31], 0b0001_0101);
        assert!(p.muster.iter().enumerate().all(|(i, &b)| i == 1 || i == 31 || b == 0b0101_0101));
        assert_eq!(p.betraege, vec![5]);
    }

    #[test]
    fn falsche_laengen_fallen_auf() {
        assert!(Ternaermatrix::neu(&[0; 32], &[1], 1, 100).is_err());
        assert!(Ternaermatrix::neu(&[0; 31], &[1], 1, 128).is_err());
        assert!(Ternaermatrix::neu(&[0; 32], &[1, 2], 1, 128).is_err());
        let t = Ternaermatrix::neu(&[0; 32], &[1], 1, 128).unwrap();
        let kurz = vec![0i16; 127];
        assert!(std::panic::catch_unwind(|| t.zeile_mal(0, &kurz)).is_err());
    }
}
