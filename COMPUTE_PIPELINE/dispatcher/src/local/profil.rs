//! **Welche Experten eines Gemischs wie oft gewaehlt werden.**
//!
//! # ⚑ Wozu
//!
//! Ein Gemisch waehlt je Token und Ebene wenige Experten, und **diese Wahl
//! ist schief**: Gemessen am int8-35B (40 Ebenen, 256 Experten, 8 je Token)
//! an 2036 Token aus deutscher Dokumentation, Rust und Englisch tragen die
//! haeufigsten 20 % der Experten 70 % der Aufrufe, die haeufigsten 50 %
//! schon 94 %. Wer den Speicher fuer Experten vergibt, vergibt ihn deshalb
//! zuerst an die haeufigen: Ein Platz fuer einen haeufigen Experten spart
//! bei vielen Token einen Weg zur Platte, einer fuer einen seltenen fast
//! nie.
//!
//! ⚠️ **Ein Profil sagt, wie viel Speicher die Experten zuerst bekommen,
//! nicht welche ihn dauerhaft belegen.** Festgehalten nach Profil waren sie
//! in der Belegungssimulation vom 2026-10-15 kaum besser als der Dateicache
//! allein, auf kleinem Speicher schlechter (siehe `vorwaermen_im_hintergrund`
//! im Client): Was ein Text gerade braucht, weiss die juengste Wahl besser
//! als eine alte Zaehlung. Das Profil waermt beim Laden vor und bemisst den
//! Platz im Plan.
//!
//! Ein Profil ist eine Zaehlung je Ebene und Experte. ⚑ **Es entsteht im
//! Betrieb**: Die Laufzeit zaehlt jede Wahl mit, und der Client fuehrt nach
//! jeder Antwort das Profil dieser Maschine fort ([`Expertenprofil::fortschreiben`]).
//! ⚠️ **Es gilt fuer das, woran es erhoben wurde**; eine andere Arbeit
//! waehlt teils andere Experten. Deshalb steht neben jeder Rangfolge die
//! Zahl der Token, auf der sie beruht, und alte Wahl verliert mit der Zeit
//! an Gewicht ([`PROFIL_FENSTER`]).

use crate::local::bedarf::Modellbedarf;
use std::path::Path;

/// **Der Anteil der Aufrufe, den die heissen Experten tragen**: 800 Promille.
///
/// ⚑ **Aus der gemessenen Kurve gewaehlt, nicht hergeleitet.** Am int8-35B
/// (2036 Token, 2026-10-15) trugen 20 % der Experten 70,3 %, 30 % schon
/// 81,9 % der Aufrufe; 80 % liegen also bei 28 % der Experten (2877, rund
/// 8,4 GiB). Darueber waechst der Speicher je gewonnenem Prozent schnell:
/// Fuer die letzten 20 % der Aufrufe braucht es fast drei Viertel der
/// Experten. 📌 An nur 155 Token sah dieselbe Kurve schiefer aus (20 % trugen
/// 76 %); ein kurzer Text beruehrt weniger Experten.
pub const HEISS_DECKUNG_PROMILLE: u32 = 800;

/// **Ab wie vielen Token alte Wahl die Haelfte ihres Gewichts verliert**:
/// 2^20, rund eine Million.
///
/// ⚑ Ein Profil, das nur waechst, haelt an der Arbeit des ersten Monats
/// fest. Mit dem Halbieren zaehlt die juengste Million Token etwa so viel
/// wie alles davor zusammen. ⚠️ Eine Vorgabe und keine Messung: Wie schnell
/// sich die Wahl mit der Arbeit verschiebt, ist nicht gemessen.
pub const PROFIL_FENSTER: u64 = 1 << 20;

/// Die Haelfte, zur naechsten geraden Zahl gerundet, wie jede Division in
/// diesem Projekt.
fn halb(v: u32) -> u32 {
    let k = v >> 1;
    if v & 1 == 1 {
        k + (k & 1)
    } else {
        k
    }
}

/// Eine Zaehlung der Expertenwahl.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Expertenprofil {
    /// Fuer welches Artefakt (sein Ordnername).
    pub modell: String,
    /// Wie viele Token durchgelaufen sind.
    pub token: u64,
    /// `zaehler[ebene][experte]`: wie oft gewaehlt.
    pub zaehler: Vec<Vec<u32>>,
}

