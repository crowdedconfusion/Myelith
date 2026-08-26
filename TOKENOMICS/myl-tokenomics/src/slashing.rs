//! Die Slashing-Matrix als Datensatz (Punkt 3.2, Whitepaper Kap. 5.5).
//!
//! Kap. 5.5 gibt eine Tabelle aus Akteur, Grund und Höhe vor. Der
//! Verlangt ist sie „als Konfigurationsdatensatz, nicht
//! Hartkodierung", und das Akzeptanzkriterium nennt den Grund:
//! **ein einziger Ort der Wahrheit**.
//!
//! ## Die Arbeitsteilung, die dieses Modul einhält
//!
//! Drei Komponenten sind beteiligt, und jede tut genau eine Sache:
//!
//! | | Frage | Ort |
//! |---|---|---|
//! | VERIFICATION | **Wer** hat verloren? | `myl_verifier::slash` |
//! | TOKENOMICS | **Wie viel** ist das? | dieses Modul |
//! | CONSENSUS | Wie wird es **gebucht**? | `myl_ledger::apply_verdict` |
//!
//! Diese Trennung ist teuer erkauft. Bis v0.2.6 hatte `myl-verifier` eine
//! eigene `SlashConfig` mit **festen Beträgen** (1 MYL Slash, 0,5 MYL
//! Kopfgeld) — ein zweites, unvereinbares Modell neben dem des Ledgers,
//! das obendrein gar nicht buchen konnte (Fund A9). Ein fester Betrag hat
//! zudem keine Abschreckungswirkung: 1 MYL ist für einen Großstaker
//! nichts, und die ganze Sicherheitsannahme aus Kap. 6.9 (Betrug muss
//! teurer sein als der erwartete Gewinn) hängt genau daran.
//!
//! Dieses Modul liefert deshalb **Anteile**, keine Beträge, und zwar in
//! genau der Form, die [`myl_ledger::transitions::SlashParams`] erwartet.
//!
//! ## Die Staffelung der beiden Spannen (entschieden 2026-08-24)
//!
//! Zwei Zeilen von Kap. 5.5 nennen eine **Spanne** statt eines Wertes:
//! „1–5 % (gestaffelt)" bei Nichtverfügbarkeit und „30–100 %" beim
//! Validator. **Wonach gestaffelt wird, steht nirgends.**
//!
//! Gestaffelt wird nach **Wiederholung innerhalb der Historie von zehn
//! Epochen** ([`WIEDERHOLUNGSFENSTER`]). Das ist der einzige
//! Anknüpfungspunkt, den das Protokoll **schon kennt**: Der Ledger führt
//! Konten, und der Epochenabschluss kennt Urteile je Miner. Eine
//! Staffelung nach Schaden (Dauer des Ausfalls, Zahl betroffener
//! Sessions) wäre sachgerechter und verlangt eine Schadensmessung, die es
//! nicht gibt; sie bliebe damit eine Absichtserklärung.
//!
//! | Verstoß | Nichtverfügbarkeit | Validator |
//! |---|---|---|
//! | erster | 1 % | 30 % |
//! | zweiter | 3 % | 65 % |
//! | ab dem dritten | 5 % | 100 % |
//!
//! Die Stufen liegen jeweils am unteren Rand, in der Mitte und am oberen
//! Rand der Spanne aus Kap. 5.5. **Der erste Schritt bleibt am unteren
//! Ende**, und das ist die eigentliche Entscheidung: Ein zu niedriger
//! Slash schwächt die Abschreckung und ist durch eine Parameteränderung
//! heilbar; ein zu hoher vernichtet den Einsatz eines ehrlichen
//! Teilnehmers und ist es nicht. Wer einmal ausfällt, hatte vielleicht
//! eine schlechte Nacht; wer dreimal ausfällt, hat ein anderes Problem.
//!
//! **Was das voraussetzt:** einen Zähler je Miner im Ledger-Zustand. Der
//! ist ein Konsensfeld; bis er existiert, nimmt [`satz_gestaffelt`] die
//! Zahl der Vorverstöße als Eingabe entgegen, und der Aufrufer
//! verantwortet sie.

use myl_ledger::transitions::SlashParams;

