//! Die Sicherheitsbedingung `S_min = g/p²` (Whitepaper Kap. 5.5, Anhang B.1).
//!
//! Ein Miner betrügt rational nur, wenn der erwartete Gewinn die
//! erwartete Strafe übersteigt. Mit Stichprobenrate `p`, Stake `S` und
//! Betrugsgewinn je Segment `g` verlangt Kap. 5.5
//!
//! ```text
//! p · S > g/p    ⟺    S_min = g/p²
//! ```
//!
//! Die schärfere Form mit `p²` statt `p` kommt aus Anhang B.1: Sie
//! verlangt zusätzlich, dass sich Betrug auch über den Zeithorizont bis
//! zur ersten erwarteten Prüfung (≈ 1/p Segmente) nicht amortisiert, also
//! **gegen Miner, die nach kurzem Betrugsfenster mit Exit rechnen**
//! (Hit-and-Run). Ohne diesen Faktor wäre die Bedingung gegen einen
//! Angreifer, der ohnehin aussteigen will, wirkungslos.
//!
//! ## Warum diese Funktion hier steht und nicht in GOVERNANCE
//!
//! GOVERNANCE braucht sie, um Parametervorschläge zu prüfen (Punkt 1.3):
//! Wer `p` senkt oder `g` hebt, ohne den Stake anzuheben, verletzt die
//! Bedingung. TOKENOMICS verlangt sie deshalb
//! ausdrücklich als Funktion, die von GOVERNANCE **benutzt** wird, statt
//! sie dort ein zweites Mal zu schreiben.
//!
//! Das ist keine Förmlichkeit. Das Audit vom 2026-08-18 fand mit A7 einen
//! Fall, in dem dieselbe Formel an zwei Orten stand und nur eine davon
//! gepflegt wurde; die Akzeptanzkriterien dieses Projekts nennen seither
//! „keine zweite, abweichende Implementierung derselben Formel"
//! ausdrücklich als Bedingung.
//!
//! **Ganzzahlig:** `p` kommt als Bruch `zaehler/nenner` herein, damit
//! keine Gleitkommazahl in den Konsenspfad gerät. `S_min = g · nenner² /
//! zaehler²`, gerechnet in `u128` und **aufgerundet**: Abrunden würde
//! einen Stake knapp unter der Schranke zulassen, und die Schranke ist
//! eine Untergrenze.

/// Fehler der `S_min`-Berechnung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SicherheitsFehler {
    /// Die Stichprobenrate ist null oder ihr Nenner ist null.
    ///
    /// `p = 0` heißt „es wird nie geprüft"; dann gibt es keinen Stake, der
    /// hoch genug wäre, und `g/p²` ist keine Zahl. Das ist kein
    /// Randfall, sondern die Aussage der Formel.
    UnbrauchbareStichprobenrate { zaehler: u64, nenner: u64 },
    /// Die Rate liegt über 1 (mehr als jedes Segment geprüft).
    RateUeberEins { zaehler: u64, nenner: u64 },
    /// Ein Bruch hat den Nenner null.
    UnbrauchbarerBruch,
    /// Der Realkostenanteil `c` liegt bei 1 oder darüber.
    ///
    /// Dann ist `c/(1−c)` keine Zahl. Sachlich hieße es, die Rechenkosten
    /// erreichten den Reward; dann lohnt sich Mining überhaupt nicht.
    KostenanteilNichtUnterEins { zaehler: u64, nenner: u64 },
    /// Der erforderliche Stake passt nicht mehr in `u64`.
    ///
    /// Tritt bei sehr kleinem `p` ein und ist eine echte Aussage: Der
    /// Parametersatz verlangt einen Stake, den das Zahlensystem des
    /// Ledgers nicht darstellen kann.
    StakeNichtDarstellbar,
}

impl std::fmt::Display for SicherheitsFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnbrauchbareStichprobenrate { zaehler, nenner } => write!(
                f,
                "Stichprobenrate {}/{} ist null oder undefiniert; ohne Prüfung gibt es keine Schranke",
                zaehler, nenner
            ),
            Self::RateUeberEins { zaehler, nenner } => write!(
                f,
                "Stichprobenrate {}/{} liegt über 1",
                zaehler, nenner
            ),
            Self::UnbrauchbarerBruch => write!(f, "ein Bruch hat den Nenner null"),
            Self::KostenanteilNichtUnterEins { zaehler, nenner } => write!(
                f,
                "Realkostenanteil c = {}/{} liegt bei 1 oder darüber; c/(1-c) ist dann keine Zahl",
                zaehler, nenner
            ),
            Self::StakeNichtDarstellbar => write!(
                f,
                "der erforderliche Stake übersteigt den u64-Bereich"
            ),
        }
    }
}

