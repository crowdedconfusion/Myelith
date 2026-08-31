//! Der Epochenabschluss: von der Bemessungsgrundlage zur Gutschrift.
//!
//! # ⚑ Punkt 38: die Prägung erreichte kein Konto
//!
//! Bis zum 2026-08-31 war die Kette in der Mitte durchtrennt. Der Ledger
//! zählte den Burn, [`crate::ema::epochenabschluss_burn`] faltete ihn in
//! den geglätteten Wert, [`crate::mint_amount`] rechnete daraus eine
//! Prägung, [`crate::distribute_mint`] teilte sie auf fünf Klassen auf,
//! und dort endete der Weg: **Es gab keinen Übergang, der ein Konto
//! erhöht.** Die Wirtschaft rechnete, und niemand wurde bezahlt.
//!
//! Diese Datei schließt die Lücke. Sie ist der einzige Aufrufer von
//! [`myl_ledger::transitions::praegen`].
//!
//! # Was tatsächlich entsteht
//!
//! ⚑ **Es wird nichts geprägt, was nicht gutgeschrieben wird.** Drei der
//! fünf Empfängerklassen haben heute keine Gewichtsquelle:
//! Koordinatoren, Validatoren und Prüfer. Ihre Anteile ließen sich
//! bequem ins Treasury schieben, damit die Summe aufgeht. Das wäre
//! bequem und falsch: Die Geldmenge wüchse um Beträge, die niemand
//! verdient hat, und das Treasury bekäme still ein Vielfaches der drei
//! Prozent, die Kap. 5.3 ihm zuspricht.
//!
//! Stattdessen wird der Anteil **gar nicht geprägt** und im Ergebnis
//! benannt ([`Ausschuettung::nicht_gepraegt`]). Die Geldmenge wächst nur
//! um das, was ankommt. Das ist die sichere Richtung des Fehlers: Zu
//! wenig zu prägen lässt sich später nachholen, zu viel nicht.
//!
//! # Ohne Auszahlungskonto kein Anteil
//!
//! Festlegung des Projektinhabers vom 2026-08-31. Wer kein
//! Auszahlungskonto eingetragen hat, wird übergangen, **und sein Gewicht
//! zählt nicht**: Die Übrigen teilen den vollen Anteil ihrer Klasse.
//! Damit sammelt sich nie ein Ertrag unter einem heißen Schlüssel an,
//! und der Fehler fällt sofort auf, weil nichts ankommt. Die
//! Übergangenen stehen namentlich im Ergebnis, damit niemand raten muss.
//!
//! # Prüfen, dann ändern
//!
//! Der ganze Plan entsteht, bevor eine Zeile Zustand sich ändert. Sonst
//! bliebe bei einem Überlauf im letzten Konto eine halb abgeschlossene
//! Epoche zurück: der Burn-Zähler zurückgesetzt, ein Teil der Konten
//! erhöht, kein Weg zurück.

use std::collections::BTreeMap;

use myl_ledger::state::LedgerState;
use myl_ledger::transitions::{auszahlungskonto, praegen, TransitionError};
use myl_types::ids::{Address, EpochId, MinerId};
use myl_types::treasury::treasury_adresse;

use crate::distribute::{distribute_mint, split_proportional, Distribution};
use crate::ema::{ema_update, epochenabschluss_burn, Abschlussfehler};
use crate::mint::{mint_amount, MintParams};
use crate::zuschreibung::Zuschreibung;

/// Die fünf Empfängerklassen aus Kap. 5.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Empfaengerklasse {
    /// Wer an einem Shard gerechnet hat.
    ShardMiner,
    /// Wer einen Pod koordiniert hat.
    Koordinatoren,
    /// Wer Blöcke vorgeschlagen und bestätigt hat.
    Validatoren,
    /// Wer Kontrollsegmente nachgerechnet hat.
    Pruefer,
    /// Die Allgemeinheit.
    Treasury,
}

