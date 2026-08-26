//! Wachstumsoperator: ein Modell wächst, ohne dabei schlechter zu werden
//! (Whitepaper Kap. 7.5, Punkt 1.2).
//!
//! ## Warum ganzzahlig besser ist als in Gleitkomma
//!
//! Die Literatur (Net2Net, bert2BERT) verdoppelt eine Einheit und
//! **halbiert** ihre ausgehenden Gewichte. In Gleitkomma ist das nur
//! näherungsweise funktionserhaltend, und die beiden Kopien bekommen
//! danach identische Gradienten: Sie bleiben für immer gleich, die neue
//! Kapazität ist tot. Die Literatur behilft sich mit künstlichem
//! Rauschen, das nicht deterministisch ist und deshalb hier ausscheidet.
//!
//! Ganzzahlig löst eine **Aufteilung** beide Probleme auf einmal:
//!
//! ```text
//! a = ⌊m / 2⌋        b = m − a        a + b = m
//! ```
//!
//! `a + b = m` gilt für jede ganze Zahl, gerade wie ungerade. Es gibt
//! keinen Rundungsfehler, weil nichts gerundet wird. Und bei jedem
//! ungeraden Eintrag trennt die Aufteilung `a` und `b` um genau ein LSB,
//! **ohne jeden Zufall**: Die Symmetrie bricht von selbst.
//!
//! ## Das Akzeptanzkriterium ist ein Digestvergleich
//!
//! „Verhält sich nachweislich identisch zum Vorgänger" ist hier **kein
//! Toleranzvergleich**. Die Ausgabe vor und nach der Expansion muss
//! bitgleich sein, geprüft über einen Digest. Vorgemessen in
//! `tests/diag/expansion_simulation.py`: Abweichung `0,00e+00`.
//!
//! ## Was hier nicht steht
//!
//! Der Trainingsschritt selbst, das stochastische Runden und der
//! zählerbasierte Würfel. Sie sind in `tests/diag/` gemessen (0.1 und
//! 0.2) und gehören in die Umsetzung des Trainingsschritts, nicht in den
//! Wachstumsoperator. Dieser Operator arbeitet auf **Mastergewichten**
//! und ist damit unabhängig davon, wie daraus int8 wird.

use myl_types::hash::Hash;

/// Eine Gewichtsmatrix in Master-Darstellung, zeilenweise abgelegt.
///
/// Der Master trägt int8-Bereich plus `F` Nachkommabits (siehe
/// `tests/diag/results/bitbudget_uebersicht.md`); für den
/// Wachstumsoperator spielt `F` keine Rolle, weil er nur addiert und
/// aufteilt und dabei nie die Skala verlässt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Master {
    pub zeilen: usize,
    pub spalten: usize,
    /// `zeilen · spalten` Werte, zeilenweise.
    pub werte: Vec<i64>,
}

/// Fehler des Wachstumsoperators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WachstumsFehler {
    /// Die Werteanzahl passt nicht zu `zeilen · spalten`.
    FormPasstNicht { zeilen: usize, spalten: usize, werte: usize },
    /// Die zu verdoppelnde Einheit liegt außerhalb.
    EinheitAusserhalb { einheit: usize, vorhanden: usize },
    /// Die beiden Matrizen passen nicht aneinander.
    MatrizenPassenNicht { aus_spalten: usize, ein_zeilen: usize },
}

impl std::fmt::Display for WachstumsFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FormPasstNicht { zeilen, spalten, werte } => write!(
                f,
                "{}x{} verlangt {} Werte, angegeben sind {}",
                zeilen, spalten, zeilen * spalten, werte
            ),
            Self::EinheitAusserhalb { einheit, vorhanden } => write!(
                f,
                "Einheit {} soll verdoppelt werden, es gibt aber nur {}",
                einheit, vorhanden
            ),
            Self::MatrizenPassenNicht { aus_spalten, ein_zeilen } => write!(
                f,
                "die ausgehende Matrix hat {} Spalten, die eingehende {} Zeilen; \
                 beides sind versteckte Einheiten und muss übereinstimmen",
                aus_spalten, ein_zeilen
            ),
        }
    }
}

