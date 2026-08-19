//! Erasure-Codierung über GF(2⁸) — Whitepaper Kap. 3.5.3.
//!
//! Reed-Solomon-artige Codierung in **systematischer Cauchy-Form**:
//! `k` Datenfragmente bleiben unverändert, `m` Paritätsfragmente kommen
//! hinzu. Aus **beliebigen `k` der `k+m`** Fragmente lässt sich das
//! Original vollständig rekonstruieren.
//!
//! Startparameter des Projekts: `k = 8`, `m = 4` (Design-Entscheidung 5
//! des CONSENSUS-Fahrplans, später Governance-Parameter). Ein Pod darf
//! damit ein Drittel seiner Fragmente verlieren, ohne dass die
//! Bisektions-Anfrage eines Streitfalls ins Leere läuft.
//!
//! ## Warum Cauchy und nicht Vandermonde
//!
//! Bei einer Vandermonde-Matrix ist die Invertierbarkeit **jeder**
//! k×k-Teilmatrix nicht automatisch gegeben — die klassische
//! „Vandermonde-RS"-Konstruktion hat dieses Loch, und es äußert sich
//! nicht als Fehler, sondern als Rekonstruktion, die für bestimmte
//! Ausfallmuster stillschweigend falsche Daten liefert. Bei einer
//! Cauchy-Matrix `C[i][j] = 1/(x_i ⊕ y_j)` mit disjunkten Mengen
//! `{x_i}` und `{y_j}` ist **jede** quadratische Teilmatrix invertierbar.
//! Das ist der Grund für die Wahl; [`ErasureCoder::decode`] verlässt
//! sich darauf.
//!
//! Die Eigenschaft ist hier nicht behauptet, sondern geprüft: der Test
//! `jede_k_aus_n_teilmenge_rekonstruiert` fährt **alle** 495 Teilmengen
//! von 8 aus 12 durch.
//!
//! ## Ganzzahligkeit
//!
//! GF(2⁸)-Arithmetik ist reine Bitarithmetik über Tabellen — kein
//! Gleitkomma, keine Ordnungsabhängigkeit, bitgleich auf jeder Hardware.
//! Das ist dieselbe Eigenschaft, auf der die Inferenz beruht
//! (Whitepaper Kap. 6.2), hier für die Datenverfügbarkeit.
//!
//! **Konsens-Feld:** Generator-Konstruktion und Fragmentreihenfolge sind
//! Teil des Konsensvertrags. Änderungen nur über Governance (Kap. 10.3).

/// Primitives Polynom von GF(2⁸): x⁸ + x⁴ + x³ + x² + 1 (0x11D).
///
/// Der übliche Reed-Solomon-Wert. Teil des Konsensvertrags — ein
/// anderes Polynom ergibt einen anderen Körper und damit andere
/// Paritätsfragmente.
pub const GF_POLY: u16 = 0x11D;

/// Standard-Datenfragmente je Segment (Design-Entscheidung 5).
pub const DEFAULT_K: usize = 8;

/// Standard-Paritätsfragmente je Segment (Design-Entscheidung 5).
pub const DEFAULT_M: usize = 4;

/// Fehler der Erasure-Codierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErasureError {
    /// `k` oder `m` ist 0, oder `k + m` übersteigt 255 (die Anzahl der
    /// von null verschiedenen Körperelemente).
    InvalidParameters {
        /// Datenfragmente.
        k: usize,
        /// Paritätsfragmente.
        m: usize,
    },
    /// Die Eingabe ist leer.
    EmptyInput,
    /// Es liegen weniger als `k` Fragmente vor — Rekonstruktion
    /// unmöglich. Das ist der definierte Ausfall, kein Fehlerfall im
    /// Sinne eines Bugs.
    NotEnoughFragments {
        /// Vorhandene Fragmente.
        have: usize,
        /// Benötigte Fragmente.
        need: usize,
    },
    /// Ein Fragmentindex liegt außerhalb von `0..k+m`.
    IndexOutOfRange {
        /// Der ungültige Index.
        index: usize,
    },
    /// Zwei Fragmente tragen denselben Index.
    DuplicateIndex {
        /// Der doppelte Index.
        index: usize,
    },
    /// Die Fragmente haben unterschiedliche Längen.
    InconsistentLength,
}