/// Wer geschlachtet wird (Kap. 5.5, Spalte „Akteur").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Akteur {
    /// Shard-Miner, Stake proportional zur beanspruchten Kapazität.
    ShardMiner,
    /// Pod-Koordinator, Zusatz-Stake.
    PodKoordinator,
    /// Validator, BFT-Stake.
    Validator,
    /// Checker, Kaution je Anfechtung.
    Checker,
}

/// Warum geschlachtet wird (Kap. 5.5, Spalte „Slash-Grund").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grund {
    /// Falsches Ergebnis, per Bisektion bewiesen.
    FalschesErgebnis,
    /// Nichtverfügbarkeit während einer Session.
    Nichtverfuegbarkeit,
    /// Falsche PoI-Aggregation.
    FalscheAggregation,
    /// Double-Signing oder bewiesene Zensur.
    DoubleSigningOderZensur,
    /// Mutwillig falsche Anfechtung.
    MutwilligeAnfechtung,
}

/// Woher der geschlachtete Betrag kommt.
///
/// Der Checker verliert **die Kaution seiner Anfechtung**, nicht einen
/// Anteil eines laufenden Stakes. Ohne diese Unterscheidung würde die
/// Matrix so aussehen, als ginge es überall um dasselbe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bemessungsgrundlage {
    /// Anteil des hinterlegten Stakes.
    Stake,
    /// Die Kaution der Anfechtung, in voller Höhe.
    Kaution,
}

/// Ein Eintrag der Matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slashsatz {
    pub akteur: Akteur,
    pub grund: Grund,
    pub grundlage: Bemessungsgrundlage,
    /// Anteil als Bruch, Zähler.
    pub anteil_zaehler: u64,
    /// Anteil als Bruch, Nenner.
    pub anteil_nenner: u64,
    /// Untere Grenze der von Kap. 5.5 genannten Spanne, in Basispunkten.
    pub spanne_min_bps: u64,
    /// Obere Grenze der Spanne, in Basispunkten.
    pub spanne_max_bps: u64,
}

impl Slashsatz {
    /// Der Anteil in Basispunkten.
    pub fn anteil_bps(&self) -> u64 {
        (self.anteil_zaehler as u128 * 10_000 / self.anteil_nenner as u128) as u64
    }

    /// Liegt der gesetzte Anteil in der Spanne aus Kap. 5.5?
    pub fn in_der_spanne(&self) -> bool {
        let bps = self.anteil_bps();
        (self.spanne_min_bps..=self.spanne_max_bps).contains(&bps)
    }

    /// Der Satz als Ledger-Parameter, mit dem Kopfgeld aus Anhang B.3.
    pub fn als_ledger_parameter(&self) -> SlashParams {
        SlashParams {
            slash_fraction_num: self.anteil_zaehler,
            slash_fraction_den: self.anteil_nenner,
            bounty_fraction_num: KOPFGELD_ZAEHLER,
            bounty_fraction_den: KOPFGELD_NENNER,
        }
    }
}

/// Fenster, in dem Verstöße als Wiederholung zählen (in Epochen).
///
/// Zehn Epochen, dieselbe Länge wie die Arbeitshistorie des
/// Stimmgewichts (`myl_consensus::voting_weight::MAX_HISTORY_EPOCHS`).
/// **Dieselbe Länge ist kein Zufall, sondern die Absicht:** Beide
/// beantworten dieselbe Frage, nämlich wie lange das Verhalten eines
/// Teilnehmers nachwirkt, und zwei verschiedene Antworten darauf wären
/// schwer zu begründen.
pub const WIEDERHOLUNGSFENSTER: u64 = 10;

/// Kopfgeldanteil `b` am geschlachteten Betrag (Anhang B.3: b = 30 %).
///
/// „Checker-Vergütung = Grundvergütung (4 % der Prägung, proportional zu
/// geprüftem Volumen) + Kopfgeld `b·S` aus Slashes (b = 30 %)."
///
/// Der Rest bleibt unverteilt, ist also faktisch verbrannt. **Das ist
/// Absicht:** Ginge der volle Slash an den Checker, wäre eine
/// erfolgreiche Anfechtung ein Geschäft, und ein Checker, der einen
/// Miner zum Betrug verleiten kann, verdient daran.
pub const KOPFGELD_ZAEHLER: u64 = 30;
/// Nenner des Kopfgeldanteils.
pub const KOPFGELD_NENNER: u64 = 100;