impl std::error::Error for SicherheitsFehler {}

/// Die Stichprobenrate der Stufe 2, als Bruch: **die eine maßgebliche
/// Stelle** (Fund 171, 2026-09-04).
///
/// # ⚑ Warum sie hier steht und nicht dreimal woanders
///
/// Bis zum 2026-09-04 gab es diese Größe an **drei** Stellen mit
/// **zwei** Werten:
///
/// | Ort | Wert | Wer las ihn |
/// |---|---|---|
/// | `ParameterRegistry`-Vorgabe | 5/100 | niemand |
/// | `Kette::STICHPROBE_BP` | 200 bp = 2/100 | **die Kette, bei jeder Ziehung** |
/// | ein Literal in der Vorgabe von `MindestStake` | 5/100 | setzte den Mindest-Einsatz |
///
/// ⚑ **Und der Unterschied war kein Schreibfehler, sondern eine
/// vergessene Entscheidung.** Entscheidung **A1** hat am 2026-09-02 die
/// Kontrollsegmente entfernt. Bis dahin teilten sich zwei Linien die
/// Arbeit: die Stichprobe `p` und die Kontrollsegmente `gamma`. Ohne
/// `gamma` muss `p` beides tragen, und
/// `security_sim.py::zusammengelegte_rate` rechnet aus, welche Rate
/// **beide gleichwertig ersetzt**: 4,96 %, aufgerundet fünf.
///
/// Die Registry hat den neuen Wert übernommen. **Die Kette nicht.**
/// `Kette::STICHPROBE_BP` trug weiter die 200 bp „aus Kap. 3.4 und dem
/// Zahlenbeispiel in Anhang B.1", also den Wert aus der Welt **mit**
/// Kontrollsegmenten. Damit hat die Kette seit A1 mit einer Rate
/// gestichprobt, die nur zusammen mit einer zweiten Linie trug, **die es
/// nicht mehr gibt**. Dieselbe Klasse wie Fund 151.
///
/// # Die Regel dahinter
///
/// Eine maßgebliche Stelle je Größe, alles andere leitet ab. So halten
/// es Cosmos SDK (ADR-046: was Governance ändert, liegt im Zustand und
/// wird beim Ausführen gelesen) und Substrate (Konstanten nur per
/// Runtime-Upgrade, alles Änderbare im Speicher, und als Ketten
/// Ratsbeschlüsse wollten, liess Parity den Konstantentyp aus dem
/// Speicher lesen statt eine zweite Kopie zu erlauben).
///
/// ⚑ **Der Endzustand ist trotzdem ein anderer:** Kap. 10 nennt `p`
/// ausdrücklich als abstimmbar, sie gehört also in den Konsenszustand,
/// nicht in eine Konstante. Bis der Konsens die Registry liest (B10),
/// ist diese Konstante die eine Stelle, und ein `const`-Riegel hält die
/// Ableitungen daran.
pub const STICHPROBE_ZAEHLER: u64 = 5;

/// Der Nenner zu [`STICHPROBE_ZAEHLER`].
pub const STICHPROBE_NENNER: u64 = 100;

/// Dieselbe Rate in Basispunkten, für die Ziehung.
///
/// `sample_segments` rechnet in Zehntausendsteln; die Umrechnung steht
/// hier, damit sie nicht an jeder Aufrufstelle neu entsteht.
pub const fn stichprobe_bp() -> u32 {
    (STICHPROBE_ZAEHLER * 10_000 / STICHPROBE_NENNER) as u32
}