impl std::error::Error for WachstumsFehler {}

impl Master {
    pub fn neu(zeilen: usize, spalten: usize, werte: Vec<i64>) -> Result<Self, WachstumsFehler> {
        if werte.len() != zeilen * spalten {
            return Err(WachstumsFehler::FormPasstNicht {
                zeilen,
                spalten,
                werte: werte.len(),
            });
        }
        Ok(Self { zeilen, spalten, werte })
    }

    pub fn null(zeilen: usize, spalten: usize) -> Self {
        Self { zeilen, spalten, werte: vec![0; zeilen * spalten] }
    }

    pub fn at(&self, zeile: usize, spalte: usize) -> i64 {
        self.werte[zeile * self.spalten + spalte]
    }

    fn setzen(&mut self, zeile: usize, spalte: usize, wert: i64) {
        self.werte[zeile * self.spalten + spalte] = wert;
    }

    /// Digest über Form und Werte, little-endian.
    ///
    /// Die **Form geht ein**: Zwei Matrizen mit denselben Werten in
    /// anderer Anordnung sind verschiedene Matrizen, und ein Digest, der
    /// das nicht sieht, wäre für einen Vergleich vor und nach dem
    /// Wachstum unbrauchbar.
    pub fn digest(&self) -> Hash {
        let mut bytes = Vec::with_capacity(16 + self.werte.len() * 8);
        bytes.extend_from_slice(&(self.zeilen as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.spalten as u64).to_le_bytes());
        for w in &self.werte {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        Hash::sha256(&bytes)
    }
}

/// Die ganzzahlige Aufteilung: `a + b = m`, ohne Rundung.
///
/// **Das Herzstück.** Eine Halbierung wäre bei ungeraden Werten ungenau
/// und müsste runden; eine Aufteilung ist exakt, und der Rest von einem
/// LSB landet in einer der beiden Hälften statt im Fehler.
///
/// `⌊m/2⌋` ist hier die **abrundende** Division, auch für negative
/// Zahlen. Rusts `/` trunkiert zur Null, und `-3 / 2 = -1` ergäbe
/// `a + b = -1 + (-2) = -3`, also rechnerisch richtig, aber mit einem
/// anderen `a` als die Referenzsimulation (`torch.floor`). Zwei
/// Implementierungen desselben Operators müssen dasselbe liefern, sonst
/// ist der Digestvergleich wertlos.
pub fn aufteilen(m: i64) -> (i64, i64) {
    let a = m.div_euclid(2);
    (a, m - a)
}

/// Breitenwachstum: verdoppelt die versteckte Einheit `einheit`.
///
/// - **eingehend** (`ein`, Form `versteckt × modell`): Zeile `einheit`
///   wird kopiert und angehängt. Die Kopie sieht dieselbe Eingabe und
///   erzeugt dieselbe Aktivierung.
/// - **ausgehend** (`aus`, Form `modell × versteckt`): Spalte `einheit`
///   wird **aufgeteilt**; `a` bleibt an ihrem Platz, `b` kommt ans Ende.
///
/// Weil die Kopie dieselbe Aktivierung erzeugt und `a + b = m` gilt, ist
/// der Beitrag der beiden zusammen exakt der Beitrag der einen zuvor.
/// **Das gilt für jede elementweise Aktivierungsfunktion**, denn beide
/// Einheiten bekommen denselben Eingang.
pub fn breite_wachsen(
    ein: &Master,
    aus: &Master,
    einheit: usize,
) -> Result<(Master, Master), WachstumsFehler> {
    if aus.spalten != ein.zeilen {
        return Err(WachstumsFehler::MatrizenPassenNicht {
            aus_spalten: aus.spalten,
            ein_zeilen: ein.zeilen,
        });
    }
    if einheit >= ein.zeilen {
        return Err(WachstumsFehler::EinheitAusserhalb {
            einheit,
            vorhanden: ein.zeilen,
        });
    }

    // Eingehend: Zeile anhängen.
    let mut ein_neu = Master {
        zeilen: ein.zeilen + 1,
        spalten: ein.spalten,
        werte: Vec::with_capacity((ein.zeilen + 1) * ein.spalten),
    };
    ein_neu.werte.extend_from_slice(&ein.werte);
    let start = einheit * ein.spalten;
    ein_neu
        .werte
        .extend_from_slice(&ein.werte[start..start + ein.spalten]);

    // Ausgehend: Spalte aufteilen, `b` als neue letzte Spalte.
    let mut aus_neu = Master::null(aus.zeilen, aus.spalten + 1);
    for z in 0..aus.zeilen {
        for s in 0..aus.spalten {
            aus_neu.setzen(z, s, aus.at(z, s));
        }
        let (a, b) = aufteilen(aus.at(z, einheit));
        aus_neu.setzen(z, einheit, a);
        aus_neu.setzen(z, aus.spalten, b);
    }

    Ok((ein_neu, aus_neu))
}

/// Tiefenwachstum: eine neue Ebene, die als Identität startet.
///
/// Im Residualstrom heißt Identität ein **Ausgabegewicht von null**:
/// `y = x + W_aus · f(W_ein · x)` ist genau dann `y = x`, wenn `W_aus`
/// null ist. Das ist exakt darstellbar und exakt prüfbar, anders als ein
/// „kleines" Gewicht.
///
/// Die eingehenden Gewichte übergibt der Aufrufer. Sie dürfen beliebig
/// sein, denn sie tragen zunächst nichts bei; sie bestimmen aber, welche
/// Richtungen die Ebene **später** lernen kann.
///
/// **Die Ebene bleibt nicht tot.** Der Gradient nach `W_aus` ist `aᵀ·g`
/// und hängt nicht von `W_aus` ab. Gemessen in
/// `tests/diag/tiefenwachstum_simulation.py`: Bewegung ab dem ersten
/// Schritt, mit stochastischem Runden 120 von 128 Gewichten über 20
/// Schritte, mit Rundung zur nächsten Stufe 33.
pub fn tiefe_wachsen(ein: Master, modellbreite: usize) -> (Master, Master) {
    let aus = Master::null(modellbreite, ein.zeilen);
    (ein, aus)
}

/// Ein Vorwärtsschritt in reiner Ganzzahlarithmetik, für die Prüfung der
/// Funktionserhaltung.
///
/// `y = aus · f(ein · x)` mit `f` als abschneidendem ReLU.
///
/// **Warum das genügt, obwohl das echte Modell SiLU rechnet:** Die
/// Funktionserhaltung hängt nicht an der Aktivierungsfunktion. Die
/// verdoppelte Einheit sieht dieselbe Eingabe wie das Original, erzeugt
/// also dieselbe Aktivierung, gleichgültig welche Funktion; und `a + b =
/// m` ist eine Eigenschaft der Aufteilung. Eine Funktion, die davon
/// abhinge, wäre nicht elementweise.
pub fn vorwaerts(ein: &Master, aus: &Master, x: &[i64]) -> Vec<i64> {
    let versteckt: Vec<i64> = (0..ein.zeilen)
        .map(|z| {
            let s: i64 = x
                .iter()
                .enumerate()
                .take(ein.spalten)
                .map(|(k, xi)| ein.at(z, k) * xi)
                .sum();
            s.max(0)
        })
        .collect();
    (0..aus.zeilen)
        .map(|z| {
            versteckt
                .iter()
                .enumerate()
                .take(aus.spalten)
                .map(|(k, v)| aus.at(z, k) * v)
                .sum()
        })
        .collect()
}

/// Digest über eine Ausgabe, für den Vergleich vor und nach dem Wachstum.
pub fn ausgabe_digest(y: &[i64]) -> Hash {
    let mut bytes = Vec::with_capacity(y.len() * 8);
    for v in y {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    Hash::sha256(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beispiel() -> (Master, Master, Vec<i64>) {
        // 3 versteckte Einheiten, 4 Eingänge, 2 Ausgänge.
        let ein = Master::neu(3, 4, vec![
            2, -1, 3, 0,
            -4, 5, 1, 2,
            7, 0, -2, 6,
        ]).unwrap();
        let aus = Master::neu(2, 3, vec![
            5, -3, 8,      // ungerade Einträge, damit die Aufteilung trennt
            -7, 4, 9,
        ]).unwrap();
        (ein, aus, vec![1, 2, -1, 3])
    }

    /// `a + b = m`, für jede ganze Zahl, gerade wie ungerade, positiv wie
    /// negativ.
    #[test]
    fn die_aufteilung_ist_exakt() {
        for m in -1000..=1000i64 {
            let (a, b) = aufteilen(m);
            assert_eq!(a + b, m, "m={m}");
        }
        for m in [i64::MIN + 1, i64::MAX, -1, 0, 1] {
            let (a, b) = aufteilen(m);
            assert_eq!(a + b, m, "m={m}");
        }
    }

    /// Abrundend, nicht zur Null trunkierend: Sonst weicht die Umsetzung
    /// von der Referenzsimulation ab, und der Digestvergleich wäre
    /// wertlos.
    #[test]
    fn abgerundet_wird_nach_unten_auch_bei_negativen() {
        assert_eq!(aufteilen(-3), (-2, -1));
        assert_eq!(aufteilen(3), (1, 2));
        assert_eq!(aufteilen(-4), (-2, -2));
        // Zur Null trunkierend wäre es (-1, -2), also ein anderes `a`.
        assert_ne!(aufteilen(-3).0, -3 / 2);
    }

    /// **Das Akzeptanzkriterium des Punkts.** Bitgleich, geprüft
    /// über einen Digest, nicht über eine Toleranz.
    #[test]
    fn breitenwachstum_ist_exakt_funktionserhaltend() {
        let (ein, aus, x) = beispiel();
        let vorher = vorwaerts(&ein, &aus, &x);

        for einheit in 0..ein.zeilen {
            let (ein_neu, aus_neu) = breite_wachsen(&ein, &aus, einheit).unwrap();
            let nachher = vorwaerts(&ein_neu, &aus_neu, &x);
            assert_eq!(
                ausgabe_digest(&vorher),
                ausgabe_digest(&nachher),
                "Einheit {einheit}: die Ausgabe muss bitgleich bleiben"
            );
            assert_eq!(ein_neu.zeilen, ein.zeilen + 1);
            assert_eq!(aus_neu.spalten, aus.spalten + 1);
        }
    }

    /// Und über viele zufällige Matrizen, damit es nicht am Beispiel hängt.
    #[test]
    fn funktionserhaltung_ueber_viele_faelle() {
        // xorshift64, damit der Fall reproduzierbar ist: Ein Test, der
        // bei jedem Lauf andere Zahlen zieht, meldet einen Fehler
        // irgendwann und danach nie wieder.
        fn naechste(z: &mut u64) -> i64 {
            *z ^= *z << 13;
            *z ^= *z >> 7;
            *z ^= *z << 17;
            (*z % 41) as i64 - 20
        }
        let mut zustand = 0x243F_6A88_85A3_08D3u64;
        for _ in 0..200 {
            let (h, d, o) = (4usize, 5usize, 3usize);
            let ein = Master::neu(h, d, (0..h * d).map(|_| naechste(&mut zustand)).collect())
                .unwrap();
            let aus = Master::neu(o, h, (0..o * h).map(|_| naechste(&mut zustand)).collect())
                .unwrap();
            let x: Vec<i64> = (0..d).map(|_| naechste(&mut zustand)).collect();
            let vorher = vorwaerts(&ein, &aus, &x);
            let einheit = (zustand % h as u64) as usize;
            let (e2, a2) = breite_wachsen(&ein, &aus, einheit).unwrap();
            assert_eq!(ausgabe_digest(&vorher), ausgabe_digest(&vorwaerts(&e2, &a2, &x)));
        }
    }

    /// **Die Symmetrie bricht ohne jeden Zufall**, an jedem ungeraden
    /// Eintrag um genau ein LSB. Ohne das wären die beiden Kopien für
    /// immer gleich und die neue Kapazität tot.
    #[test]
    fn die_aufteilung_bricht_die_symmetrie_ohne_zufall() {
        let (ein, aus, _) = beispiel();
        let (_, aus_neu) = breite_wachsen(&ein, &aus, 0).unwrap();
        let neu = aus_neu.spalten - 1;
        let verschieden = (0..aus_neu.zeilen)
            .filter(|&z| aus_neu.at(z, 0) != aus_neu.at(z, neu))
            .count();
        assert_eq!(
            verschieden, 2,
            "beide Zeilen tragen an Spalte 0 einen ungeraden Wert (5 und -7)"
        );

        // Gegenprobe: bei geraden Werten trennt die Aufteilung nicht, und
        // dann muss das stochastische Runden der eingehenden Zeilen die
        // Arbeit tun (gemessen in expansion_simulation.py).
        let gerade = Master::neu(1, 3, vec![4, 6, 8]).unwrap();
        let ein1 = Master::neu(3, 2, vec![1, 1, 1, 1, 1, 1]).unwrap();
        let (_, g2) = breite_wachsen(&ein1, &gerade, 0).unwrap();
        assert_eq!(g2.at(0, 0), g2.at(0, g2.spalten - 1));
    }

    /// Die Kopie sieht dieselbe Eingabe: Ihre Zeile ist die des Originals.
    #[test]
    fn die_kopie_ist_bitgleich_zum_original() {
        let (ein, aus, _) = beispiel();
        let (ein_neu, _) = breite_wachsen(&ein, &aus, 1).unwrap();
        let neu = ein_neu.zeilen - 1;
        for s in 0..ein.spalten {
            assert_eq!(ein_neu.at(1, s), ein_neu.at(neu, s));
        }
    }

    /// Tiefenwachstum: die neue Ebene ist die Identität, exakt.
    #[test]
    fn tiefenwachstum_startet_als_identitaet() {
        let ein = Master::neu(3, 4, vec![
            2, -1, 3, 0,
            -4, 5, 1, 2,
            7, 0, -2, 6,
        ]).unwrap();
        let (ein, aus) = tiefe_wachsen(ein, 4);
        let x = vec![1i64, 2, -1, 3];
        let beitrag = vorwaerts(&ein, &aus, &x);
        assert!(
            beitrag.iter().all(|&v| v == 0),
            "eine Identitätsebene darf im Residualstrom nichts beitragen"
        );
        assert_eq!(aus.zeilen, 4);
        assert_eq!(aus.spalten, 3);
    }

    /// Der Digest sieht die Form, nicht nur die Werte.
    #[test]
    fn der_digest_unterscheidet_die_form() {
        let a = Master::neu(2, 3, vec![1, 2, 3, 4, 5, 6]).unwrap();
        let b = Master::neu(3, 2, vec![1, 2, 3, 4, 5, 6]).unwrap();
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn unbrauchbare_eingaben_werden_abgelehnt() {
        assert!(matches!(
            Master::neu(2, 3, vec![1, 2]),
            Err(WachstumsFehler::FormPasstNicht { .. })
        ));
        let (ein, aus, _) = beispiel();
        assert!(matches!(
            breite_wachsen(&ein, &aus, 99),
            Err(WachstumsFehler::EinheitAusserhalb { .. })
        ));
        let schief = Master::neu(2, 7, vec![0; 14]).unwrap();
        assert!(matches!(
            breite_wachsen(&ein, &schief, 0),
            Err(WachstumsFehler::MatrizenPassenNicht { .. })
        ));
    }
}