/// Die Matrix aus Kap. 5.5, Zeile für Zeile.
///
/// **Jede Zeile des Papiers steht hier, und keine Zeile steht hier, die
/// nicht im Papier steht.** Der Test `matrix_deckt_kapitel_5_5` hält das
/// fest.
pub fn matrix() -> [Slashsatz; 5] {
    use Akteur::*;
    use Bemessungsgrundlage::*;
    use Grund::*;
    [
        // „Shard-Miner | falsches Ergebnis (per Bisektion bewiesen) | 100 % Stake"
        Slashsatz {
            akteur: ShardMiner,
            grund: FalschesErgebnis,
            grundlage: Stake,
            anteil_zaehler: 1,
            anteil_nenner: 1,
            spanne_min_bps: 10_000,
            spanne_max_bps: 10_000,
        },
        // „Shard-Miner | Nichtverfügbarkeit während Session | 1–5 % (gestaffelt)"
        Slashsatz {
            akteur: ShardMiner,
            grund: Nichtverfuegbarkeit,
            grundlage: Stake,
            anteil_zaehler: 1,
            anteil_nenner: 100,
            spanne_min_bps: 100,
            spanne_max_bps: 500,
        },
        // „Pod-Koordinator | falsche PoI-Aggregation | 100 %"
        Slashsatz {
            akteur: PodKoordinator,
            grund: FalscheAggregation,
            grundlage: Stake,
            anteil_zaehler: 1,
            anteil_nenner: 1,
            spanne_min_bps: 10_000,
            spanne_max_bps: 10_000,
        },
        // „Validator | Double-Signing / Zensur (bewiesen) | 30–100 %"
        Slashsatz {
            akteur: Validator,
            grund: DoubleSigningOderZensur,
            grundlage: Stake,
            anteil_zaehler: 30,
            anteil_nenner: 100,
            spanne_min_bps: 3_000,
            spanne_max_bps: 10_000,
        },
        // „Checker | Kaution pro Anfechtung | mutwillig falsche Anfechtung | Kaution"
        Slashsatz {
            akteur: Checker,
            grund: MutwilligeAnfechtung,
            grundlage: Kaution,
            anteil_zaehler: 1,
            anteil_nenner: 1,
            spanne_min_bps: 10_000,
            spanne_max_bps: 10_000,
        },
    ]
}

/// Der Satz für ein Paar aus Akteur und Grund, erster Verstoß.
pub fn satz(akteur: Akteur, grund: Grund) -> Option<Slashsatz> {
    matrix()
        .into_iter()
        .find(|s| s.akteur == akteur && s.grund == grund)
}

