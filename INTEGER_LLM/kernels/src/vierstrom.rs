//! **Bausteine der Bauart mit vier Stroemen** (Qwen3.8-Flash-Next, seit
//! dem 2026-10-19).
//!
//! Vier Dinge, die es vorher im Rechenpfad nicht gab, je als reine
//! ganzzahlige Funktion ohne Modellbezug:
//!
//! 1. [`Ngrammmasse`]: die Kennungen der N-Gramm-Einbettung. Ein Token und
//!    seine Vorgaenger werden mit festen Multiplikatoren gemischt
//!    (`xor`) und je Kopf modulo einer Primzahl auf eine Zeile der Tabelle
//!    abgebildet.
//! 2. [`vorzeichenwurzel`]: `sign(g) * sqrt(|g| / sqrt(H))`, das Tor der
//!    N-Gramm-Einbettung.
//! 3. [`Abstandsfaltung`]: eine tiefenweise Faltung, deren Kern nicht
//!    benachbarte, sondern jede dritte Position liest.
//! 4. [`blockauswahl`]: welche Viererbloecke der Folge eine Abfrage sieht.
//!
//! ⚑ **Gelesen, verstanden, neu geschrieben.** Die Vorlage ist der
//! Modellcode in `transformers` (`qwen4_exp`); keine Zeile davon steht hier.
//! Die Kennungen sind gegen die drei Puffer des echten Checkpoints und
//! gegen die Vorlage geprueft (Proben unten, Werte vom 2026-10-19).
//!
//! ⚑ **Keine dieser Konstanten steht in θ_v.** Sie gehoeren zu den
//! Merkmalen eines Artefakts dieser Bauart (`ngramm_einbettung`,
//! `blockauswahl`); ein θ_v-Sprung haette jedes vorhandene Artefakt
//! entwertet, auch die, die davon nichts benutzen.

use crate::fixed_point::{clamp_i16_from_i64, rescale_i64, rshift_round_i64};

// ============================================================================
// 1. N-Gramm-Kennungen
// ============================================================================

const MASKE64: u128 = (1u128 << 64) - 1;
const SPLITMIX_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
const SPLITMIX_M1: u64 = 0xBF58_476D_1CE4_E5B9;
const SPLITMIX_M2: u64 = 0x94D0_49BB_1331_11EB;
/// Der Abstand der Saat zwischen zwei Einbettungsebenen.
const EBENENABSTAND: u64 = 10_007;

/// splitmix64, ein Schritt. Alle Rechnungen modulo `2^64`.
fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(SPLITMIX_GAMMA);
    z = (z ^ (z >> 30)).wrapping_mul(SPLITMIX_M1);
    z = (z ^ (z >> 27)).wrapping_mul(SPLITMIX_M2);
    z ^ (z >> 31)
}

/// Primzahltest durch Probedivision. ⚑ Nur beim Laden, fuer Zahlen um
/// zwanzig Millionen (Teiler bis rund 4 500); kein Rechenpfad.
fn ist_prim(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n % 2 == 0 {
        return n == 2;
    }
    let mut d = 3u64;
    while d * d <= n {
        if n % d == 0 {
            return false;
        }
        d += 2;
    }
    true
}

/// **Die Masse der N-Gramm-Einbettung einer Ebene.**
///
/// Je N-Gramm der Laenge `2..=ngramm` gibt es `koepfe_je_ngramm` Koepfe;
/// Kopf `h` (ueber alle Laengen durchgezaehlt) hat eine eigene Primzahl als
/// Tabellengroesse und einen eigenen Versatz in der gemeinsamen Tabelle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ngrammmasse {
    /// Ein Multiplikator je Position im N-Gramm, ungerade und so klein, dass
    /// `token * multiplikator` unter `2^63` bleibt.
    pub multiplikatoren: Vec<u64>,
    /// Die Tabellengroesse je Kopf: die `(h+1)`-te Primzahl ab `basis`.
    pub groessen: Vec<u64>,
    /// Der Versatz je Kopf in der gemeinsamen Tabelle.
    pub versaetze: Vec<u64>,
    /// Zeilen der Tabelle, auf `teiler` aufgerundet.
    pub zeilen: u64,
    pub ngramm: usize,
    pub koepfe_je_ngramm: usize,
}

