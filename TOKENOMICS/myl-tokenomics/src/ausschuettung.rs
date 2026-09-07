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
    epochenausschuettung_mit_training(state, zuschreibung, &Zuschreibung::default(), 0, params)
}

/// Dasselbe, aber mit einer zweiten Zuschreibung für **Training**.
///
/// # ⚑ Gleiche Rate, andere Quelle
///
/// Trainingsarbeit wird je Rechenstunde **genauso** vergütet wie
/// Inferenz. Anhang B.7.3 begründet den Deckel aus Kap. 5.6 damit, dass
/// „Miner zwischen beiden Arbeitsklassen nach der Vergütung je
/// Rechenstunde wählen"; seit die Zuteilung erzwungen ist, wählt
/// niemand, und derselbe Anhang sagt für diesen Fall: „Bei Gleichstand
/// entscheidet allein die Zuteilung."
///
/// ⚑ **Gleichstand ist sogar besser als ein Deckel.** Läge Training
/// darunter, hätte ein zugeteilter Trainingspod einen Anreiz,
/// absichtlich zu scheitern.
///
/// ⚑ **Die Quelle bleibt die Treasury** (Kap. 5.6). Eine Finanzierung
/// aus Zusatzprägung verdoppelte die Netto-Inflation beinahe. Die
/// Treasury hat drei Prozent der Prägung gegen 78 der Shard-Miner, deckt
/// also rund 3,85 Prozent des Inferenzvolumens.
///
/// ⚑ **Was darüber liegt, wird nicht gekürzt, sondern geteilt.** Bei
/// viel Training fällt die Vergütung je Einheit, weil die Treasury nicht
/// mehr hergibt; das ist Arithmetik und keine Politik. Ein leerlaufender
/// Miner, der wenig verdient, steht immer noch besser da als einer, der
/// nichts tut. **Sichtbar wird es über
/// [`crate::vtfe::trainingsdeckung_bps`].**
pub fn epochenausschuettung_mit_training(
    state: &mut LedgerState,
    zuschreibung: &Zuschreibung,
    training: &Zuschreibung,
    auslastung: i64,
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

    // ⚑ **Die Trainingsabgabe**, siehe [`crate::trainingsabgabe`]: Bei
    // hoher Auslastung geht ein Teil der Shard-Miner-Quote in die
    // Treasury, bei niedriger nicht. Das Netz spart, wenn es reich ist,
    // und investiert, wenn Kapazität übrig ist.
    let abgabe_bps =
        crate::trainingsabgabe::abgabe_bps(auslastung, crate::trainingsabgabe::ABGABE_MAX_BPS);
    let abgabe = crate::trainingsabgabe::abgabe_betrag(m_e, abgabe_bps);
    let abgabe = abgabe.min(shard_miners);
    let shard_miners = shard_miners - abgabe;
    let treasury = treasury.saturating_add(abgabe);

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

    // ⚑ **Training zuerst aus der Treasury, der Rest an die Treasury.**
    // Die Reihenfolge ist die Aussage: Der Trainingsanteil ist kein
    // Zuschuss aus der Prägung, sondern ein Teil dessen, was die
    // Treasury ohnehin bekäme.
    let mut treasury_rest = treasury;
    // Was über den Zufluss hinaus aus dem Bestand genommen wird, und
    // an wen es geht.
    let mut aus_bestand: u64 = 0;
    let mut ueberweisung: BTreeMap<Address, u64> = BTreeMap::new();
    let mut training_gewichte: Vec<(Address, u64)> = Vec::new();
    for (miner, gewicht) in &training.je_miner {
        match auszahlungskonto(state, miner) {
            Some(konto) => training_gewichte.push((konto, *gewicht)),
            None => ohne_konto.push(*miner),
        }
    }
    if treasury_rest > 0 && !training_gewichte.is_empty() {
        // ⚑ **Der Anspruch folgt aus der Inferenzrate, nicht aus der
        // Treasury.** Der erste Entwurf teilte die **ganze** Treasury
        // unter den Trainingspods auf; ein einziges kleines Segment
        // hätte damit drei Prozent der Prägung bekommen. Gleiche Rate
        // heisst: dieselbe Vergütung je Gewichtseinheit wie Inferenz.
        let inferenz_gewicht: u64 =
            gewichte.iter().map(|(_, g)| *g).fold(0u64, |a, b| a.saturating_add(b));
        let training_gewicht: u64 =
            training_gewichte.iter().map(|(_, g)| *g).fold(0u64, |a, b| a.saturating_add(b));
        // ⚑ **Ohne Inferenz gibt es keine Rate.** Dann ist der Anspruch
        // das, was da ist: Die Prägung ist gegen den geglätteten Burn
        // erfolgt, die Shard-Miner-Quote bleibt mangels Arbeit liegen,
        // und die Trainingspods sind die einzigen, die gerechnet haben.
        // **Das ist derselbe Fall wie „Anspruch übersteigt Treasury"**,
        // nur von der anderen Seite, und wird deshalb gleich behandelt.
        let anspruch = if inferenz_gewicht > 0 {
            ((shard_miners as u128 * training_gewicht as u128) / inferenz_gewicht as u128)
                .min(u64::MAX as u128) as u64
        } else {
            treasury_rest
        };
        // ⚑ **Der Puffer: die Treasury darf ihr Angespartes ausgeben.**
        // Ohne das wäre die Abgabe nutzlos, denn sie sammelt in guten
        // Epochen genau dafür an. Was in dieser Epoche zufliesst, deckt
        // am Auslastungsziel gerade die Kosten; darunter kommt der Rest
        // aus dem Bestand.
        let bestand = state.account(&treasury_adresse()).balance;
        let verfuegbar = treasury_rest.saturating_add(bestand);
        let zu_zahlen = anspruch.min(verfuegbar);
        if zu_zahlen > 0 {
            // ⚑ **Zwei Töpfe, und sie werden verschieden gebucht.** Was
            // aus dem Zufluss dieser Epoche kommt, wird **geprägt**: Es
            // wäre sonst an die Treasury geprägt worden. Was aus dem
            // **Bestand** kommt, ist schon vorhandenes Geld und wird
            // **überwiesen**.
            //
            // ⚑ **Der Unterschied ist der ganze Punkt von Kap. 5.6.**
            // Wer den Bestand präge, verdoppelte die Netto-Inflation
            // genau so, wie es dort ausgeschlossen wird, und der Puffer
            // wäre keiner: Er nähme nichts weg, er schüfe.
            let aus_zufluss = zu_zahlen.min(treasury_rest);
            aus_bestand = zu_zahlen - aus_zufluss;
            if aus_zufluss > 0 {
                if let Ok(anteile) = split_proportional(aus_zufluss, &training_gewichte) {
                    for (konto, betrag) in anteile {
                        let e = plan.entry(konto).or_insert(0);
                        *e = e.saturating_add(betrag);
                    }
                }
            }
            if aus_bestand > 0 {
                if let Ok(anteile) = split_proportional(aus_bestand, &training_gewichte) {
                    ueberweisung = anteile;
                }
            }
            treasury_rest -= aus_zufluss;
        }
    }
    if treasury_rest > 0 {
        let e = plan.entry(treasury_adresse()).or_insert(0);
        *e = e.saturating_add(treasury_rest);
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

    // ⚑ **Der Griff in den Puffer, als Überweisung und nicht als
    // Prägung.** Er verschiebt vorhandenes Geld von der Treasury zu den
    // Trainingsminern; die Geldmenge bleibt unberührt.
    if aus_bestand > 0 {
        let von = treasury_adresse();
        let haben = state.account(&von).balance;
        debug_assert!(haben >= aus_bestand, "der Bestand wurde vorher geprueft");
        state.account_mut(&von).balance = haben.saturating_sub(aus_bestand);
        for (konto, betrag) in &ueberweisung {
            let e = state.account_mut(konto);
            e.balance = e.balance.saturating_add(*betrag);
        }
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
    /// Ein Lauf mit Inferenz- und Trainingsarbeit, gibt zurueck, was
    /// beide Seiten bekommen haben.
    fn lauf(inferenz_gewicht: u64, training_gewicht: u64) -> (u64, u64) {
        lauf_bei(inferenz_gewicht, training_gewicht, 0)
    }

    /// Derselbe Lauf, aber mit einer Auslastung, die die Abgabe steuert.
    fn lauf_bei(inferenz_gewicht: u64, training_gewicht: u64, auslastung: i64) -> (u64, u64) {
        use crate::zuschreibung::Zuschreibung;
        use myl_ledger::state::LedgerState;
        use myl_types::ids::{Address, MinerId};

        let mut st = LedgerState::genesis(1);
        st.account_mut(&Address::new([1u8; 32])).balance = 10_000_000;
        myl_ledger::transitions::burn_to_credits(
            &mut st,
            &Address::new([1u8; 32]),
            5_000_000,
            myl_types::ids::EpochId(1_000),
        )
        .expect("Burn");
        let inf_miner = MinerId::new([10u8; 32]);
        let tr_miner = MinerId::new([20u8; 32]);
        st.auszahlung.insert(inf_miner, Address::new([11u8; 32]));
        st.auszahlung.insert(tr_miner, Address::new([21u8; 32]));

        let mut inferenz = Zuschreibung::default();
        if inferenz_gewicht > 0 {
            inferenz.je_miner.insert(inf_miner, inferenz_gewicht);
        }
        let mut training = Zuschreibung::default();
        training.je_miner.insert(tr_miner, training_gewicht);

        let vor_inf = st.account(&Address::new([11u8; 32])).balance;
        let vor_tr = st.account(&Address::new([21u8; 32])).balance;
        let _ = epochenausschuettung_mit_training(
            &mut st,
            &inferenz,
            &training,
            auslastung,
            &MintParams { subsidy_num: 1, subsidy_den: 2, m_max: u64::MAX },
        )
        .expect("Ausschuettung");
        (
            st.account(&Address::new([11u8; 32])).balance - vor_inf,
            st.account(&Address::new([21u8; 32])).balance - vor_tr,
        )
    }

    /// ⚑ **Solange die Treasury reicht, ist die Rate exakt gleich.**
    ///
    /// Bei einem Trainingsanteil von drei Prozent des Inferenzgewichts
    /// liegt der Anspruch unter der Treasury, und dann bekommt jede
    /// Gewichtseinheit dasselbe wie bei der Inferenz.
    #[test]
    fn solange_die_treasury_reicht_ist_die_rate_gleich() {
        let (inf, tr) = lauf(1_000, 30);
        assert!(inf > 0 && tr > 0);
        // Je Gewichtseinheit, auf Rundung genau.
        let inf_je = inf / 1_000;
        let tr_je = tr / 30;
        assert!(
            inf_je.abs_diff(tr_je) <= 1,
            "die Rate weicht ab: Inferenz {inf_je} je Einheit, Training {tr_je}"
        );
    }

    /// ⚑ **Bei gleichem Gewicht reicht die Treasury nicht, und das ist
    /// der Befund.**
    ///
    /// Die Treasury bekommt 3 Prozent der Prägung, die Shard-Miner 78.
    /// Ein Trainingsanspruch in Höhe des Inferenzvolumens ist damit
    /// **sechsundzwanzigfach** überzeichnet, und die Vergütung je
    /// Einheit fällt entsprechend.
    ///
    /// ⚑ **Das ist Arithmetik und keine Politik**, und es steht als Test
    /// da, damit die Zahl nicht in einem Kommentar verschwindet: Wer den
    /// Trainingsanteil aus Kap. 7.1 hochsetzt, senkt damit die Vergütung
    /// je Trainingseinheit.
    #[test]
    fn bei_gleichem_gewicht_deckelt_die_treasury_auf_ein_sechsundzwanzigstel() {
        let (inf, tr) = lauf(1_000, 1_000);
        assert!(tr > 0, "der Trainingsminer bekam nichts");
        assert!(tr < inf, "die Treasury deckelt nicht");
        let verhaeltnis = inf / tr;
        assert!(
            (24..=28).contains(&verhaeltnis),
            "Inferenz {inf}, Training {tr}, Verhaeltnis {verhaeltnis} statt rund 26"
        );
    }

    /// Mehr Arbeit bringt mehr, solange die Treasury nicht ausgereizt
    /// ist, und danach nicht mehr.
    #[test]
    fn mehr_arbeit_bringt_mehr_bis_die_treasury_ausgereizt_ist() {
        let (_, wenig) = lauf(1_000, 10);
        let (_, mittel) = lauf(1_000, 30);
        let (_, viel) = lauf(1_000, 100_000);
        assert!(mittel > wenig, "dreimal so viel Arbeit brachte nicht mehr");
        assert!(viel > mittel, "sehr viel Arbeit brachte nicht mehr als mittel");
        assert!(
            viel < wenig.saturating_mul(1_000),
            "die Treasury deckelt nicht: {wenig} auf {viel}"
        );
    }

    /// ⚑ **Bei hoher Auslastung wächst der Puffer.**
    ///
    /// Ohne diesen Test wäre die Abgabe eine Zahl, die niemand einzieht.
    #[test]
    fn bei_hoher_auslastung_waechst_der_puffer() {
        use crate::zuschreibung::Zuschreibung;
        use myl_ledger::state::LedgerState;
        use myl_types::ids::{Address, MinerId};

        let bestand_nach = |auslastung: i64| -> u64 {
            let mut st = LedgerState::genesis(1);
            st.account_mut(&Address::new([1u8; 32])).balance = 10_000_000;
            myl_ledger::transitions::burn_to_credits(
                &mut st,
                &Address::new([1u8; 32]),
                5_000_000,
                myl_types::ids::EpochId(1_000),
            )
            .expect("Burn");
            let inf = MinerId::new([10u8; 32]);
            st.auszahlung.insert(inf, Address::new([11u8; 32]));
            let mut inferenz = Zuschreibung::default();
            inferenz.je_miner.insert(inf, 1_000);
            let _ = epochenausschuettung_mit_training(
                &mut st,
                &inferenz,
                &Zuschreibung::default(),
                auslastung,
                &MintParams { subsidy_num: 1, subsidy_den: 2, m_max: u64::MAX },
            )
            .expect("Ausschuettung");
            st.account(&treasury_adresse()).balance
        };

        let skala = myl_types::auslastung::AUSLASTUNG_SKALA;
        let leer = bestand_nach(0);
        let ziel = bestand_nach(skala * 7 / 10);
        let voll = bestand_nach(skala);
        assert!(ziel > leer, "am Ziel wird nicht mehr abgefuehrt als im Leerlauf");
        assert!(voll > ziel, "bei voller Auslastung wird nicht mehr abgefuehrt");
        // ⚑ Rund das Achtfache: 3 Prozent ohne Abgabe gegen 33 Prozent
        // mit voller Abgabe.
        let faktor = voll / leer.max(1);
        assert!((8..=12).contains(&faktor), "der Faktor ist {faktor} statt rund zehn");
    }

    /// ⚑ **Und bei niedriger Auslastung wird er ausgegeben.**
    ///
    /// Das ist die andere Hälfte des Puffers. Ohne sie wäre die Abgabe
    /// eine Steuer ohne Zweck.
    #[test]
    fn ein_angesparter_puffer_zahlt_mehr_training_als_die_epoche_hergibt() {
        use crate::zuschreibung::Zuschreibung;
        use myl_ledger::state::LedgerState;
        use myl_types::ids::{Address, MinerId};

        let mut st = LedgerState::genesis(1);
        st.account_mut(&Address::new([1u8; 32])).balance = 10_000_000;
        myl_ledger::transitions::burn_to_credits(
            &mut st,
            &Address::new([1u8; 32]),
            5_000_000,
            myl_types::ids::EpochId(1_000),
        )
        .expect("Burn");
        // Ein angesparter Bestand aus guten Epochen.
        st.account_mut(&treasury_adresse()).balance = 50_000_000;
        let geldmenge_vor: u128 =
            st.accounts.values().map(|a| u128::from(a.balance) + u128::from(a.staked)).sum();

        let inf = MinerId::new([10u8; 32]);
        let tr = MinerId::new([20u8; 32]);
        st.auszahlung.insert(inf, Address::new([11u8; 32]));
        st.auszahlung.insert(tr, Address::new([21u8; 32]));
        let mut inferenz = Zuschreibung::default();
        inferenz.je_miner.insert(inf, 1_000);
        let mut training = Zuschreibung::default();
        training.je_miner.insert(tr, 1_000);

        let vor_tr = st.account(&Address::new([21u8; 32])).balance;
        let bestand_vor = st.account(&treasury_adresse()).balance;
        let a = epochenausschuettung_mit_training(
            &mut st,
            &inferenz,
            &training,
            0, // leerlaufendes Netz: keine Abgabe, nur der Bestand
            &MintParams { subsidy_num: 1, subsidy_den: 2, m_max: u64::MAX },
        )
        .expect("Ausschuettung");
        let tr_bekommen = st.account(&Address::new([21u8; 32])).balance - vor_tr;
        let inf_bekommen = st.account(&Address::new([11u8; 32])).balance;

        // ⚑ **Gleiche Rate, weil der Puffer sie trägt.** Ohne Bestand
        // wäre das Training auf ein Sechsundzwanzigstel gedeckelt.
        assert!(
            tr_bekommen > inf_bekommen / 2,
            "der Puffer traegt nicht: Inferenz {inf_bekommen}, Training {tr_bekommen}"
        );
        assert!(
            st.account(&treasury_adresse()).balance < bestand_vor,
            "der Bestand wurde nicht angetastet"
        );

        // ⚑ **Der Griff in den Puffer prägt nicht.** Die Geldmenge darf
        // nur um das Geprägte wachsen, nicht um die Überweisung.
        let geldmenge_nach: u128 =
            st.accounts.values().map(|a| u128::from(a.balance) + u128::from(a.staked)).sum();
        assert_eq!(
            geldmenge_nach - geldmenge_vor,
            u128::from(a.gutgeschrieben),
            "der Puffergriff hat Geld geschaffen"
        );
    }

    /// ⚑ **Ohne Inferenz gibt es keine Rate**, und dann bekommt das
    /// Training, was die Treasury hergibt. Ein leerlaufendes Netz zahlt
    /// seinen Trainingspods aus dem, was da ist.
    #[test]
    fn ohne_inferenz_bekommt_training_die_treasury() {
        let (inf, tr) = lauf(0, 1_000);
        assert_eq!(inf, 0, "ohne Arbeit kein Inferenzanteil");
        assert!(tr > 0, "ein leerlaufendes Netz zahlt seinen Trainingspods nichts");
    }


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