/// `S_min = g/p²` mit `p = zaehler/nenner`, aufgerundet.
///
/// **Parameter:**
/// - `betrugsgewinn_g`: Gewinn aus einem betrogenen Segment, in
///   Kleinstbeträgen
/// - `p_zaehler`, `p_nenner`: die Stichprobenrate als Bruch
///
/// **Returns:** der Mindest-Stake in Kleinstbeträgen.
///
/// **Aufgerundet**, weil die Schranke eine Untergrenze ist: Ein
/// abgerundeter Wert ließe einen Stake knapp darunter als „ausreichend"
/// durchgehen, und genau diese Lücke wäre der rationale Angriff.
pub fn s_min(
    betrugsgewinn_g: u64,
    p_zaehler: u64,
    p_nenner: u64,
) -> Result<u64, SicherheitsFehler> {
    if p_zaehler == 0 || p_nenner == 0 {
        return Err(SicherheitsFehler::UnbrauchbareStichprobenrate {
            zaehler: p_zaehler,
            nenner: p_nenner,
        });
    }
    if p_zaehler > p_nenner {
        return Err(SicherheitsFehler::RateUeberEins {
            zaehler: p_zaehler,
            nenner: p_nenner,
        });
    }

    // S_min = g · nenner² / zaehler², aufgerundet.
    let nenner_quadrat = (p_nenner as u128).saturating_mul(p_nenner as u128);
    let zaehler_quadrat = (p_zaehler as u128) * (p_zaehler as u128);
    let zaehler_gesamt = (betrugsgewinn_g as u128).saturating_mul(nenner_quadrat);
    let aufgerundet = zaehler_gesamt.div_ceil(zaehler_quadrat);

    u64::try_from(aufgerundet).map_err(|_| SicherheitsFehler::StakeNichtDarstellbar)
}

/// Prüft, ob ein hinterlegter Stake die Sicherheitsbedingung erfüllt.
///
/// Bequemlichkeit für GOVERNANCE: `stake ≥ S_min(g, p)`.
pub fn stake_genuegt(
    stake: u64,
    betrugsgewinn_g: u64,
    p_zaehler: u64,
    p_nenner: u64,
) -> Result<bool, SicherheitsFehler> {
    Ok(stake >= s_min(betrugsgewinn_g, p_zaehler, p_nenner)?)
}

/// Das **untere** Ende des Realkosten-Bandes aus Anhang B.4, Zähler.
///
/// Das Papier nennt `c` „empirisch 0,6–0,8". Geprüft wird die
/// Self-Dealing-Grenze gegen **0,6**, nicht gegen einen abstimmbaren
/// Wert, und das ist eine Entscheidung mit einem Grund (⚑ Fund 49):
///
/// `c` ist keine Protokollgröße, sondern eine Beobachtung über die Welt.
/// Stünde es als Parameter in der Prüfung, ließe sich die Grenze in
/// **zwei je zulässigen Schritten** verschieben: erst `c` heben (die
/// Ungleichung bleibt erfüllt), dann `s` unter die neue, höhere Grenze.
/// Beide Vorschläge bestünden die Prüfung, das Ergebnis verletzte die
/// Bedingung, denn das wahre `c` hat sich nicht bewegt.
///
/// Gegen das untere Bandende geprüft, gilt die Schranke auch dann, wenn
/// die realen Kosten am unteren Rand des beobachteten Bereichs liegen.
/// Die Grenze ist damit `0,6/0,4 = 1,5`; die Start-Subvention `s = 0,5`
/// liegt mit dem Dreifachen Abstand darunter, die Anlaufphase ist also
/// nicht betroffen.
///
/// **Steigt das beobachtete Band später nachweislich**, ist das eine
/// Änderung dieser Konstante, also ein Code-Diff mit Fundstelle, und
/// keine Abstimmung mit Nebenwirkung.
pub const KOSTENANTEIL_UNTEN_ZAEHLER: u64 = 6;
/// Nenner des unteren Bandendes ([`KOSTENANTEIL_UNTEN_ZAEHLER`]).
pub const KOSTENANTEIL_UNTEN_NENNER: u64 = 10;

/// Ist Self-Dealing bei dieser Subventionsrate verlustbringend, gemessen
/// am **unteren** Ende des Realkosten-Bandes?
///
/// Die Form, in der GOVERNANCE die Bedingung prüft. Sie nimmt kein `c`
/// entgegen, denn genau das war die Lücke aus Fund 49.
pub fn self_dealing_sicher_konservativ(
    s_zaehler: u64,
    s_nenner: u64,
) -> Result<bool, SicherheitsFehler> {
    self_dealing_sicher(
        s_zaehler,
        s_nenner,
        KOSTENANTEIL_UNTEN_ZAEHLER,
        KOSTENANTEIL_UNTEN_NENNER,
    )
}

