//! Der Einsatz: was hinterlegt wird, wie es gekündigt wird und wie
//! lange es haftet (Punkt B11, 2026-09-03).
//!
//! # ⚑ Warum es das bis zum 2026-09-02 nicht gab (Fund 145)
//!
//! `AccountState::staked` steht seit Langem im Zustand, und der ganze
//! wirtschaftliche Sicherheitsbau hängt daran: `S_min = g/p²`, das
//! Stimmgewicht, die Slashing-Staffelung, das Kopfgeld. **Nur schrieb
//! es niemand.** Keine der acht Anweisungen setzte einen Einsatz, also
//! war `staked` im Betrieb immer null, also schlachtete `apply_verdict`
//! immer null, also hatte `MindestStake` nichts zu begrenzen.
//!
//! **Die siebte Ausprägung des häufigsten Fehlerbilds dieses Projekts,
//! und die grösste:** nicht eine fehlende Naht, sondern eine ganze
//! Schicht ohne Anschluss.
//!
//! # ⚑ Warum das hier steht und nicht in TOKENOMICS
//!
//! Der erste Anlauf legte es dorthin, wo die übrigen wirtschaftlichen
//! Grössen stehen. **Das ging nicht, und der Compiler hatte recht:**
//! `myl-tokenomics` hängt an `myl-ledger`, nicht umgekehrt.
//!
//! **Und die Einordnung war ohnehin falsch.** Was hier steht, ist keine
//! Formel, sondern **Zustandsmechanik**: hinterlegen, kündigen, reifen,
//! haften. TOKENOMICS hält Formeln wie `S_min` und die Prägung; die
//! Warteschlange einer Kündigung ist eine Regel des Ledgers.
//!
//! # ⚑ Die Sperrfrist ist hergeleitet, nicht gewählt
//!
//! Ein Einsatz, den man sofort abziehen kann, ist keiner. Wer falsch
//! rechnet, zieht ab, bevor das Urteil da ist, und die Schlachtung
//! findet ein leeres Konto.
//!
//! **Also muss die Frist mindestens so lang sein wie das Fenster, in
//! dem noch ein Urteil kommen kann**, und das ist die Streitfrist:
//! [`myl_consensus::epoch_close::DEFAULT_DISPUTE_EPOCHS`], sieben Tage
//! bei Stunden-Epochen. Kürzer wäre ein Fluchtweg, länger wäre eine
//! Härte ohne Begründung.
//!
//! ⚑ **Und das genügt allein noch nicht.** Wer kündigt, hätte seinen
//! Einsatz sonst schon aus `staked` heraus und damit aus der
//! Schlachtmasse, obwohl er noch haftet. Deshalb zählt
//! [`schlachtbar`] den gekündigten Teil **mit**, solange er nicht
//! abgeholt ist. Die Frist verschiebt die Auszahlung; sie beendet die
//! Haftung nicht.
//!
//! # Warum das Abholen eine eigene Anweisung ist
//!
//! Freigewordenen Einsatz beim Epochenwechsel automatisch
//! zurückzubuchen hiesse, **jedes Konto in jeder Epoche anzufassen**.
//! Das ist eine Arbeit in der Grösse des Netzes für einen Vorgang, der
//! einzelne betrifft. Wer sein Geld will, holt es.

use std::collections::BTreeMap;

/// Wie viele Epochen ein gekündigter Einsatz gesperrt bleibt.
///
/// ⚑ **Gleich der Streitfrist, und das ist die Herleitung.** Ein
/// Urteil kann bis zum Ende der Streitfrist kommen; wer vorher abziehen
/// könnte, entzöge sich der Schlachtung. Siehe den Modulkopf.
///
/// ⚑ **Die Zahl steht hier und die Bindung woanders**, und das hat
/// einen Grund: Die Streitfrist ist ein Governance-Parameter, und
/// `DEFAULT_DISPUTE_EPOCHS` steht in `myl-consensus`. Diese Kiste kennt
/// keine von beiden, denn beide hängen an ihr und nicht sie an ihnen.
/// **Die Gleichheit prüft GOVERNANCE**, wo alle drei sichtbar sind, mit
/// derselben Arbeitsteilung wie bei `S_min`.
///
/// Sieben Tage bei Stunden-Epochen sind 168.
pub const SPERRFRIST_EPOCHEN: u64 = 168;