impl Ngrammmasse {
    /// **Rechnet die Masse aus der Konfiguration**, wie die Vorlage beim
    /// Anlegen der Ebene.
    ///
    /// ⚠️ Die Zaehlung der Primzahlen beginnt bei `basis` selbst: Gesucht
    /// ist die `(h+1)`-te Primzahl, die groesser ist als `basis - 1`.
    pub fn bemessen(
        vokabular: u64,
        ngramm: usize,
        koepfe_je_ngramm: usize,
        basis: u64,
        saat: u64,
        ebene_index: u64,
        teiler: u64,
    ) -> Self {
        assert!(ngramm >= 2, "ein N-Gramm hat mindestens zwei Token");
        assert!(vokabular > 0 && koepfe_je_ngramm > 0 && teiler > 0);
        let koepfe = (ngramm - 1) * koepfe_je_ngramm;

        // Multiplikatoren: `2 * (splitmix64(saat_i) % halb) + 1`, wobei
        // `halb` dafuer sorgt, dass `token * m` in einem i64 bleibt.
        let groesster = (i64::MAX as u64) / vokabular;
        let halb = (groesster / 2).max(1);
        let grundsaat = saat.wrapping_add(EBENENABSTAND.wrapping_mul(ebene_index));
        let multiplikatoren = (0..ngramm as u64)
            .map(|i| {
                let wert = (u128::from(grundsaat) + u128::from(SPLITMIX_GAMMA) * u128::from(i + 1)) & MASKE64;
                2 * (splitmix64(wert as u64) % halb) + 1
            })
            .collect();

        let mut groessen = Vec::with_capacity(koepfe);
        let mut versaetze = Vec::with_capacity(koepfe);
        let mut summe = 0u64;
        for h in 0..koepfe as u64 {
            let global = ebene_index * koepfe as u64 + h;
            // Die (global+1)-te Primzahl oberhalb von `basis - 1`.
            let mut p = basis - 1;
            for _ in 0..=global {
                p += 1;
                while !ist_prim(p) {
                    p += 1;
                }
            }
            groessen.push(p);
            versaetze.push(summe);
            summe += p;
        }
        let zeilen = summe.div_ceil(teiler) * teiler;
        Self { multiplikatoren, groessen, versaetze, zeilen, ngramm, koepfe_je_ngramm }
    }

    /// Wie viele Koepfe, also Tabellenzeilen je Token.
    pub fn koepfe(&self) -> usize {
        self.groessen.len()
    }

    /// **Die Tabellenzeilen eines Tokens.**
    ///
    /// `kontext[0]` ist der Vorgaenger, `kontext[1]` der davor, und so
    /// weiter, schon nach der EOS-Regel aus [`Ngrammmasse::weiter`].
    /// Zurueck kommen [`Ngrammmasse::koepfe`] Zeilen, erst die Koepfe der
    /// Bigramme, dann die der Trigramme.
    pub fn kennungen(&self, token: u64, kontext: &[u64]) -> Vec<u64> {
        assert_eq!(kontext.len(), self.ngramm - 1, "Kontext passt nicht zur N-Gramm-Laenge");
        let mut aus = Vec::with_capacity(self.koepfe());
        for n in 2..=self.ngramm {
            // ⚑ `token * m` passt nach `bemessen` in 63 Bit; `wrapping` nur,
            //   damit ein kaputtes Vokabular nicht abbricht, sondern eine
            //   Zeile trifft, die der Lader ohnehin begrenzt.
            let mut gemischt = token.wrapping_mul(self.multiplikatoren[0]);
            for (pos, &vor) in kontext.iter().take(n - 1).enumerate() {
                gemischt ^= vor.wrapping_mul(self.multiplikatoren[pos + 1]);
            }
            let erster = (n - 2) * self.koepfe_je_ngramm;
            for h in erster..erster + self.koepfe_je_ngramm {
                aus.push(gemischt % self.groessen[h] + self.versaetze[h]);
            }
        }
        aus
    }