impl std::fmt::Display for ErasureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidParameters { k, m } => {
                write!(f, "Ungültige Parameter k={}, m={}", k, m)
            }
            Self::EmptyInput => write!(f, "Leere Eingabe"),
            Self::NotEnoughFragments { have, need } => write!(
                f,
                "Zu wenige Fragmente: {} vorhanden, {} nötig",
                have, need
            ),
            Self::IndexOutOfRange { index } => {
                write!(f, "Fragmentindex {} außerhalb des gültigen Bereichs", index)
            }
            Self::DuplicateIndex { index } => {
                write!(f, "Fragmentindex {} kommt mehrfach vor", index)
            }
            Self::InconsistentLength => write!(f, "Fragmente unterschiedlicher Länge"),
        }
    }
}

impl std::error::Error for ErasureError {}

/// Multiplikation in GF(2⁸) (russische Bauernmultiplikation mit
/// Reduktion modulo [`GF_POLY`]).
///
/// Bewusst tabellenfrei und in Konstantzeit bezüglich der Bitlänge:
/// eine Log/Exp-Tabelle wäre schneller, hätte aber einen Sonderfall bei
/// der Null, der sich leicht falsch schreibt.
fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p: u8 = 0;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hoch = a & 0x80;
        a <<= 1;
        if hoch != 0 {
            a ^= (GF_POLY & 0xFF) as u8;
        }
        b >>= 1;
    }
    p
}

/// Multiplikatives Inverses in GF(2⁸).
///
/// Über den kleinen Satz von Fermat: `a^(255-1) = a^254 = a⁻¹`.
/// `gf_inv(0)` ist nicht definiert und gibt 0 zurück; die Aufrufer in
/// diesem Modul rufen es nie mit 0 auf (die Cauchy-Konstruktion
/// garantiert `x_i ⊕ y_j ≠ 0`).
fn gf_inv(a: u8) -> u8 {
    if a == 0 {
        return 0;
    }
    let mut ergebnis: u8 = 1;
    let mut basis = a;
    let mut exponent = 254u32;
    while exponent > 0 {
        if exponent & 1 == 1 {
            ergebnis = gf_mul(ergebnis, basis);
        }
        basis = gf_mul(basis, basis);
        exponent >>= 1;
    }
    ergebnis
}

/// Ein Fragment mit seinem Index innerhalb des Codeworts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// Position im Codewort: `0..k` sind Datenfragmente, `k..k+m`
    /// Paritätsfragmente.
    pub index: usize,
    /// Die Nutzdaten dieses Fragments.
    pub data: Vec<u8>,
}

/// Erasure-Codierer mit fester Parametrierung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErasureCoder {
    k: usize,
    m: usize,
}

impl Default for ErasureCoder {
    fn default() -> Self {
        Self {
            k: DEFAULT_K,
            m: DEFAULT_M,
        }
    }
}

impl ErasureCoder {
    /// Neuer Codierer mit `k` Daten- und `m` Paritätsfragmenten.
    ///
    /// **Fehler:** [`ErasureError::InvalidParameters`], wenn `k` oder
    /// `m` null ist oder `k + m > 255` — mehr Fragmente als GF(2⁸) von
    /// null verschiedene Elemente hat, dann bricht die
    /// Cauchy-Konstruktion.
    pub fn new(k: usize, m: usize) -> Result<Self, ErasureError> {
        if k == 0 || m == 0 || k + m > 255 {
            return Err(ErasureError::InvalidParameters { k, m });
        }
        Ok(Self { k, m })
    }

    /// Anzahl der Datenfragmente.
    pub fn k(&self) -> usize {
        self.k
    }

    /// Anzahl der Paritätsfragmente.
    pub fn m(&self) -> usize {
        self.m
    }