/// Ist Self-Dealing bei dieser Subventionsrate verlustbringend?
///
/// Anhang B.4: In der Subventionsphase (`s > 0`) genügt der reine
/// Burn-Mint-Vergleich nicht, denn mit `M_e = B̄_e·(1+s)` erntet ein
/// Self-Dealer nominell mehr, als er verbrennt. Die Sicherheit beruht
/// dort auf der **Arbeitsbindung** der Prägung: Rewards fließen nur gegen
/// verifizierte Rechenarbeit, deren reale Kosten (Anteil `c` am Reward,
/// empirisch 0,6 bis 0,8) der Angreifer wie jeder Miner trägt.
/// Verlustbringend ist Self-Dealing genau dann, wenn
///
/// ```text
/// s < c / (1 − c)
/// ```
///
/// Das Papier führt die Ungleichung ausdrücklich „als
/// **Governance-Invariante**"; durchgesetzt wird sie in `myl-governance`
/// (Punkt 1.3), gerechnet wird sie hier — dieselbe Arbeitsteilung wie
/// bei [`s_min`], damit die Formel nur an einem Ort steht.
///
/// **Strikt**, nicht `≤`: Bei Gleichheit ist Self-Dealing weder
/// verlustbringend noch gewinnbringend, und ein Angreifer, der es
/// kostenlos betreiben kann, betreibt es.
///
/// **Ganzzahlig ausgewertet:** `s < c/(1−c)` ⟺
/// `s_zaehler · (c_nenner − c_zaehler) < c_zaehler · s_nenner`. Beide
/// Seiten in `u128`, keine Division, also keine Rundung, die die Antwort
/// an der Grenze kippen könnte.
///
/// **Fehler** bei `c ≥ 1` oder Nenner null: Dann ist `c/(1−c)` keine
/// Zahl. `c ≥ 1` hieße, die Rechenkosten erreichten oder überstiegen den
/// Reward; dann lohnt sich Mining überhaupt nicht, und die Frage nach
/// Self-Dealing stellt sich nicht.
pub fn self_dealing_sicher(
    s_zaehler: u64,
    s_nenner: u64,
    c_zaehler: u64,
    c_nenner: u64,
) -> Result<bool, SicherheitsFehler> {
    if s_nenner == 0 || c_nenner == 0 {
        return Err(SicherheitsFehler::UnbrauchbarerBruch);
    }
    if c_zaehler >= c_nenner {
        return Err(SicherheitsFehler::KostenanteilNichtUnterEins {
            zaehler: c_zaehler,
            nenner: c_nenner,
        });
    }
    let eins_minus_c = (c_nenner - c_zaehler) as u128;
    let links = (s_zaehler as u128) * eins_minus_c;
    let rechts = (c_zaehler as u128) * (s_nenner as u128);
    Ok(links < rechts)
}