/// Der Satz für ein Paar, gestaffelt nach der Zahl der **Vorverstöße**
/// innerhalb von [`WIEDERHOLUNGSFENSTER`] Epochen.
///
/// `vorverstoesse = 0` ist der erste Verstoß. Bei Zeilen ohne Spanne
/// (100 % Stake, Kaution) ändert die Staffelung nichts; dort gibt es
/// nichts zu steigern.
///
/// **Der Anteil bleibt stets in der Spanne aus Kap. 5.5.** Das ist die
/// Invariante der Matrix und gilt auch für gestaffelte Sätze.
pub fn satz_gestaffelt(akteur: Akteur, grund: Grund, vorverstoesse: u64) -> Option<Slashsatz> {
    let basis = satz(akteur, grund)?;
    if basis.spanne_min_bps == basis.spanne_max_bps {
        return Some(basis);
    }
    // Drei Stufen: unterer Rand, Mitte, oberer Rand.
    let bps = match vorverstoesse {
        0 => basis.spanne_min_bps,
        1 => (basis.spanne_min_bps + basis.spanne_max_bps) / 2,
        _ => basis.spanne_max_bps,
    };
    Some(Slashsatz {
        anteil_zaehler: bps,
        anteil_nenner: 10_000,
        ..basis
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Die Matrix deckt Kap. 5.5 Zeile für Zeile**, und nicht mehr.
    ///
    /// Eine Zeile zu viel wäre eine erfundene Slash-Regel; eine zu wenig
    /// wäre ein Vergehen ohne Folge.
    #[test]
    fn matrix_deckt_kapitel_5_5() {
        let m = matrix();
        assert_eq!(m.len(), 5, "Kap. 5.5 nennt fünf Zeilen");
        let paare: Vec<(Akteur, Grund)> = m.iter().map(|s| (s.akteur, s.grund)).collect();
        assert!(paare.contains(&(Akteur::ShardMiner, Grund::FalschesErgebnis)));
        assert!(paare.contains(&(Akteur::ShardMiner, Grund::Nichtverfuegbarkeit)));
        assert!(paare.contains(&(Akteur::PodKoordinator, Grund::FalscheAggregation)));
        assert!(paare.contains(&(Akteur::Validator, Grund::DoubleSigningOderZensur)));
        assert!(paare.contains(&(Akteur::Checker, Grund::MutwilligeAnfechtung)));
        // Kein Paar doppelt: sonst hinge die Höhe daran, welchen Eintrag
        // der Aufrufer zuerst findet.
        let mut sortiert = paare.clone();
        sortiert.sort();
        sortiert.dedup();
        assert_eq!(sortiert.len(), paare.len());
    }

    /// Die Höhen stimmen mit der Tabelle des Papiers.
    #[test]
    fn die_hoehen_stimmen_mit_dem_papier() {
        assert_eq!(
            satz(Akteur::ShardMiner, Grund::FalschesErgebnis).unwrap().anteil_bps(),
            10_000,
            "falsches Ergebnis: 100 %"
        );
        assert_eq!(
            satz(Akteur::PodKoordinator, Grund::FalscheAggregation).unwrap().anteil_bps(),
            10_000,
            "falsche Aggregation: 100 %"
        );
        assert_eq!(
            satz(Akteur::ShardMiner, Grund::Nichtverfuegbarkeit).unwrap().anteil_bps(),
            100,
            "Nichtverfügbarkeit: unteres Ende von 1 bis 5 %"
        );
        assert_eq!(
            satz(Akteur::Validator, Grund::DoubleSigningOderZensur).unwrap().anteil_bps(),
            3_000,
            "Double-Signing: unteres Ende von 30 bis 100 %"
        );
    }

    /// **Jeder gesetzte Anteil liegt in der Spanne, die Kap. 5.5 nennt.**
    ///
    /// Das ist die Invariante der Matrix: Wer eine Höhe ändert, darf die
    /// Spanne des Papiers nicht verlassen, ohne das Papier zu ändern.
    #[test]
    fn jeder_satz_liegt_in_seiner_spanne() {
        for s in matrix() {
            assert!(
                s.in_der_spanne(),
                "{:?}/{:?}: {} bps liegt außerhalb von {}..{}",
                s.akteur,
                s.grund,
                s.anteil_bps(),
                s.spanne_min_bps,
                s.spanne_max_bps
            );
            assert!(s.spanne_min_bps <= s.spanne_max_bps);
            assert!(s.spanne_max_bps <= 10_000, "mehr als der volle Einsatz gibt es nicht");
        }
    }

    /// Der Satz wird zum Ledger-Parameter, ohne dass jemand eine Zahl
    /// abtippt.
    #[test]
    fn der_satz_wird_zum_ledger_parameter() {
        let s = satz(Akteur::ShardMiner, Grund::FalschesErgebnis).unwrap();
        let p = s.als_ledger_parameter();
        assert_eq!(p.slash_fraction_num, 1);
        assert_eq!(p.slash_fraction_den, 1);
        assert_eq!(p.bounty_fraction_num, KOPFGELD_ZAEHLER);
        assert_eq!(p.bounty_fraction_den, KOPFGELD_NENNER);
    }

    /// **Das Kopfgeld übersteigt nie den Slash**, für jede Zeile.
    ///
    /// Dieselbe Invariante prüft `myl-ledger` über zufällige Folgen; hier
    /// wird sie an der Quelle geprüft, also an den Parametern selbst.
    #[test]
    fn das_kopfgeld_uebersteigt_nie_den_slash() {
        for s in matrix() {
            let p = s.als_ledger_parameter();
            assert!(p.bounty_fraction_num <= p.bounty_fraction_den);
            assert!(p.slash_fraction_num <= p.slash_fraction_den);
        }
    }

    /// Ein Paar, das die Tabelle nicht kennt, hat keinen Satz.
    ///
    /// Ein Vorgabewert wäre hier gefährlich: Er machte aus einem nicht
    /// vorgesehenen Vorwurf eine buchbare Strafe.
    #[test]
    fn ein_unbekanntes_paar_hat_keinen_satz() {
        assert!(satz(Akteur::Checker, Grund::FalschesErgebnis).is_none());
        assert!(satz(Akteur::Validator, Grund::Nichtverfuegbarkeit).is_none());
    }
}

#[cfg(test)]
mod staffelung_tests {
    use super::*;

    /// **Die Staffelung, Stufe für Stufe** (entschieden 2026-08-24).
    #[test]
    fn die_staffelung_folgt_der_entscheidung() {
        let faelle: &[(Akteur, Grund, [u64; 3])] = &[
            (Akteur::ShardMiner, Grund::Nichtverfuegbarkeit, [100, 300, 500]),
            (Akteur::Validator, Grund::DoubleSigningOderZensur, [3_000, 6_500, 10_000]),
        ];
        for &(a, g, erwartet) in faelle {
            for (i, &bps) in erwartet.iter().enumerate() {
                let s = satz_gestaffelt(a, g, i as u64).unwrap();
                assert_eq!(s.anteil_bps(), bps, "{a:?}/{g:?} bei {i} Vorverstößen");
            }
            // Ab dem dritten bleibt es am oberen Rand.
            for n in 3..20u64 {
                assert_eq!(satz_gestaffelt(a, g, n).unwrap().anteil_bps(), erwartet[2]);
            }
        }
    }

    /// **Jeder gestaffelte Satz bleibt in der Spanne aus Kap. 5.5.**
    ///
    /// Die Invariante der Matrix gilt auch für die Staffelung; sonst
    /// hätte die Spanne des Papiers keine Wirkung mehr.
    #[test]
    fn jeder_gestaffelte_satz_bleibt_in_der_spanne() {
        for s in matrix() {
            for n in 0..50u64 {
                let g = satz_gestaffelt(s.akteur, s.grund, n).unwrap();
                assert!(
                    g.in_der_spanne(),
                    "{:?}/{:?} bei {n} Vorverstößen: {} bps außerhalb {}..{}",
                    s.akteur,
                    s.grund,
                    g.anteil_bps(),
                    g.spanne_min_bps,
                    g.spanne_max_bps
                );
            }
        }
    }

    /// **Die Staffelung steigt monoton und nie über 100 %.**
    #[test]
    fn die_staffelung_steigt_monoton() {
        for s in matrix() {
            let mut vorher = 0u64;
            for n in 0..10u64 {
                let bps = satz_gestaffelt(s.akteur, s.grund, n).unwrap().anteil_bps();
                assert!(bps >= vorher, "{:?}: {bps} < {vorher}", s.akteur);
                assert!(bps <= 10_000, "mehr als der volle Einsatz gibt es nicht");
                vorher = bps;
            }
        }
    }

    /// Zeilen ohne Spanne bleiben unverändert: Bei 100 % Stake und bei
    /// der Kaution gibt es nichts zu steigern.
    #[test]
    fn zeilen_ohne_spanne_werden_nicht_gestaffelt() {
        for (a, g) in [
            (Akteur::ShardMiner, Grund::FalschesErgebnis),
            (Akteur::PodKoordinator, Grund::FalscheAggregation),
            (Akteur::Checker, Grund::MutwilligeAnfechtung),
        ] {
            for n in 0..5u64 {
                assert_eq!(satz_gestaffelt(a, g, n).unwrap(), satz(a, g).unwrap());
            }
        }
    }

    /// Das Wiederholungsfenster ist so lang wie die Arbeitshistorie des
    /// Stimmgewichts. Zwei verschiedene Antworten auf dieselbe Frage
    /// wären schwer zu begründen.
    #[test]
    fn das_fenster_passt_zur_arbeitshistorie() {
        assert_eq!(WIEDERHOLUNGSFENSTER, 10);
    }
}