    /// Gesamtzahl der Fragmente eines Codeworts.
    pub fn n(&self) -> usize {
        self.k + self.m
    }

    /// Cauchy-Koeffizient der Paritätszeile `i` für Datenspalte `j`.
    ///
    /// `1/(x_i ⊕ y_j)` mit `x_i = i` und `y_j = m + j`. Die Mengen sind
    /// disjunkt (`i < m ≤ m + j`), deshalb ist `x_i ⊕ y_j` nie null und
    /// jede quadratische Teilmatrix invertierbar.
    fn cauchy(&self, i: usize, j: usize) -> u8 {
        let x = i as u8;
        let y = (self.m + j) as u8;
        gf_inv(x ^ y)
    }

    /// Zerlegt `data` in `k+m` Fragmente.
    ///
    /// Die Nutzdaten werden auf ein Vielfaches von `k` aufgefüllt; die
    /// ursprüngliche Länge muss der Aufrufer mitführen (im Protokoll
    /// steht sie im Segment-Kopf). [`Self::decode`] gibt deshalb die
    /// aufgefüllte Länge zurück.
    ///
    /// **Fehler:** [`ErasureError::EmptyInput`] bei leerer Eingabe.
    pub fn encode(&self, data: &[u8]) -> Result<Vec<Fragment>, ErasureError> {
        if data.is_empty() {
            return Err(ErasureError::EmptyInput);
        }
        let shard_len = data.len().div_ceil(self.k);
        let mut fragmente = Vec::with_capacity(self.n());

        // Datenfragmente: unveränderte Streifen, letzter mit Nullen
        // aufgefüllt (systematischer Code).
        for j in 0..self.k {
            let start = j * shard_len;
            let mut streifen = vec![0u8; shard_len];
            if start < data.len() {
                let ende = (start + shard_len).min(data.len());
                streifen[..ende - start].copy_from_slice(&data[start..ende]);
            }
            fragmente.push(Fragment {
                index: j,
                data: streifen,
            });
        }

        // Paritätsfragmente aus der Cauchy-Matrix.
        for i in 0..self.m {
            let mut paritaet = vec![0u8; shard_len];
            for (j, quelle) in fragmente.iter().take(self.k).enumerate() {
                let koeff = self.cauchy(i, j);
                for (p, q) in paritaet.iter_mut().zip(quelle.data.iter()) {
                    *p ^= gf_mul(koeff, *q);
                }
            }
            fragmente.push(Fragment {
                index: self.k + i,
                data: paritaet,
            });
        }
        Ok(fragmente)
    }

    /// Rekonstruiert die aufgefüllten Nutzdaten aus beliebigen `k`
    /// Fragmenten.
    ///
    /// Mehr als `k` Fragmente sind erlaubt; verwendet werden die ersten
    /// `k` in der übergebenen Reihenfolge.
    ///
    /// **Fehler:** [`ErasureError::NotEnoughFragments`] bei weniger als
    /// `k` — das ist der **definierte** Ausfall, nicht ein Bug.
    /// Außerdem [`ErasureError::IndexOutOfRange`],
    /// [`ErasureError::DuplicateIndex`] und
    /// [`ErasureError::InconsistentLength`]: Ein Aufrufer, der
    /// beschädigte Fragmentlisten übergibt, bekommt einen Fehler und
    /// keine stillschweigend falsche Rekonstruktion.
    pub fn decode(&self, fragmente: &[Fragment]) -> Result<Vec<u8>, ErasureError> {
        if fragmente.len() < self.k {
            return Err(ErasureError::NotEnoughFragments {
                have: fragmente.len(),
                need: self.k,
            });
        }
        let shard_len = fragmente[0].data.len();
        let mut gesehen = vec![false; self.n()];
        for f in fragmente {
            if f.index >= self.n() {
                return Err(ErasureError::IndexOutOfRange { index: f.index });
            }
            if gesehen[f.index] {
                return Err(ErasureError::DuplicateIndex { index: f.index });
            }
            gesehen[f.index] = true;
            if f.data.len() != shard_len {
                return Err(ErasureError::InconsistentLength);
            }
        }

        let genutzt = &fragmente[..self.k];

        // Zeilen der Generatormatrix zu den vorhandenen Fragmenten.
        let mut matrix = vec![vec![0u8; self.k]; self.k];
        for (r, f) in genutzt.iter().enumerate() {
            if f.index < self.k {
                matrix[r][f.index] = 1;
            } else {
                let i = f.index - self.k;
                for (j, zelle) in matrix[r].iter_mut().enumerate() {
                    *zelle = self.cauchy(i, j);
                }
            }
        }

        let inverse = invertiere(&mut matrix, self.k)?;

        let mut ausgabe = vec![0u8; self.k * shard_len];
        for j in 0..self.k {
            let ziel = &mut ausgabe[j * shard_len..(j + 1) * shard_len];
            for (r, f) in genutzt.iter().enumerate() {
                let koeff = inverse[j][r];
                if koeff == 0 {
                    continue;
                }
                for (z, q) in ziel.iter_mut().zip(f.data.iter()) {
                    *z ^= gf_mul(koeff, *q);
                }
            }
        }
        Ok(ausgabe)
    }
}