/// Wie viele getrennte Kündigungen ein Konto offen haben darf.
///
/// ⚑ **Eine Schranke gegen Zustandswachstum**, dieselbe Klasse wie bei
/// den Bündeln (Fund 144): Ohne sie könnte ein Konto beliebig viele
/// Kleinstkündigungen einlegen, und jede stünde bis zur Freigabe in
/// jeder Zustandswurzel.
///
/// **Sie kann nicht binden, wer sich normal verhält.** Kündigungen
/// derselben Epoche werden zusammengelegt, es gibt also höchstens einen
/// Eintrag je Freigabe-Epoche, und mehr als [`SPERRFRIST_EPOCHEN`]
/// verschiedene Freigabe-Epochen kann niemand offen haben. Die Schranke
/// steht trotzdem: Eine Grenze, die aus einer anderen folgt, gehört
/// hingeschrieben, sonst verschwindet sie mit ihrer Herleitung.
pub const MAX_OFFENE_KUENDIGUNGEN: usize = SPERRFRIST_EPOCHEN as usize + 1;

/// Was beim Umgang mit dem Einsatz schiefgehen kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EinsatzFehler {
    /// Betrag null: ein Vorgang ohne Wirkung, der Blockplatz kostet.
    Nullbetrag,
    /// Das Guthaben deckt den Betrag nicht.
    GuthabenReichtNicht { verfuegbar: u64, verlangt: u64 },
    /// Der hinterlegte Einsatz deckt den Betrag nicht.
    EinsatzReichtNicht { verfuegbar: u64, verlangt: u64 },
    /// Zu viele offene Kündigungen (siehe [`MAX_OFFENE_KUENDIGUNGEN`]).
    ZuVieleKuendigungen { offen: usize },
    /// Nichts ist fällig.
    NichtsFaellig,
}

impl std::fmt::Display for EinsatzFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nullbetrag => f.write_str("ein Einsatz von null bewirkt nichts"),
            Self::GuthabenReichtNicht { verfuegbar, verlangt } => write!(
                f,
                "Guthaben {verfuegbar} deckt den Einsatz {verlangt} nicht"
            ),
            Self::EinsatzReichtNicht { verfuegbar, verlangt } => write!(
                f,
                "hinterlegter Einsatz {verfuegbar} deckt die Kuendigung {verlangt} nicht"
            ),
            Self::ZuVieleKuendigungen { offen } => write!(
                f,
                "{offen} offene Kuendigungen, hoechstens {MAX_OFFENE_KUENDIGUNGEN} sind erlaubt"
            ),
            Self::NichtsFaellig => f.write_str("keine Kuendigung ist faellig"),
        }
    }
}

impl std::error::Error for EinsatzFehler {}

/// ⚑ **Eine Frist von null wäre gar keine, und das gilt schon beim
/// Übersetzen.**
///
/// Eine Zusicherung über eine Konstante gehört nicht in einen Test: Ein
/// Test läuft, wenn jemand ihn startet, **eine `const`-Zusicherung
/// hält, sonst gibt es kein Programm.** Wer die Frist auf null setzt,
/// bekommt keinen fehlschlagenden Test, sondern gar keinen Bau.
const _: () = assert!(
    SPERRFRIST_EPOCHEN > 0,
    "ohne Sperrfrist ist der Einsatz kein Einsatz"
);

/// Die Epoche, zu der eine in `jetzt` ausgesprochene Kündigung frei
/// wird.
pub fn freigabe_epoche(jetzt: u64) -> u64 {
    jetzt.saturating_add(SPERRFRIST_EPOCHEN)
}

/// Was insgesamt haftet: hinterlegter **und** gekündigter Einsatz.
///
/// ⚑ **Die Zahl, gegen die geschlachtet wird.** Wer kündigt, hat den
/// Betrag aus `staked` heraus; er haftet aber weiter, bis er abgeholt
/// ist. Zählte man ihn nicht mit, wäre die Kündigung der Fluchtweg, den
/// die Sperrfrist gerade schliessen soll.
pub fn schlachtbar(hinterlegt: u64, gekuendigt: &BTreeMap<u64, u64>) -> u64 {
    gekuendigt
        .values()
        .fold(hinterlegt, |summe, betrag| summe.saturating_add(*betrag))
}

/// Was in `jetzt` abgeholt werden kann.
pub fn faellig(gekuendigt: &BTreeMap<u64, u64>, jetzt: u64) -> u64 {
    gekuendigt
        .range(..=jetzt)
        .fold(0u64, |summe, (_, betrag)| summe.saturating_add(*betrag))
}