/// Die Grenze `c/(1−c)` als gekürzter Bruch, für Meldungen und Berichte.
///
/// Nicht für die Entscheidung selbst: Dafür ist
/// [`self_dealing_sicher`] zuständig, das ohne Division auskommt.
pub fn self_dealing_grenze(
    c_zaehler: u64,
    c_nenner: u64,
) -> Result<(u64, u64), SicherheitsFehler> {
    if c_nenner == 0 {
        return Err(SicherheitsFehler::UnbrauchbarerBruch);
    }
    if c_zaehler >= c_nenner {
        return Err(SicherheitsFehler::KostenanteilNichtUnterEins {
            zaehler: c_zaehler,
            nenner: c_nenner,
        });
    }
    Ok((c_zaehler, c_nenner - c_zaehler))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UNITS_PER_MYL;

    /// **Das Zahlenbeispiel aus Kap. 5.5 und Anhang B.1**, exakt.
    ///
    /// „Bei p = 2 % und g = Reward eines Segments folgt S_min = 2500
    /// Segment-Rewards" und „p = 0,02, Segment-Reward g = 0,5 MYL folgt
    /// S_min = 1250 MYL pro Segment-Kapazität."
    ///
    /// Das ist das Akzeptanzkriterium von TOKENOMICS Punkt 3.3, und es
    /// wird gegen **beide** im Papier genannten Formen geprüft: den
    /// Faktor und den absoluten Betrag.
    #[test]
    fn zahlenbeispiel_aus_kapitel_5_5() {
        // Form 1: der Faktor. g = 1 Einheit → S_min = 2500 Einheiten.
        assert_eq!(s_min(1, 2, 100).unwrap(), 2_500);

        // Form 2: der absolute Betrag. g = 0,5 MYL → S_min = 1250 MYL.
        let g = UNITS_PER_MYL / 2;
        assert_eq!(s_min(g, 2, 100).unwrap(), 1_250 * UNITS_PER_MYL);
    }

    /// **Anhang B.8.2**, die quadratische Wirkung der Stichprobenrate.
    ///
    /// „Da S_min quadratisch von p abhängt, sinkt der Bedarf für
    /// zweihundert Miner von 250.000 MYL bei zwei Prozent auf 40.000 bei
    /// fünf, 10.000 bei zehn, 1.600 bei fünfundzwanzig und 400 MYL bei
    /// fünfzig Prozent."
    ///
    /// Nachgerechnet: 200 Miner mit je einer Kapazitätseinheit, g = 0,5
    /// MYL. Die Tabelle des Papiers muss Zahl für Zahl herauskommen.
    #[test]
    fn anhang_b_8_2_quadratische_wirkung() {
        let g = UNITS_PER_MYL / 2;
        let miner = 200u64;
        let faelle: &[(u64, u64, u64)] = &[
            // (p in Prozent, erwartet in MYL für 200 Miner)
            (2, 100, 250_000),
            (5, 100, 40_000),
            (10, 100, 10_000),
            (25, 100, 1_600),
            (50, 100, 400),
        ];
        for &(p_zaehler, p_nenner, erwartet_myl) in faelle {
            let je_einheit = s_min(g, p_zaehler, p_nenner).unwrap();
            let gesamt = je_einheit * miner;
            assert_eq!(
                gesamt,
                erwartet_myl * UNITS_PER_MYL,
                "p = {p_zaehler}/{p_nenner}: {} statt {} MYL",
                gesamt / UNITS_PER_MYL,
                erwartet_myl
            );
        }
    }

    /// **Anhang B.8.1**, der Bedarf der Anlaufphase.
    ///
    /// „Bei der Zielrate von zwei Prozent beträgt S_min je
    /// Kapazitätseinheit 1.250 MYL. Für fünfzig Startminer ergibt das
    /// einen Stake-Bedarf von 62.500 MYL."
    #[test]
    fn anhang_b_8_1_anlaufphase() {
        let g = UNITS_PER_MYL / 2;
        let je_einheit = s_min(g, 2, 100).unwrap();
        assert_eq!(je_einheit * 50, 62_500 * UNITS_PER_MYL);
    }

    /// Aufgerundet wird, weil die Schranke eine Untergrenze ist.
    #[test]
    fn wird_aufgerundet_nicht_ab() {
        // g = 1, p = 2/3 → 1 · 9/4 = 2,25 → 3, nicht 2.
        assert_eq!(s_min(1, 2, 3).unwrap(), 3);
        // p = 1 (jedes Segment geprüft) → S_min = g, exakt.
        assert_eq!(s_min(7, 1, 1).unwrap(), 7);
    }

    /// Eine Rate von null hat keine Schranke, und das ist die Aussage der
    /// Formel, kein Randfall.
    #[test]
    fn ohne_pruefung_keine_schranke() {
        assert!(matches!(
            s_min(1_000, 0, 100),
            Err(SicherheitsFehler::UnbrauchbareStichprobenrate { .. })
        ));
        assert!(matches!(
            s_min(1_000, 2, 0),
            Err(SicherheitsFehler::UnbrauchbareStichprobenrate { .. })
        ));
        assert!(matches!(
            s_min(1_000, 3, 2),
            Err(SicherheitsFehler::RateUeberEins { .. })
        ));
    }

    /// Ein Parametersatz, der einen nicht darstellbaren Stake verlangt,
    /// muss als Fehler herauskommen und nicht als kleine Zahl.
    #[test]
    fn nicht_darstellbarer_stake_ist_ein_fehler() {
        assert!(matches!(
            s_min(u64::MAX, 1, u64::MAX),
            Err(SicherheitsFehler::StakeNichtDarstellbar)
        ));
    }

    /// `S_min` fällt monoton mit steigender Stichprobenrate. Wäre es
    /// anders, wäre „mehr prüfen" ein Nachteil.
    #[test]
    fn mehr_pruefen_verlangt_nie_mehr_stake() {
        let g = 1_000_000u64;
        let mut vorher = u64::MAX;
        for p in 1..=100u64 {
            let jetzt = s_min(g, p, 100).unwrap();
            assert!(jetzt <= vorher, "p = {p}: {jetzt} > {vorher}");
            vorher = jetzt;
        }
    }

    #[test]
    fn stake_genuegt_entscheidet_an_der_schranke() {
        let schranke = s_min(1, 2, 100).unwrap(); // 2500
        assert!(!stake_genuegt(schranke - 1, 1, 2, 100).unwrap());
        assert!(stake_genuegt(schranke, 1, 2, 100).unwrap());
        assert!(stake_genuegt(schranke + 1, 1, 2, 100).unwrap());
    }
}