    /// **Der Kontext fuer das naechste Token.**
    ///
    /// ⚑ **Ein EOS beginnt einen neuen Abschnitt:** Hinter einem EOS sieht
    /// kein Token mehr, was davor stand, und jeder fehlende Vorgaenger ist
    /// selbst ein EOS. Am Anfang einer Folge ist der Kontext ganz EOS.
    pub fn weiter(kontext: &[u64], token: u64, eos: u64) -> Vec<u64> {
        if token == eos {
            return vec![eos; kontext.len()];
        }
        let mut neu = Vec::with_capacity(kontext.len());
        neu.push(token);
        neu.extend_from_slice(&kontext[..kontext.len().saturating_sub(1)]);
        neu
    }
}

// ============================================================================
// 2. Wurzeln und die Vorzeichenwurzel
// ============================================================================

/// Abgerundete Ganzzahlwurzel in u128.
fn isqrt_u128(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    // Newton ab einer Zweierpotenz ueber der Wurzel; faellt monoton.
    let bits = 128 - n.leading_zeros();
    let mut x: u128 = 1u128 << bits.div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            break;
        }
        x = y;
    }
    while x * x > n {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x
}

/// **`round(sqrt(g * 2^-g_frac) * 2^aus_frac)`**, ganzzahlig.
///
/// ⚑ Gerechnet wird `sqrt(g * 2^(2*aus_frac - g_frac))`. Ist der Exponent
/// negativ oder ungerade aufzufuellen, rechnet die Funktion zwei Bit feiner
/// und rundet danach einmal, damit kein Rechtsshift vor der Wurzel Bits
/// verschenkt.
pub fn wurzel_q(g: u128, g_frac: u32, aus_frac: u32) -> u64 {
    // Feiner um `extra` Bit, bis der Exponent nicht negativ ist.
    let mut extra = 0u32;
    while 2 * (aus_frac + extra) < g_frac {
        extra += 1;
    }
    let e = 2 * (aus_frac + extra) - g_frac;
    assert!(e < 128 && (g == 0 || g.leading_zeros() > e), "wurzel_q: {g} << {e} sprengt u128");
    let r = isqrt_u128(g << e);
    if extra == 0 {
        // Runden zur naechsten Wurzel: liegt n naeher an (r+1)^2?
        let n = g << e;
        let unten = n - r * r;
        let oben = (r + 1) * (r + 1) - n;
        let r = if oben < unten { r + 1 } else { r };
        return u64::try_from(r).expect("wurzel_q: Ergebnis sprengt u64");
    }
    let r = i64::try_from(r).expect("wurzel_q: Ergebnis sprengt i64");
    rshift_round_i64(r, extra as u8) as u64
}

/// **`round(2^16 / h^(1/4))`**, ganzzahlig: die vierte Wurzel als
/// Festkommakonstante, einmal beim Laden.
///
/// ⚑ Gesucht ist das `c`, dessen vierte Potenz `2^64 / h` am naechsten
/// kommt; die Suche laeuft ueber u128 und braucht keine Gleitkommazahl.
pub fn kehrwert_vierte_wurzel_q16(h: u64) -> u64 {
    assert!(h > 0, "die vierte Wurzel von null hat keinen Kehrwert");
    let ziel = (1u128 << 64) / u128::from(h);
    // c^4 ~ ziel: erst die Quadratwurzel, dann noch einmal.
    let c = isqrt_u128(isqrt_u128(ziel));
    // Nachbarn vergleichen, gegen den unverkuerzten Bruch 2^64 / h.
    let abstand = |c: u128| -> u128 {
        let v = c * c * c * c * u128::from(h);
        let z = 1u128 << 64;
        v.abs_diff(z)
    };
    let mut best = c;
    for k in [c.saturating_sub(1), c + 1, c + 2] {
        if abstand(k) < abstand(best) {
            best = k;
        }
    }
    best as u64
}