/// Nimmt `betrag` aus der Schlachtmasse: erst aus dem hinterlegten
/// Einsatz, dann aus den Kündigungen.
///
/// ⚑ **Die Kündigungen in der Reihenfolge ihrer Freigabe**, also die
/// zuerst, die am nächsten an der Auszahlung ist. Das ist die Richtung,
/// die einem Fliehenden zuerst nimmt, was er zu retten versucht.
///
/// Gibt zurück, was tatsächlich genommen wurde; das kann weniger sein
/// als `betrag`, wenn die Masse nicht reicht. **Weniger ist die
/// ehrliche Antwort**, denn ein Konto kann nicht mehr verlieren, als es
/// hat.
pub fn nimm_aus_der_masse(
    hinterlegt: &mut u64,
    gekuendigt: &mut BTreeMap<u64, u64>,
    betrag: u64,
) -> u64 {
    let mut offen = betrag;
    let aus_hinterlegt = (*hinterlegt).min(offen);
    *hinterlegt -= aus_hinterlegt;
    offen -= aus_hinterlegt;

    if offen > 0 {
        let epochen: Vec<u64> = gekuendigt.keys().copied().collect();
        for e in epochen {
            if offen == 0 {
                break;
            }
            let hier = gekuendigt.get(&e).copied().unwrap_or(0);
            let nehmen = hier.min(offen);
            offen -= nehmen;
            if hier == nehmen {
                gekuendigt.remove(&e);
            } else {
                gekuendigt.insert(e, hier - nehmen);
            }
        }
    }
    betrag - offen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kuendigungen(paare: &[(u64, u64)]) -> BTreeMap<u64, u64> {
        paare.iter().copied().collect()
    }

    /// ⚑ **Die Sperrfrist ist die Streitfrist**, und das ist keine
    /// Übereinstimmung, sondern die Herleitung.
    #[test]
    fn die_sperrfrist_deckt_das_streitfenster() {
        // Sieben Tage bei Stunden-Epochen. **Die Bindung an die
        // Streitfrist prueft GOVERNANCE**, wo beide Zahlen sichtbar
        // sind; hier steht, was daraus folgt.
        assert_eq!(SPERRFRIST_EPOCHEN, 7 * 24);
        assert_eq!(freigabe_epoche(10), 10 + SPERRFRIST_EPOCHEN);
    }

    /// ⚑ **Gekündigtes haftet weiter.** Ohne das wäre die Kündigung der
    /// Fluchtweg, den die Frist schliessen soll.
    #[test]
    fn gekuendigtes_zaehlt_zur_schlachtmasse() {
        let g = kuendigungen(&[(200, 300), (300, 400)]);
        assert_eq!(schlachtbar(1_000, &g), 1_700);
        assert_eq!(schlachtbar(0, &g), 700, "auch ohne hinterlegten Rest haftet es");
    }

    /// Fällig ist, was seine Freigabe-Epoche erreicht hat, und sonst
    /// nichts.
    #[test]
    fn faellig_ist_nur_was_frei_ist() {
        let g = kuendigungen(&[(100, 5), (200, 7), (300, 9)]);
        assert_eq!(faellig(&g, 99), 0);
        assert_eq!(faellig(&g, 100), 5);
        assert_eq!(faellig(&g, 250), 12);
        assert_eq!(faellig(&g, 1_000), 21);
    }

    /// Geschlachtet wird erst der hinterlegte Einsatz.
    #[test]
    fn die_masse_wird_zuerst_dem_hinterlegten_entnommen() {
        let mut h = 500;
        let mut g = kuendigungen(&[(100, 300)]);
        assert_eq!(nimm_aus_der_masse(&mut h, &mut g, 200), 200);
        assert_eq!(h, 300);
        assert_eq!(g[&100], 300, "die Kuendigung wurde angefasst, obwohl Einsatz da war");
    }

    /// ⚑ **Und danach die Kündigungen, die früheste zuerst.**
    #[test]
    fn danach_die_naechstliegende_kuendigung() {
        let mut h = 100;
        let mut g = kuendigungen(&[(100, 300), (200, 400)]);
        // 100 aus dem Einsatz, 300 aus der ersten, 100 aus der zweiten.
        assert_eq!(nimm_aus_der_masse(&mut h, &mut g, 500), 500);
        assert_eq!(h, 0);
        assert!(!g.contains_key(&100), "die erste Kuendigung ist nicht leer geworden");
        assert_eq!(g[&200], 300);
    }

    /// Mehr als da ist, kann niemand verlieren, und die Antwort sagt
    /// das.
    #[test]
    fn mehr_als_da_ist_geht_nicht() {
        let mut h = 10;
        let mut g = kuendigungen(&[(100, 5)]);
        assert_eq!(nimm_aus_der_masse(&mut h, &mut g, 1_000), 15);
        assert_eq!(h, 0);
        assert!(g.is_empty());
    }

    /// ⚑ Die Schranke gegen Zustandswachstum folgt aus der Frist und
    /// steht trotzdem hin.
    #[test]
    fn die_zahl_offener_kuendigungen_folgt_der_frist() {
        assert_eq!(MAX_OFFENE_KUENDIGUNGEN, SPERRFRIST_EPOCHEN as usize + 1);
    }
}