impl std::fmt::Display for Empfaengerklasse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::ShardMiner => "Shard-Miner",
            Self::Koordinatoren => "Koordinatoren",
            Self::Validatoren => "Validatoren",
            Self::Pruefer => "Pruefer",
            Self::Treasury => "Treasury",
        };
        f.write_str(name)
    }
}

/// Warum ein Anteil nicht geprägt wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Auslassungsgrund {
    /// Für diese Klasse gibt es noch keine Gewichtsquelle.
    ///
    /// Kein Versehen, sondern der Stand der Verdrahtung: Wer koordiniert,
    /// validiert oder prüft, wird bisher nirgends epochenweise gezählt.
    KeineGewichtsquelle,
    /// Es gab Arbeit, aber niemanden mit Auszahlungskonto.
    NiemandMitKonto,
}

impl std::fmt::Display for Auslassungsgrund {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::KeineGewichtsquelle => "keine Gewichtsquelle",
            Self::NiemandMitKonto => "niemand mit Auszahlungskonto",
        };
        f.write_str(text)
    }
}

/// Ein Anteil, der nicht geprägt wurde, mit Betrag und Auslassungsgrund.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ausgelassen {
    /// Wessen Anteil.
    pub klasse: Empfaengerklasse,
    /// Wie viel er betragen hätte.
    pub betrag: u64,
    /// Warum er ausblieb.
    pub grund: Auslassungsgrund,
}

/// Was ein Epochenabschluss bewirkt hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ausschuettung {
    /// Die Epoche, für die abgerechnet wurde.
    pub epoche: EpochId,
    /// Der geglättete Burn nach dem Abschluss.
    pub burn_ema: u64,
    /// `M_e` nach der Formel, also was die Klassen zusammen bekämen.
    pub berechnet: u64,
    /// Was wirklich entstanden ist.
    ///
    /// **Kleiner oder gleich [`Self::berechnet`]**, und die Differenz
    /// steht vollständig in [`Self::nicht_gepraegt`].
    pub gutgeschrieben: u64,
    /// Wer wie viel bekommen hat.
    pub je_konto: BTreeMap<Address, u64>,
    /// Wer gerechnet hat und kein Auszahlungskonto eingetragen hatte.
    pub ohne_auszahlungskonto: Vec<MinerId>,
    /// Was nicht geprägt wurde, je Klasse mit Auslassungsgrund.
    pub nicht_gepraegt: Vec<Ausgelassen>,
}

impl Ausschuettung {
    /// Summe dessen, was ausblieb.
    pub fn ausgelassen_summe(&self) -> u128 {
        self.nicht_gepraegt.iter().map(|a| a.betrag as u128).sum()
    }
}

/// Was den Abschluss scheitern lässt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ausschuettungsfehler {
    /// Diese Epoche wurde schon abgeschlossen.
    Abschluss(Abschlussfehler),
    /// Eine Gutschrift ginge über den Zahlenbereich hinaus.
    Buchung(TransitionError),
}

impl std::fmt::Display for Ausschuettungsfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Abschluss(e) => write!(f, "Epochenabschluss: {}", e),
            Self::Buchung(e) => write!(f, "Buchung: {}", e),
        }
    }
}

impl std::error::Error for Ausschuettungsfehler {}