/// **Das Tor der N-Gramm-Einbettung: `sign(g) * sqrt(|g| / sqrt(H))`.**
///
/// `g` ist das rohe Skalarprodukt auf `g_frac`; `c_q16` ist
/// [`kehrwert_vierte_wurzel_q16`] der Breite `H`. Zurueck kommt der Wert auf
/// `aus_frac`, als i32 (fuer die Sigmoid-Tabelle).
///
/// ⚑ **`sqrt(|g| / sqrt(H)) = sqrt(|g|) / H^(1/4)`**: eine Wurzel und eine
/// Multiplikation, statt einer Division durch eine Wurzel.
pub fn vorzeichenwurzel(g: i128, g_frac: u32, c_q16: u64, aus_frac: u32) -> i32 {
    // ⚑ Die Wurzel 16 Bit feiner, mal `c` auf Q16, dann **ein** Shift um 32
    //   mit Rundung zur geraden Zahl.
    let w = wurzel_q(g.unsigned_abs(), g_frac, aus_frac + 16);
    let produkt = i128::from(w) * i128::from(c_q16);
    let r = crate::fixed_point::rshift_round_i128(produkt, 32);
    let r = r.clamp(0, i128::from(i32::MAX)) as i32;
    if g < 0 { -r } else { r }
}

// ============================================================================
// 3. Die Faltung mit Abstand
// ============================================================================

/// **Eine tiefenweise Faltung, deren Kern jede `abstand`-te Position liest.**
///
/// Kernstelle `k` (von `0` bis `kern - 1`) gewichtet die Position
/// `t - (kern - 1 - k) * abstand`; die letzte Stelle also das Token selbst.
/// Vor dem Anfang der Folge steht null.
///
/// ⚑ **Ein Ringpuffer der letzten `(kern - 1) * abstand` Eingaben**, je
/// Kanal. Er ist Zustand der Folge wie der der rekurrenten Ebenen: Er laesst
/// sich aufheben, nicht zurueckrechnen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abstandsfaltung {
    pub kanaele: usize,
    pub kern: usize,
    pub abstand: usize,
    /// `verlauf[p * kanaele + c]`, `p = 0` die aelteste Position.
    verlauf: Vec<i16>,
}

impl Abstandsfaltung {
    pub fn leer(kanaele: usize, kern: usize, abstand: usize) -> Self {
        assert!(kern >= 1 && abstand >= 1);
        Self { kanaele, kern, abstand, verlauf: vec![0; (kern - 1) * abstand * kanaele] }
    }

    /// Wie viele Positionen der Puffer haelt.
    pub fn tiefe(&self) -> usize {
        (self.kern - 1) * self.abstand
    }

    /// Alles auf null, wie am Anfang einer Folge.
    pub fn leeren(&mut self) {
        self.verlauf.iter_mut().for_each(|x| *x = 0);
    }

    /// **Ein Schritt**: `x` auf `x_frac` hinein, die Faltung je Kanal auf
    /// `aus_frac` heraus, als i64 (ohne Klemmung; der Aufrufer wendet die
    /// Aktivierung an).
    ///
    /// `gewichte[c * kern + k]` mit der Zeilenskala `shifts[c]`.
    pub fn schritt(&mut self, x: &[i16], x_frac: u8, gewichte: &[i8], shifts: &[u8], aus_frac: u8) -> Vec<i64> {
        let (c_n, kern, tiefe) = (self.kanaele, self.kern, self.tiefe());
        assert_eq!(x.len(), c_n, "Abstandsfaltung: Eingang hat {} statt {} Kanaele", x.len(), c_n);
        assert_eq!(gewichte.len(), c_n * kern, "Abstandsfaltung: Gewichte passen nicht");
        assert_eq!(shifts.len(), c_n, "Abstandsfaltung: eine Skala je Kanal");
        let mut aus = Vec::with_capacity(c_n);
        for c in 0..c_n {
            let mut acc = 0i64;
            for k in 0..kern {
                let zurueck = (kern - 1 - k) * self.abstand;
                let wert = if zurueck == 0 {
                    x[c]
                } else {
                    // Position `t - zurueck` liegt im Puffer an `tiefe - zurueck`.
                    self.verlauf[(tiefe - zurueck) * c_n + c]
                };
                acc += i64::from(gewichte[c * kern + k]) * i64::from(wert);
            }
            aus.push(rescale_i64(acc, x_frac + shifts[c], aus_frac));
        }
        // Puffer weiterschieben: die aelteste Position faellt heraus.
        if tiefe > 0 {
            self.verlauf.copy_within(c_n.., 0);
            let start = (tiefe - 1) * c_n;
            self.verlauf[start..start + c_n].copy_from_slice(x);
        }
        aus
    }
}

// ============================================================================
// 4. Die Blockauswahl
// ============================================================================