#[cfg(test)]
mod self_dealing_tests {
    use super::*;

    /// **Das Zahlenbeispiel aus Anhang B.4**, exakt.
    ///
    /// „Bei c = 0,7 also s < 2,33, die Start-Subvention s = 0,5 liegt
    /// weit darunter."
    #[test]
    fn zahlenbeispiel_aus_anhang_b4() {
        // Die Grenze bei c = 0,7 ist 7/3 = 2,333…
        assert_eq!(self_dealing_grenze(7, 10).unwrap(), (7, 3));
        // Die Start-Subvention s = 0,5 ist sicher.
        assert!(self_dealing_sicher(1, 2, 7, 10).unwrap());
        // 2,33 ebenfalls, knapp.
        assert!(self_dealing_sicher(233, 100, 7, 10).unwrap());
    }

    /// Die Bedingung ist **strikt**: Bei Gleichheit ist Self-Dealing
    /// weder verlustbringend noch gewinnbringend, und wer es kostenlos
    /// betreiben kann, betreibt es.
    #[test]
    fn bei_gleichheit_ist_es_nicht_mehr_sicher() {
        assert!(!self_dealing_sicher(7, 3, 7, 10).unwrap());
        assert!(!self_dealing_sicher(3, 1, 7, 10).unwrap());
        // Und einen Hauch darunter gilt es noch.
        assert!(self_dealing_sicher(699_999, 300_000, 7, 10).unwrap());
    }

    /// Das ganze Band aus Anhang B.4: c von 0,6 bis 0,8.
    ///
    /// Die Grenze wandert dabei von 1,5 auf 4,0 — **Fund 49 in Zahlen**:
    /// Wer `c` bestimmen darf, bestimmt die Grenze mit.
    #[test]
    fn das_band_aus_anhang_b4() {
        assert_eq!(self_dealing_grenze(6, 10).unwrap(), (6, 4)); // 1,5
        assert_eq!(self_dealing_grenze(7, 10).unwrap(), (7, 3)); // 2,333
        assert_eq!(self_dealing_grenze(8, 10).unwrap(), (8, 2)); // 4,0

        // s = 2 ist bei c = 0,7 sicher und bei c = 0,6 nicht.
        assert!(self_dealing_sicher(2, 1, 7, 10).unwrap());
        assert!(!self_dealing_sicher(2, 1, 6, 10).unwrap());
    }

    /// `c ≥ 1` ist keine Rechnung, sondern ein Parameterfehler.
    #[test]
    fn ein_kostenanteil_ab_eins_ist_ein_fehler() {
        for c in [(10u64, 10u64), (11, 10), (u64::MAX, 1)] {
            assert!(matches!(
                self_dealing_sicher(1, 2, c.0, c.1),
                Err(SicherheitsFehler::KostenanteilNichtUnterEins { .. })
            ));
            assert!(self_dealing_grenze(c.0, c.1).is_err());
        }
        assert!(matches!(
            self_dealing_sicher(1, 0, 7, 10),
            Err(SicherheitsFehler::UnbrauchbarerBruch)
        ));
    }

    /// Die Entscheidung darf an keiner Eingabe umlaufen oder abstürzen.
    #[test]
    fn extreme_eingaben_laufen_nicht_um() {
        let faelle = [
            (u64::MAX, 1u64, 1u64, u64::MAX),
            (1, u64::MAX, u64::MAX - 1, u64::MAX),
            (u64::MAX, u64::MAX, 1, 2),
            (0, 1, 1, 2),
        ];
        for (sz, sn, cz, cn) in faelle {
            let _ = self_dealing_sicher(sz, sn, cz, cn);
        }
        // s = 0 ist immer sicher: ohne Subvention gibt es nichts zu ernten.
        assert!(self_dealing_sicher(0, 1, 1, 1_000_000).unwrap());
    }
}

#[cfg(test)]
mod konservativ_tests {
    use super::*;