/// Invertiert eine k×k-Matrix über GF(2⁸) per Gauß-Jordan.
///
/// Gibt [`ErasureError::NotEnoughFragments`] zurück, wenn die Matrix
/// singulär ist. Bei der Cauchy-Konstruktion kann das nicht eintreten;
/// der Zweig steht als Sicherung, falls jemand die Generator-Konstruktion
/// ändert — dann bricht die Rekonstruktion sichtbar ab, statt falsche
/// Daten zu liefern.
fn invertiere(matrix: &mut [Vec<u8>], k: usize) -> Result<Vec<Vec<u8>>, ErasureError> {
    let mut inv = vec![vec![0u8; k]; k];
    for (i, zeile) in inv.iter_mut().enumerate() {
        zeile[i] = 1;
    }

    for spalte in 0..k {
        // Pivot suchen.
        let pivot = (spalte..k).find(|&zeile| matrix[zeile][spalte] != 0);
        let pivot = pivot.ok_or(ErasureError::NotEnoughFragments {
            have: k,
            need: k + 1,
        })?;
        matrix.swap(spalte, pivot);
        inv.swap(spalte, pivot);

        // Pivotzeile normieren.
        let faktor = gf_inv(matrix[spalte][spalte]);
        for j in 0..k {
            matrix[spalte][j] = gf_mul(matrix[spalte][j], faktor);
            inv[spalte][j] = gf_mul(inv[spalte][j], faktor);
        }

        // Übrige Zeilen eliminieren.
        for zeile in 0..k {
            if zeile == spalte {
                continue;
            }
            let faktor = matrix[zeile][spalte];
            if faktor == 0 {
                continue;
            }
            for j in 0..k {
                matrix[zeile][j] ^= gf_mul(faktor, matrix[spalte][j]);
                inv[zeile][j] ^= gf_mul(faktor, inv[spalte][j]);
            }
        }
    }
    Ok(inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Körperarithmetik ────────────────────────────────────────────

    #[test]
    fn gf_multiplikation_hat_neutrales_element() {
        for a in 0..=255u8 {
            assert_eq!(gf_mul(a, 1), a);
            assert_eq!(gf_mul(1, a), a);
            assert_eq!(gf_mul(a, 0), 0);
        }
    }

    #[test]
    fn gf_multiplikation_ist_kommutativ_und_assoziativ() {
        // Stichprobe ueber den ganzen Koerper waere 16 Mio Tripel;
        // ein deterministisches Raster genuegt.
        for a in (0..=255u8).step_by(7) {
            for b in (0..=255u8).step_by(11) {
                assert_eq!(gf_mul(a, b), gf_mul(b, a));
                for c in (0..=255u8).step_by(31) {
                    assert_eq!(gf_mul(gf_mul(a, b), c), gf_mul(a, gf_mul(b, c)));
                }
            }
        }
    }

    #[test]
    fn gf_inverses_ist_fuer_jedes_element_ausser_null_korrekt() {
        for a in 1..=255u8 {
            assert_eq!(gf_mul(a, gf_inv(a)), 1, "a={}", a);
        }
        assert_eq!(gf_inv(0), 0);
    }

    // ── Parametrierung ──────────────────────────────────────────────

    #[test]
    fn ungueltige_parameter_werden_abgelehnt() {
        assert!(ErasureCoder::new(0, 4).is_err());
        assert!(ErasureCoder::new(8, 0).is_err());
        assert!(ErasureCoder::new(200, 100).is_err());
        assert!(ErasureCoder::new(8, 4).is_ok());
    }

    #[test]
    fn standardparameter_sind_acht_und_vier() {
        let c = ErasureCoder::default();
        assert_eq!((c.k(), c.m(), c.n()), (8, 4, 12));
    }

    // ── Codierung ───────────────────────────────────────────────────

    #[test]
    fn datenfragmente_bleiben_unveraendert() {
        // Systematischer Code: die ersten k Fragmente sind die Eingabe.
        let c = ErasureCoder::default();
        let daten: Vec<u8> = (0..64u8).collect();
        let f = c.encode(&daten).expect("encode");
        assert_eq!(f.len(), 12);
        for (j, frag) in f.iter().take(8).enumerate() {
            assert_eq!(frag.index, j);
            assert_eq!(frag.data, &daten[j * 8..(j + 1) * 8]);
        }
    }

    #[test]
    fn leere_eingabe_wird_abgelehnt() {
        assert_eq!(
            ErasureCoder::default().encode(&[]).unwrap_err(),
            ErasureError::EmptyInput
        );
    }

    #[test]
    fn vollstaendige_fragmente_rekonstruieren() {
        let c = ErasureCoder::default();
        let daten: Vec<u8> = (0..64u8).collect();
        let f = c.encode(&daten).expect("encode");
        let zurueck = c.decode(&f[..8]).expect("decode");
        assert_eq!(&zurueck[..daten.len()], &daten[..]);
    }

    /// **Die tragende Eigenschaft.** Aus *jeder* Teilmenge von k aus n
    /// Fragmenten muss die Rekonstruktion gelingen — das ist der Grund
    /// fuer die Cauchy- statt Vandermonde-Konstruktion. Hier nicht
    /// behauptet, sondern fuer alle 495 Teilmengen durchgefahren.
    #[test]
    fn jede_k_aus_n_teilmenge_rekonstruiert() {
        let c = ErasureCoder::default();
        let daten: Vec<u8> = (0..96u8).map(|i| i.wrapping_mul(7).wrapping_add(3)).collect();
        let alle = c.encode(&daten).expect("encode");

        let mut geprueft = 0usize;
        // Alle 12 ueber 8 Teilmengen als Bitmasken.
        for maske in 0u32..(1 << 12) {
            if maske.count_ones() != 8 {
                continue;
            }
            let teil: Vec<Fragment> = (0..12)
                .filter(|i| maske & (1 << i) != 0)
                .map(|i| alle[i].clone())
                .collect();
            let zurueck = c
                .decode(&teil)
                .unwrap_or_else(|e| panic!("Maske {:012b}: {:?}", maske, e));
            assert_eq!(
                &zurueck[..daten.len()],
                &daten[..],
                "Maske {:012b} rekonstruiert falsch",
                maske
            );
            geprueft += 1;
        }
        assert_eq!(geprueft, 495, "es gibt genau 495 Teilmengen von 8 aus 12");
    }

    #[test]
    fn maximaler_ausfall_wird_noch_getragen() {
        // k=8, m=4: vier beliebige Fragmente duerfen fehlen.
        let c = ErasureCoder::default();
        let daten: Vec<u8> = (0..80u8).collect();
        let alle = c.encode(&daten).expect("encode");
        // Die vier Datenfragmente 0..4 fallen aus — der schlimmste Fall
        // fuer einen systematischen Code.
        let rest: Vec<Fragment> = alle[4..].to_vec();
        assert_eq!(rest.len(), 8);
        let zurueck = c.decode(&rest).expect("decode");
        assert_eq!(&zurueck[..daten.len()], &daten[..]);
    }

    #[test]
    fn ein_fragment_zu_wenig_ist_ein_definierter_ausfall() {
        // Kein Bug, sondern das erwartete Verhalten — und es muss als
        // solches erkennbar sein, nicht als Muellrekonstruktion.
        let c = ErasureCoder::default();
        let daten: Vec<u8> = (0..64u8).collect();
        let alle = c.encode(&daten).expect("encode");
        assert_eq!(
            c.decode(&alle[..7]).unwrap_err(),
            ErasureError::NotEnoughFragments { have: 7, need: 8 }
        );
    }

    // ── Beschädigte Eingaben ────────────────────────────────────────

    #[test]
    fn doppelter_index_wird_abgelehnt() {
        // Sonst waere die Matrix singulaer und die Rekonstruktion
        // lieferte stillschweigend Unsinn.
        let c = ErasureCoder::default();
        let alle = c.encode(&(0..64u8).collect::<Vec<_>>()).expect("encode");
        let mut teil = alle[..8].to_vec();
        teil[7] = teil[0].clone();
        assert_eq!(
            c.decode(&teil).unwrap_err(),
            ErasureError::DuplicateIndex { index: 0 }
        );
    }

    #[test]
    fn index_ausserhalb_des_bereichs_wird_abgelehnt() {
        let c = ErasureCoder::default();
        let alle = c.encode(&(0..64u8).collect::<Vec<_>>()).expect("encode");
        let mut teil = alle[..8].to_vec();
        teil[3].index = 99;
        assert_eq!(
            c.decode(&teil).unwrap_err(),
            ErasureError::IndexOutOfRange { index: 99 }
        );
    }

    #[test]
    fn uneinheitliche_laengen_werden_abgelehnt() {
        let c = ErasureCoder::default();
        let alle = c.encode(&(0..64u8).collect::<Vec<_>>()).expect("encode");
        let mut teil = alle[..8].to_vec();
        teil[2].data.push(0);
        assert_eq!(
            c.decode(&teil).unwrap_err(),
            ErasureError::InconsistentLength
        );
    }

    // ── Determinismus und Randfälle ─────────────────────────────────

    #[test]
    fn codierung_ist_deterministisch() {
        // Kap. 10.3: dieselbe Eingabe muss auf jedem Knoten dieselben
        // Fragmente ergeben.
        let c = ErasureCoder::default();
        let daten: Vec<u8> = (0..100u8).collect();
        assert_eq!(c.encode(&daten).unwrap(), c.encode(&daten).unwrap());
    }

    #[test]
    fn eingabe_kuerzer_als_k_funktioniert() {
        let c = ErasureCoder::default();
        let daten = vec![42u8, 7, 9];
        let alle = c.encode(&daten).expect("encode");
        let zurueck = c.decode(&alle[4..]).expect("decode");
        assert_eq!(&zurueck[..daten.len()], &daten[..]);
    }

    #[test]
    fn andere_parametrierung_traegt_ebenfalls() {
        // k=3, m=2: alle 10 Teilmengen von 3 aus 5.
        let c = ErasureCoder::new(3, 2).expect("coder");
        let daten: Vec<u8> = (0..30u8).collect();
        let alle = c.encode(&daten).expect("encode");
        let mut geprueft = 0;
        for maske in 0u32..(1 << 5) {
            if maske.count_ones() != 3 {
                continue;
            }
            let teil: Vec<Fragment> = (0..5)
                .filter(|i| maske & (1 << i) != 0)
                .map(|i| alle[i].clone())
                .collect();
            let zurueck = c.decode(&teil).expect("decode");
            assert_eq!(&zurueck[..daten.len()], &daten[..]);
            geprueft += 1;
        }
        assert_eq!(geprueft, 10);
    }
}