/// **Welche Bloecke eine Abfrage sieht**: die `k` mit der hoechsten
/// Wertung, aufsteigend nach Blocknummer.
///
/// ⚑ **Bei Gleichstand gewinnt der fruehere Block.** Die Vorlage nimmt
/// `topk` ueber Gleitkommawerte und laesst die Reihenfolge bei Gleichstand
/// offen; im Konsens muss sie festliegen.
///
/// ⚑ **Aufsteigend zurueck**, damit die Aufmerksamkeit die gewaehlten Token
/// in derselben Reihenfolge liest wie die volle; ihre Summe ist dann
/// dieselbe Zahl, wenn alle Bloecke gewaehlt sind.
pub fn blockauswahl(wertungen: &[i64], k: usize) -> Vec<usize> {
    if k >= wertungen.len() {
        return (0..wertungen.len()).collect();
    }
    let mut ordnung: Vec<usize> = (0..wertungen.len()).collect();
    // Hoehere Wertung zuerst, bei Gleichstand der kleinere Index.
    ordnung.sort_unstable_by(|&a, &b| wertungen[b].cmp(&wertungen[a]).then(a.cmp(&b)));
    let mut gewaehlt: Vec<usize> = ordnung.into_iter().take(k).collect();
    gewaehlt.sort_unstable();
    gewaehlt
}

/// **Die Wertung eines Blocks**: `Summe der Koepfe von relu(q_h . k)`.
///
/// ⚑ **Ohne den Faktor `1/sqrt(Kopfbreite)` der Vorlage.** Er ist fuer
/// alle Bloecke derselbe positive Faktor und aendert die Reihenfolge nicht;
/// gebraucht wird nur sie.
pub fn blockwertung(abfragen: &[i16], kopf_dim: usize, schluessel: &[i16]) -> i64 {
    assert_eq!(schluessel.len(), kopf_dim, "Blockschluessel hat die falsche Breite");
    assert_eq!(abfragen.len() % kopf_dim, 0, "Abfragen sind keine ganzen Koepfe");
    abfragen
        .chunks_exact(kopf_dim)
        .map(|q| crate::attention::dot_int(q, schluessel).max(0))
        .sum()
}