impl Expertenprofil {
    /// Liest ein Profil; fehlt die Datei, `Ok(None)`, ist sie kaputt, ein
    /// Fehler (kaputt ist nicht leer, wie beim Eichbuch).
    pub fn lesen(pfad: &Path) -> Result<Option<Self>, String> {
        match std::fs::read_to_string(pfad) {
            Ok(t) => serde_json::from_str(&t).map(Some).map_err(|e| format!("{}: {e}", pfad.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", pfad.display())),
        }
    }

    /// Schreibt das Profil, zuerst daneben und dann umbenannt.
    pub fn schreiben(&self, pfad: &Path) -> Result<(), String> {
        let text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        let neben = pfad.with_extension("unfertig");
        std::fs::write(&neben, text).map_err(|e| format!("{}: {e}", neben.display()))?;
        std::fs::rename(&neben, pfad).map_err(|e| format!("{}: {e}", pfad.display()))
    }

    /// Zaehlt eine Wahl.
    pub fn zaehlen(&mut self, ebene: usize, experte: usize) {
        if self.zaehler.len() <= ebene {
            self.zaehler.resize(ebene + 1, Vec::new());
        }
        let z = &mut self.zaehler[ebene];
        if z.len() <= experte {
            z.resize(experte + 1, 0);
        }
        z[experte] = z[experte].saturating_add(1);
    }

    /// **Fuehrt das Profil um eine abgeholte Wahl fort**: `wahl` je
    /// Gemischebene `(Ebene, Zaehler je Experte)`, wie die Laufzeit sie
    /// liefert, `top_k` die Experten je Token. Die Token zaehlen aus der
    /// ersten Ebene; ueberschreitet das Profil damit [`PROFIL_FENSTER`],
    /// verliert alles Bisherige vorher die Haelfte.
    pub fn fortschreiben(&mut self, wahl: &[(usize, Vec<u32>)], top_k: u32) {
        let neu = wahl.first().map_or(0, |(_, z)| z.iter().map(|&x| u64::from(x)).sum::<u64>() / u64::from(top_k.max(1)));
        if neu == 0 {
            return;
        }
        if self.token.saturating_add(neu) > PROFIL_FENSTER {
            for z in self.zaehler.iter_mut().flatten() {
                *z = halb(*z);
            }
            self.token = (self.token + (self.token >> 1 & 1)) >> 1;
        }
        for (ebene, z) in wahl {
            for (experte, &n) in z.iter().enumerate().filter(|(_, &n)| n > 0) {
                if self.zaehler.len() <= *ebene {
                    self.zaehler.resize(ebene + 1, Vec::new());
                }
                let reihe = &mut self.zaehler[*ebene];
                if reihe.len() <= experte {
                    reihe.resize(experte + 1, 0);
                }
                reihe[experte] = reihe[experte].saturating_add(n);
            }
        }
        self.token += neu;
    }

    fn zahl(&self, ebene: usize, experte: usize) -> u64 {
        u64::from(self.zaehler.get(ebene).and_then(|z| z.get(experte)).copied().unwrap_or(0))
    }

    /// Alle Aufrufe zusammen.
    pub fn gesamt(&self) -> u64 {
        self.zaehler.iter().flatten().map(|&z| u64::from(z)).sum()
    }

    /// **Alle Experten des Artefakts, die haeufigsten zuerst**, als
    /// `(ebene, experte)`. Bei gleicher Zahl entscheidet die Lage (Ebene,
    /// dann Nummer), damit die Rangfolge auf jeder Maschine dieselbe ist.
    /// Experten, die das Profil nicht kennt, zaehlen null.
    pub fn rangfolge(&self, b: &Modellbedarf) -> Vec<(usize, usize)> {
        let mut alle: Vec<(usize, usize)> =
            b.ebenen.iter().flat_map(|e| (0..e.experten_bytes.len()).map(move |x| (e.index, x))).collect();
        alle.sort_by(|a, c| self.zahl(c.0, c.1).cmp(&self.zahl(a.0, a.1)).then(a.cmp(c)));
        alle
    }

    /// **Wie viele der vordersten Experten der Rangfolge `promille` der
    /// Aufrufe tragen**; null ohne einen einzigen Aufruf.
    pub fn heisse(&self, rang: &[(usize, usize)], promille: u32) -> usize {
        let gesamt = self.gesamt();
        if gesamt == 0 {
            return 0;
        }
        let mut summe = 0u64;
        for (i, &(e, x)) in rang.iter().enumerate() {
            summe += self.zahl(e, x);
            // summe / gesamt >= promille / 1000, ueber Kreuz und ganzzahlig.
            if summe * 1000 >= gesamt * u64::from(promille) {
                return i + 1;
            }
        }
        rang.len()
    }

    /// **Welcher Anteil der Aufrufe** (in Promille) auf diese Experten faellt.
    pub fn deckung_promille(&self, experten: &[(usize, usize)]) -> u32 {
        let gesamt = self.gesamt();
        if gesamt == 0 {
            return 0;
        }
        let summe: u64 = experten.iter().map(|&(e, x)| self.zahl(e, x)).sum();
        u32::try_from(summe * 1000 / gesamt).unwrap_or(1000)
    }
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::local::bedarf::{Ebene, Ebenenart};

    fn bedarf() -> Modellbedarf {
        Modellbedarf {
            name: "g".into(),
            rand_bytes: 0,
            rand_selten_bytes: 0,
            ebenen: (0..2).map(|i| Ebene { index: i, art: Ebenenart::Achtsam, dicht_bytes: 0, experten_bytes: vec![1; 3] }).collect(),
            kv_je_token: 0,
            zustand_je_folge: 0,
            hoechstkontext: 16,
        }
    }

    /// ⚑ **Die haeufigsten zuerst, bei Gleichstand nach Lage**, und
    /// Unbekannte zaehlen null.
    #[test]
    fn die_rangfolge_folgt_den_zaehlern() {
        let mut p = Expertenprofil::default();
        for (e, x, n) in [(1, 2, 5), (0, 1, 3), (1, 0, 3), (0, 0, 1)] {
            for _ in 0..n {
                p.zaehlen(e, x);
            }
        }
        assert_eq!(p.rangfolge(&bedarf()), vec![(1, 2), (0, 1), (1, 0), (0, 0), (0, 2), (1, 1)]);
        assert_eq!(p.gesamt(), 12);
    }

    /// ⚑ **Heiss ist, was zusammen den verlangten Anteil traegt.**
    #[test]
    fn heiss_ist_die_spitze_bis_zur_deckung() {
        let mut p = Expertenprofil::default();
        for (e, x, n) in [(1, 2, 5), (0, 1, 3), (1, 0, 1), (0, 0, 1)] {
            for _ in 0..n {
                p.zaehlen(e, x);
            }
        }
        let rang = p.rangfolge(&bedarf());
        assert_eq!(p.heisse(&rang, 500), 1, "5 von 10 tragen die Haelfte");
        assert_eq!(p.heisse(&rang, 800), 2, "5 + 3 von 10");
        assert_eq!(p.heisse(&rang, 1000), 4);
        assert_eq!(p.deckung_promille(&rang[..2]), 800);
        assert_eq!(Expertenprofil::default().heisse(&rang, 800), 0, "ohne Aufrufe nichts heiss");
    }

    /// ⚑ **Fortschreiben addiert, und ueber dem Fenster verliert das Alte
    /// die Haelfte**, zur geraden Zahl gerundet.
    #[test]
    fn fortschreiben_addiert_und_halbiert_das_alte() {
        let mut p = Expertenprofil { modell: "m".into(), ..Default::default() };
        // Zwei Token, top_k 2: Ebene 3 waehlt Experte 1 zweimal, 4 und 0 je einmal.
        p.fortschreiben(&[(3, vec![1, 2, 0, 0, 1]), (5, vec![0, 4])], 2);
        assert_eq!((p.token, p.zahl(3, 1), p.zahl(3, 4), p.zahl(5, 1), p.zahl(0, 0)), (2, 2, 1, 4, 0));
        p.fortschreiben(&[(3, vec![0, 0, 0, 0, 0])], 2);
        assert_eq!(p.token, 2, "ohne Wahl kein Token");
        // Am Fenster: 5 -> 2, 7 -> 4, 3 -> 2, 2 -> 1, 1 -> 0 (halb, zur geraden Zahl).
        let mut q = Expertenprofil { modell: "m".into(), token: PROFIL_FENSTER, zaehler: vec![vec![5, 7, 3, 2, 1]] };
        q.fortschreiben(&[(0, vec![0, 0, 0, 0, 0, 2])], 2);
        assert_eq!(q.zaehler[0], vec![2, 4, 2, 1, 0, 2]);
        assert_eq!(q.token, PROFIL_FENSTER / 2 + 1);
    }

    #[test]
    fn ein_kaputtes_profil_ist_ein_fehler() {
        let ort = std::env::temp_dir().join(format!("profil-probe-{}", std::process::id()));
        let _ = std::fs::remove_file(&ort);
        assert_eq!(Expertenprofil::lesen(&ort), Ok(None));
        let mut p = Expertenprofil { modell: "m".into(), token: 3, ..Default::default() };
        p.zaehlen(0, 4);
        p.schreiben(&ort).expect("schreiben");
        assert_eq!(Expertenprofil::lesen(&ort), Ok(Some(p)));
        std::fs::write(&ort, "{").expect("kaputt");
        assert!(Expertenprofil::lesen(&ort).is_err());
        let _ = std::fs::remove_file(&ort);
    }
}