    /// Die konservative Grenze ist `0,6/0,4 = 1,5`.
    #[test]
    fn die_konservative_grenze_ist_eineinhalb() {
        assert_eq!(
            self_dealing_grenze(KOSTENANTEIL_UNTEN_ZAEHLER, KOSTENANTEIL_UNTEN_NENNER).unwrap(),
            (6, 4)
        );
        // s = 1,49 gilt, s = 1,5 nicht mehr (die Bedingung ist strikt).
        assert!(self_dealing_sicher_konservativ(149, 100).unwrap());
        assert!(!self_dealing_sicher_konservativ(150, 100).unwrap());
        assert!(!self_dealing_sicher_konservativ(3, 2).unwrap());
    }

    /// **Die Anlaufphase ist nicht betroffen.**
    ///
    /// Kap. 5.7 setzt die Start-Subvention auf `s = 0,5`. Sie liegt mit
    /// dem Dreifachen Abstand unter der konservativen Grenze.
    #[test]
    fn die_startsubvention_hat_dreifachen_abstand() {
        assert!(self_dealing_sicher_konservativ(1, 2).unwrap());
        // Und noch das Dreifache davon gilt.
        assert!(self_dealing_sicher_konservativ(3, 2 * 3 / 3).is_ok());
        assert!(self_dealing_sicher_konservativ(149, 100).unwrap());
    }

    /// **Fund 49 ist damit geschlossen: Kein `c` kann die Grenze heben.**
    ///
    /// Die konservative Prüfung nimmt gar kein `c` entgegen. Der Test
    /// hält fest, dass die Antwort für jedes `s` dieselbe bleibt, egal
    /// welches `c` jemand anderswo einträgt — die Zwei-Schritte-Lücke
    /// hat keinen ersten Schritt mehr.
    #[test]
    fn kein_kostenanteil_verschiebt_die_konservative_grenze() {
        for s in 0..=300u64 {
            let konservativ = self_dealing_sicher_konservativ(s, 100).unwrap();
            // Bei c = 0,8 wäre die Grenze 4,0 statt 1,5; die konservative
            // Antwort darf sich davon nicht bewegen.
            let bei_c_08 = self_dealing_sicher(s, 100, 8, 10).unwrap();
            if (150..400).contains(&s) {
                assert!(
                    !konservativ && bei_c_08,
                    "s = {s}/100: genau hier trennen sich die beiden Fassungen"
                );
            }
            assert_eq!(
                konservativ,
                self_dealing_sicher(s, 100, 6, 10).unwrap(),
                "die konservative Fassung ist die mit c = 0,6"
            );
        }
    }
}

// ---------------------------------------------------------------------
// Burn-Cap je Adresse (Kap. 5.6)
// ---------------------------------------------------------------------

/// Anteil des geglätteten Burns, den **eine Adresse** je Epoche
/// verbrennen darf, Zähler.
///
/// Kap. 5.6 nennt das Gegenmittel bereits: „In der Subventionsphase
/// (`s > 0`) wird Self-Dealing durch die EMA-Glättung und ein **Burn-Cap
/// pro Adresse** gedämpft." Es war bis zum 2026-08-24 nicht
/// implementiert.
///
/// ## Was der Deckel abfängt
///
/// Die Epochensimulation aus K8 zeigte einen Verlauf, der ohne Betrug
/// auskommt und trotzdem teuer ist: Wer den Verbrauch hochtreibt und dann
/// aussteigt, lässt eine Prägung zurück, die der EMA nachläuft. Zwischen
/// Epoche 100 und 125 wuchs der Umlauf so von 282 auf 30 222 MYL. **Ob
/// sich das für einen Angreifer lohnt, hängt am Preis** und ist mit der
/// Überschlagsrechnung nicht beantwortet.
///
/// Der Deckel beantwortet die Frage nicht, er **begrenzt sie**: Eine
/// einzelne Adresse kann den geglätteten Burn nicht mehr im Alleingang
/// bewegen. Wer den Stoß trotzdem will, braucht `1/anteil` Adressen mit
/// je eigener Deckung, und das ist keine Sybil-Frage mehr, sondern eine
/// Kapitalfrage: Die MYL müssen tatsächlich vorhanden sein.
///
/// ## Warum ein Zwanzigstel
///
/// Bei zwanzig gleich großen Verbrauchern wäre der Deckel gerade nicht
/// bindend. Das ist die Größenordnung, ab der von einem Netz und nicht
/// von einem Betreiber die Rede sein kann; ein einzelner Teilnehmer, der
/// mehr als ein Zwanzigstel des gesamten geglätteten Verbrauchs
/// ausmacht, ist selbst das Risiko.
///
/// **Der Wert ist eine Festlegung dieses Entwurfs**, kein Wert aus dem
/// Papier, und er gehört in die Governance-Registry.
pub const BURN_DECKEL_ZAEHLER: u64 = 1;
/// Nenner des Burn-Deckels ([`BURN_DECKEL_ZAEHLER`]).
pub const BURN_DECKEL_NENNER: u64 = 20;