/// **Der Blockschluessel vor Norm und Drehung**: das Mittel der rohen
/// Schluessel eines Blocks, je Stelle `sum / n` mit Rundung.
pub fn blockmittel(schluessel: &[&[i16]]) -> Vec<i16> {
    let n = schluessel.len();
    assert!(n > 0, "ein Block ohne Token hat kein Mittel");
    let breite = schluessel[0].len();
    (0..breite)
        .map(|j| {
            let summe: i64 = schluessel.iter().map(|k| i64::from(k[j])).sum();
            if n.is_power_of_two() {
                clamp_i16_from_i64(rshift_round_i64(summe, n.trailing_zeros() as u8))
            } else {
                // Rundung zur naechsten, bei Gleichstand zur geraden Zahl.
                let q = summe.div_euclid(n as i64);
                let r = summe.rem_euclid(n as i64) * 2;
                let q = if r > n as i64 || (r == n as i64 && q % 2 != 0) { q + 1 } else { q };
                clamp_i16_from_i64(q)
            }
        })
        .collect()
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Die Konfiguration des echten Modells (`config.json`, 2026-10-19).
    fn echte_masse() -> Ngrammmasse {
        Ngrammmasse::bemessen(248_320, 3, 8, 20_000_000, 1234, 0, 128)
    }

    /// ⚑ **Gegen die drei I64-Puffer des echten Checkpoints**, geholt am
    /// 2026-10-19 per Range-Anfrage aus
    /// `model.language_model.layers.1.ple.ple_embedding.*`.
    #[test]
    fn die_masse_stimmen_mit_dem_checkpoint() {
        let m = echte_masse();
        assert_eq!(m.multiplikatoren, vec![23_703_573_157_769, 20_109_073_645_365, 8_052_911_324_071]);
        assert_eq!(
            m.groessen,
            vec![
                20000003, 20000023, 20000033, 20000047, 20000059, 20000063, 20000069, 20000077, 20000081,
                20000093, 20000107, 20000147, 20000153, 20000159, 20000161, 20000171
            ]
        );
        assert_eq!(m.versaetze[1], 20_000_003);
        assert_eq!(m.versaetze[15], 300_001_275);
        // Die Tabelle des Checkpoints hat 128 Teile zu 2 500 012 Zeilen.
        assert_eq!(m.zeilen, 320_001_536);
        assert_eq!(m.zeilen, 128 * 2_500_012);
    }

    /// Kennungen gegen die Vorlage, drei Folgen; EOS ist 248044.
    #[test]
    fn die_kennungen_stimmen_mit_der_vorlage() {
        let m = echte_masse();
        let eos = 248_044u64;
        let pruefe = |folge: &[u64], soll: &[[u64; 16]]| {
            let mut kontext = vec![eos; 2];
            for (t, &tok) in folge.iter().enumerate() {
                assert_eq!(m.kennungen(tok, &kontext), soll[t].to_vec(), "Folge {folge:?}, Position {t}");
                kontext = Ngrammmasse::weiter(&kontext, tok, eos);
            }
        };
        pruefe(
            &[100, 2000, 248319, 7],
            &[
                [5727835, 21884476, 43702108, 66434789, 84094509, 104112171, 134886528, 157314943, 162363667, 189466439, 200618315, 232121522, 258547276, 266199001, 289022207, 305180972],
                [18305251, 20651032, 51860967, 75596311, 93122238, 105638796, 124421037, 149477849, 169789685, 181530548, 207004672, 221757328, 241317278, 261228606, 294610680, 302105392],
                [15586355, 27602253, 57994815, 73455087, 82695252, 106710742, 123610832, 154447887, 171505319, 199661297, 217830223, 238455281, 243583131, 269763510, 298724326, 305281239],
                [1196148, 36287911, 47578981, 67581089, 88620749, 116433004, 128900332, 146921636, 168773818, 189493805, 201555055, 237329026, 247148663, 277869377, 288309821, 302013249],
            ],
        );
        // ⚑ EOS mitten in der Folge: Das Token danach sieht nur EOS.
        pruefe(
            &[5, eos, 9, 11],
            &[
                [15389869, 39778609, 55713969, 62213332, 88817728, 118483999, 133731511, 155458159, 179763390, 197956758, 205378969, 220499474, 242466248, 265658744, 293662119, 315720898],
                [18392917, 34857659, 47499630, 74137148, 90126688, 103063933, 133351754, 142048315, 173113148, 192355065, 202997842, 236534513, 246250533, 276928146, 287367671, 301167364],
                [18043673, 37626835, 51159316, 78294604, 94015356, 106720349, 136526052, 144330141, 176817901, 186368539, 203707490, 230017629, 247662678, 266533413, 293096193, 307951937],
                [10041117, 28960672, 48420531, 71664411, 83016360, 106800418, 122476460, 150044571, 163654473, 184259024, 206781966, 224776026, 248853488, 273290488, 294849492, 303242927],
            ],
        );
        pruefe(
            &[eos, eos, 3],
            &[
                [9663979, 26558231, 56120240, 74755659, 80459717, 109265651, 132697467, 151022725, 170054832, 192967038, 200687722, 225763581, 259275737, 272983544, 297596484, 300986548],
                [9663979, 26558231, 56120240, 74755659, 80459717, 109265651, 132697467, 151022725, 170054832, 192967038, 200687722, 225763581, 259275737, 272983544, 297596484, 300986548],
                [5318554, 26730617, 51177670, 61593418, 94411889, 119482836, 127837362, 140373362, 175277381, 199452204, 217186100, 238911097, 253868456, 270051525, 282384981, 306094789],
            ],
        );
    }

    /// Jede Kennung liegt in ihrem Kopf und damit in der Tabelle.
    #[test]
    fn jede_kennung_liegt_in_ihrem_kopf() {
        let m = echte_masse();
        let mut x = 0x1234_5678u64;
        for _ in 0..2000 {
            x = splitmix64(x);
            let tok = x % 248_320;
            let kennungen = m.kennungen(tok, &[x % 1000, (x >> 20) % 248_320]);
            for (h, &k) in kennungen.iter().enumerate() {
                assert!(k >= m.versaetze[h] && k < m.versaetze[h] + m.groessen[h]);
                assert!(k < m.zeilen);
            }
        }
    }

    #[test]
    fn wurzeln_sind_genau() {
        for n in [0u128, 1, 2, 3, 4, 15, 16, 17, 1 << 40, (1 << 64) - 1, u128::MAX >> 2] {
            let r = isqrt_u128(n);
            assert!(r * r <= n && (r + 1) * (r + 1) > n, "{n}");
        }
        // sqrt(2) auf Q16: 92682
        assert_eq!(wurzel_q(2, 0, 16), 92_682);
        // sqrt(0,25) = 0,5 auf Q8, Eingang auf Q2 (1 = 0,25)
        assert_eq!(wurzel_q(1, 2, 8), 128);
        // Ungerader Eingangsexponent: sqrt(0,5) auf Q10 = 724,08
        assert_eq!(wurzel_q(1, 1, 10), 724);
        // Grobe Ausgabe unter feinem Eingang: sqrt(9 * 2^-20) = 3/1024 auf Q12 = 12
        assert_eq!(wurzel_q(9, 20, 12), 12);
    }

    /// `round(2^16 / 2560^(1/4))` = round(65536 / 7.1131) = 9213
    #[test]
    fn die_vierte_wurzel_der_breite() {
        assert_eq!(kehrwert_vierte_wurzel_q16(1), 65_536);
        assert_eq!(kehrwert_vierte_wurzel_q16(16), 32_768);
        assert_eq!(kehrwert_vierte_wurzel_q16(2560), 9_213);
    }

    /// `sign(g) sqrt(|g|/sqrt(2560))`: g = 100 ergibt 10 / 7,1131 = 1,4058.
    #[test]
    fn die_vorzeichenwurzel() {
        let c = kehrwert_vierte_wurzel_q16(2560);
        // g = 100 auf Q0, Ausgabe auf Q9: 1,4058 * 512 = 719,8
        assert_eq!(vorzeichenwurzel(100, 0, c, 9), 720);
        assert_eq!(vorzeichenwurzel(-100, 0, c, 9), -720);
        assert_eq!(vorzeichenwurzel(0, 0, c, 9), 0);
        // Derselbe Wert auf Q20 eingegeben.
        assert_eq!(vorzeichenwurzel(100 << 20, 20, c, 9), 720);
    }

    /// Kern 4, Abstand 3: die Faltung liest t, t-3, t-6 und t-9.
    #[test]
    fn die_faltung_liest_jede_dritte_position() {
        let mut f = Abstandsfaltung::leer(1, 4, 3);
        assert_eq!(f.tiefe(), 9);
        // Gewichte [1, 10, 100, 100] auf Skala 0: Stelle 0 gewichtet t-9,
        // Stelle 3 das Token selbst.
        let w = [1i8, 10, 100, 100];
        let mut aus = Vec::new();
        for t in 0..12i16 {
            aus.push(f.schritt(&[t + 1], 0, &w, &[0], 0)[0]);
        }
        // t = 9: x_9 = 10, x_6 = 7, x_3 = 4, x_0 = 1
        assert_eq!(aus[9], 100 * 10 + 100 * 7 + 10 * 4 + 1);
        // t = 2: nur x_2 = 3; davor ist null
        assert_eq!(aus[2], 100 * 3);
        // t = 5: x_5 = 6, x_2 = 3
        assert_eq!(aus[5], 100 * 6 + 100 * 3);
        f.leeren();
        assert_eq!(f.schritt(&[1], 0, &w, &[0], 0)[0], 100);
    }

    #[test]
    fn die_blockauswahl_ist_bestimmt() {
        assert_eq!(blockauswahl(&[5, 9, 9, 1, 7], 2), vec![1, 2]);
        // Gleichstand an der Grenze: der fruehere gewinnt.
        assert_eq!(blockauswahl(&[3, 9, 3, 3], 2), vec![0, 1]);
        // Alle gewaehlt, wenn k reicht.
        assert_eq!(blockauswahl(&[1, 2, 3], 5), vec![0, 1, 2]);
        assert_eq!(blockwertung(&[1, 0, -1, 0], 2, &[3, 4]), 3);
        assert_eq!(blockmittel(&[&[1, 2], &[2, 2], &[3, 2], &[3, -2]]), vec![2, 1]);
    }
}