/// Schließt eine Epoche ab und schreibt die Prägung den Konten gut.
///
/// Ruft [`epochenabschluss_burn`] mit auf; ein zweiter Aufruf in
/// derselben Epoche scheitert daran und ändert nichts.
///
/// # Ablauf
///
/// 1. Den geglätteten Burn fortschreiben, daraus `M_e` rechnen.
/// 2. `M_e` auf die fünf Klassen aufteilen (Kap. 5.3).
/// 3. Für die Shard-Miner die Gewichte auf Auszahlungskonten abbilden,
///    Miner ohne Konto fallen heraus **samt ihrem Gewicht**.
/// 4. Den Anteil proportional aufteilen, das Treasury bekommt seinen
///    unmittelbar.
/// 5. Alles gutschreiben, was einen Empfänger hat; den Rest benennen.
pub fn epochenausschuettung(
    state: &mut LedgerState,
    zuschreibung: &Zuschreibung,
    params: &MintParams,
) -> Result<Ausschuettung, Ausschuettungsfehler> {
    // ---- Prüfphase: der ganze Plan, ohne eine Zustandsänderung. ----

    // Was der Abschluss ergäbe. Deterministisch dieselbe Rechnung, die
    // `epochenabschluss_burn` unten ausführt.
    if state.burn_ema_bis >= state.epoch && state.epoch.0 > 0 {
        return Err(Ausschuettungsfehler::Abschluss(
            Abschlussfehler::SchonFortgeschrieben {
                bis: state.burn_ema_bis,
            },
        ));
    }
    let ema = ema_update(state.burn_ema, state.burn_epoche);
    let m_e = mint_amount(ema, params);
    let Distribution {
        shard_miners,
        coordinators,
        validators,
        checkers,
        treasury,
    } = distribute_mint(m_e);

    let mut ohne_konto: Vec<MinerId> = Vec::new();
    let mut gewichte: Vec<(Address, u64)> = Vec::new();
    for (miner, gewicht) in &zuschreibung.je_miner {
        match auszahlungskonto(state, miner) {
            Some(konto) => gewichte.push((konto, *gewicht)),
            // Ohne Eintrag kein Anteil, und das Gewicht zählt nicht.
            None => ohne_konto.push(*miner),
        }
    }

    let mut plan: BTreeMap<Address, u64> = BTreeMap::new();
    let mut nicht_gepraegt: Vec<Ausgelassen> = Vec::new();

    match split_proportional(shard_miners, &gewichte) {
        Ok(anteile) => {
            for (konto, betrag) in anteile {
                let e = plan.entry(konto).or_insert(0);
                *e = e.saturating_add(betrag);
            }
        }
        // Positiver Betrag, aber kein Empfänger mit Gewicht.
        Err(_) => {
            nicht_gepraegt.push(Ausgelassen {
                klasse: Empfaengerklasse::ShardMiner,
                betrag: shard_miners,
                grund: Auslassungsgrund::NiemandMitKonto,
            });
        }
    }

    for (klasse, betrag) in [
        (Empfaengerklasse::Koordinatoren, coordinators),
        (Empfaengerklasse::Validatoren, validators),
        (Empfaengerklasse::Pruefer, checkers),
    ] {
        if betrag > 0 {
            nicht_gepraegt.push(Ausgelassen {
                klasse,
                betrag,
                grund: Auslassungsgrund::KeineGewichtsquelle,
            });
        }
    }

    if treasury > 0 {
        let e = plan.entry(treasury_adresse()).or_insert(0);
        *e = e.saturating_add(treasury);
    }

    // ⚑ Nullbeträge fallen heraus, **bevor** der Plan gilt. Bei kleinen
    // Prägungen bekommen Empfänger mit positivem Gewicht rechnerisch
    // null, und `split_proportional` führt sie dann mit null im
    // Ergebnis. Blieben sie stehen, behauptete `je_konto` eine
    // Gutschrift, die nie erfolgte: `praegen` lehnt null ab, und die
    // Aufstellung wäre eine andere als die Buchung.
    plan.retain(|_, betrag| *betrag > 0);

    // Überlauf vorher prüfen, nicht mitten im Buchen.
    for (konto, betrag) in &plan {
        state
            .account(konto)
            .balance
            .checked_add(*betrag)
            .ok_or(Ausschuettungsfehler::Buchung(TransitionError::Overflow))?;
    }

    // ---- Änderungsphase. ----

    let burn_ema =
        epochenabschluss_burn(state).map_err(Ausschuettungsfehler::Abschluss)?;
    debug_assert_eq!(burn_ema, ema, "die Prüfphase rechnete etwas anderes");

    let mut gutgeschrieben: u64 = 0;
    for (konto, betrag) in &plan {
        praegen(state, konto, *betrag).map_err(Ausschuettungsfehler::Buchung)?;
        gutgeschrieben = gutgeschrieben.saturating_add(*betrag);
    }

    Ok(Ausschuettung {
        epoche: state.epoch,
        burn_ema,
        berechnet: m_e,
        gutgeschrieben,
        je_konto: plan,
        ohne_auszahlungskonto: ohne_konto,
        nicht_gepraegt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_ledger::transitions::{auszahlungskonto_eintragen, burn_to_credits};
    use myl_types::ids::EpochId;

    fn params() -> MintParams {
        MintParams {
            subsidy_num: 0,
            subsidy_den: 1,
            m_max: u64::MAX,
        }
    }

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    /// Die Adresse, die aus derselben Kennung folgt. Kennung und Adresse
    /// sind verschiedene Typen über denselben Bytes.
    fn eigen(b: u8) -> Address {
        Address::new([b; 32])
    }

    fn kaltes_konto(b: u8) -> Address {
        Address::new([200 + b; 32])
    }

    /// Ein Zustand mit Burn in der Epoche und `n` Minern, die je ein
    /// kaltes Auszahlungskonto eingetragen haben.
    fn aufbau(verbrannt: u64, mit_konto: &[u8], ohne_konto: &[u8]) -> (LedgerState, Zuschreibung) {
        let mut st = LedgerState::genesis(1);
        let quelle = Address::new([9; 32]);
        st.account_mut(&quelle).balance = verbrannt;
        if verbrannt > 0 {
            burn_to_credits(&mut st, &quelle, verbrannt, EpochId(0)).expect("Burn");
        }
        for b in mit_konto {
            auszahlungskonto_eintragen(&mut st, &eigen(*b), &miner(*b), kaltes_konto(*b))
                .expect("Eintragung");
        }
        st.epoch = EpochId(1);

        let mut je_miner = BTreeMap::new();
        for b in mit_konto.iter().chain(ohne_konto.iter()) {
            je_miner.insert(miner(*b), 1_000u64);
        }
        (
            st,
            Zuschreibung {
                je_miner,
                reserve_ohne_anteil: vec![],
            },
        )
    }

    fn summe_der_guthaben(st: &LedgerState) -> u128 {
        st.accounts.values().map(|a| a.balance as u128).sum()
    }

    /// ⚑ **Punkt 38, die Kernaussage:** Am Ende einer Epoche wächst ein
    /// Konto. Bis zum 2026-08-31 tat es das nicht.
    #[test]
    fn die_praegung_erreicht_ein_konto() {
        let (mut st, z) = aufbau(1_000_000, &[1, 2], &[]);
        let vorher = st.account(&kaltes_konto(1)).balance;
        let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
        assert!(a.berechnet > 0, "es wurde nichts gepraegt");
        assert!(
            st.account(&kaltes_konto(1)).balance > vorher,
            "das Auszahlungskonto ist leer geblieben"
        );
        assert!(a.gutgeschrieben > 0);
    }

    /// Die Invariante: Was geprägt wurde plus was ausblieb ergibt, was
    /// die Formel ausgerechnet hat. Kein Betrag verschwindet unbenannt.
    #[test]
    fn gepraegt_plus_ausgelassen_ergibt_die_rechnung() {
        for verbrannt in [0u64, 1, 7, 1_000, 999_983, 12_345_678] {
            let (mut st, z) = aufbau(verbrannt, &[1, 2, 3], &[]);
            let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
            assert_eq!(
                a.gutgeschrieben as u128 + a.ausgelassen_summe(),
                a.berechnet as u128,
                "Burn {verbrannt}: die Summe geht nicht auf"
            );
        }
    }

    /// Die Geldmenge wächst um genau das, was gutgeschrieben wurde.
    #[test]
    fn die_geldmenge_waechst_um_das_gutgeschriebene() {
        let (mut st, z) = aufbau(5_000_000, &[1, 2, 3], &[]);
        let vorher = summe_der_guthaben(&st);
        let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
        assert_eq!(summe_der_guthaben(&st) - vorher, a.gutgeschrieben as u128);
    }

    /// ⚑ **Ohne Eintrag kein Anteil**, und der Übergangene wird genannt.
    #[test]
    fn ohne_auszahlungskonto_kein_anteil() {
        let (mut st, z) = aufbau(1_000_000, &[1, 2], &[3]);
        let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
        assert_eq!(a.ohne_auszahlungskonto, vec![miner(3)]);
        assert_eq!(
            st.account(&eigen(3)).balance,
            0,
            "der Miner ohne Konto wurde bezahlt"
        );
    }

    /// ⚑ **Sein Gewicht zählt nicht:** Die Übrigen teilen den vollen
    /// Anteil ihrer Klasse, nicht zwei Drittel davon.
    #[test]
    fn das_gewicht_des_uebergangenen_zaehlt_nicht() {
        let (mut st_a, z_a) = aufbau(1_000_000, &[1, 2], &[]);
        let a = epochenausschuettung(&mut st_a, &z_a, &params()).expect("ohne Dritten");
        let (mut st_b, z_b) = aufbau(1_000_000, &[1, 2], &[3]);
        let b = epochenausschuettung(&mut st_b, &z_b, &params()).expect("mit Drittem");
        assert_eq!(
            st_a.account(&kaltes_konto(1)).balance,
            st_b.account(&kaltes_konto(1)).balance,
            "ein Miner ohne Konto hat den Anteil der anderen verkleinert"
        );
        assert_eq!(a.gutgeschrieben, b.gutgeschrieben);
    }

    /// Das Treasury bekommt seinen Anteil, und zwar auf die
    /// schlüssellose Adresse.
    #[test]
    fn das_treasury_bekommt_seinen_anteil() {
        let (mut st, z) = aufbau(10_000_000, &[1], &[]);
        let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
        let auf_treasury = st.account(&treasury_adresse()).balance;
        assert!(auf_treasury > 0, "das Treasury ging leer aus");
        assert_eq!(a.je_konto[&treasury_adresse()], auf_treasury);
    }

    /// ⚑ Drei Klassen haben keine Gewichtsquelle. Ihr Anteil wird **nicht
    /// geprägt** und namentlich benannt, statt still im Treasury zu
    /// landen.
    #[test]
    fn drei_klassen_werden_benannt_statt_umgeleitet() {
        let (mut st, z) = aufbau(10_000_000, &[1], &[]);
        let vor_treasury = st.account(&treasury_adresse()).balance;
        let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
        let klassen: Vec<_> = a.nicht_gepraegt.iter().map(|x| x.klasse).collect();
        assert_eq!(
            klassen,
            vec![
                Empfaengerklasse::Koordinatoren,
                Empfaengerklasse::Validatoren,
                Empfaengerklasse::Pruefer
            ]
        );
        assert!(a
            .nicht_gepraegt
            .iter()
            .all(|x| x.grund == Auslassungsgrund::KeineGewichtsquelle));
        let zuwachs = st.account(&treasury_adresse()).balance - vor_treasury;
        assert!(
            (zuwachs as u128) < a.ausgelassen_summe(),
            "die ausgelassenen Anteile sind im Treasury gelandet"
        );
    }

    /// Hat niemand ein Konto, wird der Shard-Anteil nicht geprägt und der
    /// Auslassungsgrund steht dabei.
    #[test]
    fn ohne_einen_einzigen_empfaenger_wird_nicht_gepraegt() {
        let (mut st, z) = aufbau(10_000_000, &[], &[1, 2]);
        let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
        let shard = a
            .nicht_gepraegt
            .iter()
            .find(|x| x.klasse == Empfaengerklasse::ShardMiner)
            .expect("der Shard-Anteil fehlt in der Aufstellung");
        assert_eq!(shard.grund, Auslassungsgrund::NiemandMitKonto);
        assert!(shard.betrag > 0);
        assert_eq!(a.ohne_auszahlungskonto, vec![miner(1), miner(2)]);
    }

    /// Gegenprobe: zweimal in derselben Epoche geht nicht, und der
    /// zweite Versuch lässt den Zustand unberührt.
    #[test]
    fn zweimal_in_derselben_epoche_aendert_nichts() {
        let (mut st, z) = aufbau(1_000_000, &[1, 2], &[]);
        epochenausschuettung(&mut st, &z, &params()).expect("erster Abschluss");
        let nach_dem_ersten = st.commitment();
        let zweiter = epochenausschuettung(&mut st, &z, &params());
        assert!(
            matches!(zweiter, Err(Ausschuettungsfehler::Abschluss(_))),
            "der zweite Abschluss lief durch: {zweiter:?}"
        );
        assert_eq!(
            st.commitment(),
            nach_dem_ersten,
            "der gescheiterte Abschluss hat den Zustand veraendert"
        );
    }

    /// ⚑ Gegenprobe: Läuft eine Gutschrift über, bleibt **nichts**
    /// zurück: kein zurückgesetzter Burn-Zähler und kein halb erhöhtes
    /// Konto.
    #[test]
    fn ein_ueberlauf_laesst_den_zustand_unberuehrt() {
        let (mut st, z) = aufbau(10_000_000, &[1], &[]);
        st.account_mut(&kaltes_konto(1)).balance = u64::MAX;
        let vorher = st.commitment();
        let ergebnis = epochenausschuettung(&mut st, &z, &params());
        assert_eq!(
            ergebnis,
            Err(Ausschuettungsfehler::Buchung(TransitionError::Overflow))
        );
        assert_eq!(
            st.commitment(),
            vorher,
            "nach dem Ueberlauf ist der Zustand veraendert"
        );
    }

    /// Ohne Burn gibt es nichts zu prägen, und das ist kein Fehler.
    #[test]
    fn ohne_burn_wird_nichts_gepraegt() {
        let (mut st, z) = aufbau(0, &[1, 2], &[]);
        let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
        assert_eq!(a.berechnet, 0);
        assert_eq!(a.gutgeschrieben, 0);
        assert!(a.nicht_gepraegt.is_empty(), "null gehoert nicht aufgelistet");
        assert_eq!(summe_der_guthaben(&st), 0);
    }

    /// ⚑ Was in `je_konto` steht, wurde auch gebucht. Bei einer winzigen
    /// Prägung bekommen Empfänger mit positivem Gewicht rechnerisch
    /// null; sie gehören nicht in eine Aufstellung der Gutschriften.
    #[test]
    fn je_konto_nennt_nur_was_gebucht_wurde() {
        // So klein, dass nicht für jeden eine Einheit übrig bleibt: Der
        // Burn von 80 ergibt einen geglätteten Wert von 5, davon gehen
        // 78 % an die Shard-Miner, also 3 Einheiten für fünf Empfänger.
        let (mut st, z) = aufbau(80, &[1, 2, 3, 4, 5], &[]);
        let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
        // ⚑ Die Lage muss wirklich eintreten, sonst prüft der Test nichts.
        assert!(
            a.gutgeschrieben > 0 && a.je_konto.len() < 5,
            "der Fall trat nicht ein: {} gutgeschrieben auf {} Konten",
            a.gutgeschrieben,
            a.je_konto.len()
        );
        assert!(
            a.je_konto.values().all(|b| *b > 0),
            "eine Nullgutschrift steht in der Aufstellung: {:?}",
            a.je_konto
        );
        let gebucht: u64 = a
            .je_konto
            .keys()
            .map(|k| st.account(k).balance)
            .sum();
        assert_eq!(gebucht, a.gutgeschrieben);
    }

    /// Der Epochenabschluss läuft mit: Der Zähler steht danach auf null.
    #[test]
    fn der_burn_zaehler_wird_zurueckgesetzt() {
        let (mut st, z) = aufbau(1_000_000, &[1], &[]);
        assert!(st.burn_epoche > 0);
        let a = epochenausschuettung(&mut st, &z, &params()).expect("Ausschuettung");
        assert_eq!(st.burn_epoche, 0);
        assert_eq!(st.burn_ema, a.burn_ema);
        assert_eq!(st.burn_ema_bis, EpochId(1));
    }
}