/// Wie viel eine Adresse in dieser Epoche noch verbrennen darf.
///
/// **Parameter:**
/// - `geglaetteter_burn`: `B̄_e`, der EMA-Wert der laufenden Epoche
/// - `bereits_verbrannt`: was diese Adresse in dieser Epoche schon
///   verbrannt hat
///
/// **In der Anlaufphase greift der Deckel nicht.** Solange `B̄_e` klein
/// ist, wäre ein Zwanzigstel davon ebenfalls klein, und die ersten Nutzer
/// kämen nicht an ihre Credits. Deshalb gilt er erst ab einem
/// Mindestvolumen ([`BURN_DECKEL_AB`]); darunter ist er unbegrenzt.
///
/// Das ist kein Schlupfloch: Unterhalb dieses Volumens ist die Prägung
/// ohnehin zu klein, als dass sich der beschriebene Stoß lohnte.
pub fn burn_spielraum(geglaetteter_burn: u64, bereits_verbrannt: u64) -> u64 {
    if geglaetteter_burn < BURN_DECKEL_AB {
        return u64::MAX;
    }
    let deckel = ((geglaetteter_burn as u128) * BURN_DECKEL_ZAEHLER as u128
        / BURN_DECKEL_NENNER as u128) as u64;
    deckel.saturating_sub(bereits_verbrannt)
}

/// Volumen, ab dem der Burn-Deckel greift, in Kleinstbeträgen.
///
/// 1000 MYL geglätteter Burn je Epoche. Darunter ist das Netz so klein,
/// dass ein Deckel mehr Nutzer aussperrt als Angreifer bremst.
pub const BURN_DECKEL_AB: u64 = 1_000 * crate::UNITS_PER_MYL;

#[cfg(test)]
mod burn_deckel_tests {
    use super::*;
    use crate::UNITS_PER_MYL;

    /// In der Anlaufphase greift der Deckel nicht.
    #[test]
    fn in_der_anlaufphase_greift_der_deckel_nicht() {
        assert_eq!(burn_spielraum(0, 0), u64::MAX);
        assert_eq!(burn_spielraum(BURN_DECKEL_AB - 1, 0), u64::MAX);
        assert_eq!(burn_spielraum(999 * UNITS_PER_MYL, 10_000), u64::MAX);
    }

    /// Ab dem Mindestvolumen ist es ein Zwanzigstel.
    #[test]
    fn ab_dem_mindestvolumen_ist_es_ein_zwanzigstel() {
        let burn = 20_000 * UNITS_PER_MYL;
        assert_eq!(burn_spielraum(burn, 0), 1_000 * UNITS_PER_MYL);
        // Schon Verbranntes wird abgezogen.
        assert_eq!(burn_spielraum(burn, 400 * UNITS_PER_MYL), 600 * UNITS_PER_MYL);
        // Und der Spielraum wird nie negativ.
        assert_eq!(burn_spielraum(burn, 5_000 * UNITS_PER_MYL), 0);
    }

    /// **Zwanzig gleich große Verbraucher sind gerade nicht gedeckelt.**
    ///
    /// Das ist die Begründung des Wertes, und sie steht als Test, damit
    /// eine Änderung des Nenners sichtbar macht, was sie bedeutet.
    #[test]
    fn zwanzig_gleiche_verbraucher_sind_gerade_nicht_gedeckelt() {
        let burn = 100_000 * UNITS_PER_MYL;
        let je_verbraucher = burn / 20;
        assert!(burn_spielraum(burn, 0) >= je_verbraucher);
        // Einundzwanzig wären es auch, zwanzig ist die Grenze der Aussage.
        assert!(burn_spielraum(burn, je_verbraucher) == 0);
    }

    /// Der Deckel läuft an keiner Eingabe um.
    #[test]
    fn der_deckel_laeuft_nicht_um() {
        assert_eq!(burn_spielraum(u64::MAX, u64::MAX), 0);
        assert!(burn_spielraum(u64::MAX, 0) > 0);
        let _ = burn_spielraum(u64::MAX, 1);
    }
}
