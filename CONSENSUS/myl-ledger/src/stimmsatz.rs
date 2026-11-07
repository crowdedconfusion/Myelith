//! **Der Stimmsatz aus dem Zustand** (CONSENSUS 7.8, 2026-11-02).
//!
//! ⚑ **Wer stimmt:** jeder eingetragene Miner, dessen Einsatz den
//! Mindesteinsatz erreicht und der die Arbeitsschwelle hält; Gewicht ist der
//! Einsatz (Entscheidung A3, „Arbeit qualifiziert, Stake wiegt“). Über die
//! Prüfpunkte stimmen damit **alle** mit genügend Einsatz und Arbeit ab,
//! nicht ein ausgelostes oder reichstes Komitee. Vorbild ist Casper FFG: Die
//! Sicherheit hängt an einem Drittel des gesamten Einsatzes. 📌 Kritisch
//! geprüft gegen die Alternativen: Die 21 mit dem meisten Einsatz wären eine
//! feste Elite; eine Auslosung nach Einsatz bricht mit 21 Sitzen bei 20 %
//! Angreifer in 4 von 100 Epochen und braucht für Sicherheit rund 512 Sitze,
//! mit einem Restrisiko.
//!
//! ⚑ **Ab wann:** Der Satz für Epoche `e + VORLAUF_EPOCHEN` wird am Ende von
//! Epoche `e` bestimmt (Vorbild CometBFT, Anwendung zwei Höhen später):
//! Jeder kennt ihn eine ganze Epoche im Voraus.
//!
//! ⚑ **Die Startphase** (2026-11-03): Bis das Ziel erstmals tragfähig ist
//! ([`tragfaehig`]: mindestens vier Mitglieder, keines mit einem Drittel,
//! dazu das Mindestgewicht), wird kein Satz geführt, und es gilt der Satz
//! von aussen. Dann gilt das Ziel ganz, wie eine Genesis. Danach geht es nie
//! zurück: Ein späterer, nicht tragfähiger Satz lässt die Finalität ruhen,
//! nie gilt ein alter. 📌 Mit der Wechselgrenze schon ab dem ersten,
//! winzigen Satz hätten früh eingetragene Scheinidentitäten den Satz lange
//! beherrscht, denn jeder spätere Zuwachs zählte gegen ihr kleines Gewicht.
//!
//! ⚑ **Wie stark er sich ändern darf:** Zuwächse (neue Mitglieder und mehr
//! Einsatz) zusammen höchstens [`WECHSEL_NENNER`]tel des alten Gewichts je
//! Wechsel, mindestens aber ein Mindesteinsatz, damit ein kleines Netz
//! wachsen kann; liegen sie darüber, werden **alle anteilig** gekürzt,
//! ganzzahlig und ohne Reihenfolge (eine Reihenfolge nach Grösse hiesse
//! Zentralisierung). 📌 Zuerst stand hier ein Drittel. Das ist nur die
//! Obergrenze, unter der ein Lichtclient einen Wechsel noch prüfen kann, kein
//! Schutz vor Übernahme: Bei einer Stunde je Epoche hielte ein Angreifer mit
//! gut doppeltem ehrlichem Einsatz nach 2 Stunden mehr als ein Drittel, nach
//! 4 Stunden mehr als zwei Drittel. Mit einem Vierundsechzigstel braucht er
//! rund 27 Stunden für ein Drittel und rund 71 für zwei Drittel. Abgänge
//! wirken sofort: Sie lassen niemanden herein, und gekündigter Einsatz haftet
//! bis zum Ende der Sperrfrist.
//!
//! ⚑ **Die Arbeitsschwelle** (Entscheidung A3): Die abgeklungene bezeugte
//! Arbeit eines Kandidaten muss einen Bruchteil des **nach Einsatz
//! gewichteten** Medians der Kandidaten erreichen. 📌 Ein Median über alle
//! Köpfe liesse sich mit Scheinidentitäten ohne Arbeit auf null drücken;
//! gewichtet muss jede von ihnen einen Mindesteinsatz binden, und Aufteilen
//! ändert nichts.
//!
//! ⚑ **Inaktivität** (2026-11-03, Vorbild Ethereum Altair für die Punkte,
//! Cosmos für das Ruhen): Je Prüfpunkt wird vermerkt, wer seine Stimme
//! abgegeben hat, aus dem Zertifikat **oder** aus einer eigenen Transaktion
//! ([`stimme_vermerken`]). Eine Epoche gilt als erfüllt, wer mindestens halb
//! so viele Prüfpunkte gestimmt hat wie das beste Mitglied; sonst gibt es
//! [`STRAFPUNKTE`], für eine erfüllte einen Punkt zurück. Ab
//! [`AUSSCHLUSS_PUNKTE`] fällt ein Mitglied heraus und **ruht**, bis es sich
//! neu anmeldet; dann tritt es wieder unter der Wechselgrenze ein. Wer nie
//! stimmt, ist nach [`INAKTIV_EPOCHEN`] Epochen heraus, wer nur jede zweite
//! Epoche stimmt, nach rund 64; erfüllen muss man auf Dauer vier von fünf.
//! 📌 Zuerst zählte nur, ob jemand in [`INAKTIV_EPOCHEN`] Epochen überhaupt
//! einmal unterschrieben hatte: Eine Stimme je 23 Epochen hielt ihn im Satz.
//! 📌 Und es zählten nur die Zertifikate. Fällt mehr als ein Drittel aus,
//! gibt es keines, also für niemanden einen Beleg; die Regel hätte dann alle
//! hinausgeworfen. Deshalb die Stimme als Transaktion, wie Ethereum
//! Attestierungen einzeln verbucht; sie schützt zugleich davor, dass der
//! Erzeuger Unterschriften aus dem Zertifikat weglässt. Ohne jede Stimme im
//! ganzen Satz wird nicht geurteilt. ⚠️ Ein Abzug am Einsatz wie Ethereums
//! Inaktivitätsabzug wartet auf den wechselnden Erzeuger: Ein fester könnte
//! Stimmtransaktionen zensieren und Ehrliche verbrennen lassen; der
//! Ausschluss ist umkehrbar, ein Abzug nicht.
//!
//! **Schritt 1:** Der Satz wird bestimmt und geführt, wirkt aber noch nicht.

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};

use crate::state::LedgerState;
use myl_types::bls::BlsPublicKey;
use myl_types::{Address, MinerId};

/// Ein Stimmsatz: Kennung, Schlüssel, Gewicht, nach Kennung geordnet.
pub type Satz = Vec<(MinerId, BlsPublicKey, u64)>;

/// So viele Epochen nach seiner Bestimmung gilt ein Satz.
pub const VORLAUF_EPOCHEN: u64 = 2;

/// Zuwächse je Wechsel höchstens dieser Teil des alten Gewichts.
pub const WECHSEL_NENNER: u64 = 64;

/// Ohne jede Stimme in so vielen Epochen fällt ein Mitglied heraus (ein Tag).
pub const INAKTIV_EPOCHEN: u64 = 24;

/// Punkte je verfehlter Epoche; eine erfüllte nimmt einen zurück.
pub const STRAFPUNKTE: u64 = 4;

/// Ab so vielen Punkten fällt ein Mitglied heraus.
pub const AUSSCHLUSS_PUNKTE: u64 = STRAFPUNKTE * INAKTIV_EPOCHEN;

/// So viele Prüfpunkte je Epoche fasst der Vermerk höchstens.
pub const PLAETZE_JE_EPOCHE: u64 = 16;

/// Die Arbeit klingt je Epoche um diesen Teil ab (Zeitkonstante ein Tag).
pub const ABKLING_NENNER: u64 = 24;

/// So viele Mitglieder braucht ein tragfähiger Satz mindestens: Drei Werte,
/// die alle unter einem Drittel ihrer Summe liegen, gibt es nicht.
pub const MINDESTMITGLIEDER: usize = 4;

/// Die Regeln, nach denen der Satz bestimmt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Regeln {
    /// Ab diesem Einsatz ist ein Miner Kandidat.
    pub mindesteinsatz: u64,
    /// Die Arbeitsschwelle als Bruchteil (Zähler, Nenner) des gewichteten
    /// Medians; ein Zähler null heisst keine Schwelle.
    pub arbeit_anteil: (u64, u64),
    /// Ab diesem Gesamtgewicht wirkt der Satz erstmals.
    pub mindestgewicht: u64,
    /// ⚑ Ab so vielen Punkten ruht ein Mitglied (2026-11-05): im Netz
    /// [`AUSSCHLUSS_PUNKTE`], auf der Probekette weniger, damit ein Lauf die
    /// Rückkehr der Finalität nach einem Ausfall von mehr als einem Drittel
    /// in Stunden statt in Tagen messen kann.
    pub ausschluss_punkte: u64,
}

/// Was ein Miner beiträgt, im kleinen Baum geführt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Mitwirkung {
    /// Die abgeklungene bezeugte Arbeit.
    pub arbeit: u64,
    /// Die Inaktivitätspunkte.
    pub punkte: u64,
    /// Je Prüfpunkt der laufenden und der vorigen Epoche ein Bit, ob seine
    /// Stimme vermerkt ist: Bit `(epoche % 2) * 16 + platz`.
    pub stimmen: u32,
    /// Wegen Inaktivität ausgeschlossen, bis er sich neu anmeldet.
    pub ruht: bool,
    /// ⛔️ **Für immer ausgeschlossen** (2026-11-05), nach einem gebuchten
    /// Doppelbeleg: Eine Neuanmeldung hebt das nicht auf. Vorbild Ethereum
    /// (ein geschlachteter Validator kommt nie zurück) und Cosmos
    /// („tombstoned“). 📌 Bis hierher hob jede Neuanmeldung auch das Ruhen
    /// eines Doppelsigners auf; er stand eine Epoche später wieder im Satz.
    pub getilgt: bool,
    /// Wie oft das Mitglied schon wegen Inaktivität geruht hat.
    pub ruhepausen: u32,
    /// ⚑ **Ab dieser Epoche ist es nach einer Neuanmeldung wieder
    /// Kandidat** (CONSENSUS 7.9, 2026-11-05): nach der n-ten Ruhe
    /// 2^(n−1) Epochen, höchstens die Sperrfrist ([`wartezeit`]). Ohne sie
    /// konnte mehr als ein Drittel die Finalität blockieren, ruhen, sich
    /// neu anmelden und wieder blockieren; ein Abzug am Einsatz wie
    /// Ethereums Inaktivitätsabzug ist damit nicht nötig (die Finalität
    /// kehrt über das Ruhen zurück, gemessen), und ein Netzausfall kostet
    /// keinen Einsatz.
    pub wieder_ab: u64,
}

/// Die Wartezeit vor der Rückkehr nach der `n`-ten Ruhe, in Epochen:
/// eine, zwei, vier und so fort, höchstens [`crate::einsatz::SPERRFRIST_EPOCHEN`].
pub fn wartezeit(ruhepausen: u32) -> u64 {
    match ruhepausen {
        0 => 0,
        n => (1u64 << (n - 1).min(62)).min(crate::einsatz::SPERRFRIST_EPOCHEN),
    }
}

/// Das Gesamtgewicht eines Satzes.
pub fn gewicht(satz: &Satz) -> u64 {
    satz.iter().fold(0u64, |s, (_, _, w)| s.saturating_add(*w))
}

/// **Trägt dieser Satz eine Abstimmung?** Mindestens vier Mitglieder, keines
/// mit einem Drittel oder mehr, und das Mindestgewicht; dieselbe Schranke
/// wie für den Satz von aussen.
pub fn tragfaehig(satz: &Satz, mindestgewicht: u64) -> bool {
    let summe = u128::from(gewicht(satz));
    satz.len() >= MINDESTMITGLIEDER
        && summe >= u128::from(mindestgewicht)
        && satz.iter().all(|(_, _, w)| 3 * u128::from(*w) < summe)
}

/// Der nach Einsatz gewichtete Median der Arbeit.
fn gewichteter_median(mut paare: Vec<(u64, u64)>) -> u64 {
    paare.sort_unstable();
    let summe: u128 = paare.iter().map(|(_, e)| u128::from(*e)).sum();
    let mut bisher = 0u128;
    for (arbeit, einsatz) in paare {
        bisher += u128::from(einsatz);
        if 2 * bisher >= summe {
            return arbeit;
        }
    }
    0
}

/// **Wer nach dem Zustand stimmen soll**: ohne die Ruhenden.
pub fn ziel(state: &LedgerState, regeln: &Regeln) -> Satz {
    let kandidaten: Vec<(MinerId, BlsPublicKey, u64, u64)> = state
        .miner
        .values()
        .filter_map(|r| {
            // Aus dem Spiegel im kleinen Baum, nicht aus dem Konto: Der
            // Wechselblock berührt so keinen Zeugen.
            let einsatz = state.einsaetze.get(&Address::new(*r.miner_id.as_bytes())).copied().unwrap_or(0);
            if einsatz < regeln.mindesteinsatz || einsatz == 0 {
                return None;
            }
            let w = state.mitwirkung.get(&r.miner_id).copied().unwrap_or_default();
            if w.ruht || state.epoch.0 < w.wieder_ab {
                return None;
            }
            Some((r.miner_id, r.schluessel, einsatz, w.arbeit))
        })
        .collect();
    let (zaehler, nenner) = regeln.arbeit_anteil;
    let schwelle = if zaehler == 0 || nenner == 0 {
        0
    } else {
        let median = gewichteter_median(kandidaten.iter().map(|(_, _, e, a)| (*a, *e)).collect());
        (u128::from(median) * u128::from(zaehler) / u128::from(nenner)) as u64
    };
    kandidaten.into_iter().filter(|(_, _, _, a)| *a >= schwelle).map(|(m, k, e, _)| (m, k, e)).collect()
}

/// Die Bits der Prüfpunkte einer Epoche.
fn maske(epoche: u64) -> u32 {
    0xFFFF << ((epoche % 2) * 16)
}

/// **Eine Stimme vermerken**: `miner` hat über den Prüfpunkt an Platz
/// `platz` der Epoche `epoche_des_pruefpunkts` gestimmt. Nur für die
/// laufende Epoche `jetzt` und die vorige, nur für eingetragene Miner, je
/// Prüfpunkt einmal. Gibt zurück, ob die Stimme neu ist.
pub fn stimme_vermerken(state: &mut LedgerState, miner: &MinerId, epoche_des_pruefpunkts: u64, platz: u64, jetzt: u64) -> bool {
    if platz >= PLAETZE_JE_EPOCHE
        || epoche_des_pruefpunkts > jetzt
        || epoche_des_pruefpunkts.saturating_add(1) < jetzt
        || !state.miner.contains_key(miner)
    {
        return false;
    }
    let bit = 1u32 << ((epoche_des_pruefpunkts % 2) * 16 + platz);
    let w = state.mitwirkung.get(miner).copied().unwrap_or_default();
    if w.stimmen & bit != 0 {
        return false;
    }
    state.mitwirkung.insert(*miner, Mitwirkung { stimmen: w.stimmen | bit, ..w });
    true
}

/// Ob die Stimme von `miner` über diesen Prüfpunkt vermerkt ist.
pub fn stimme_vermerkt(state: &LedgerState, miner: &MinerId, epoche_des_pruefpunkts: u64, platz: u64) -> bool {
    platz < PLAETZE_JE_EPOCHE
        && state.mitwirkung.get(miner).is_some_and(|w| w.stimmen & (1u32 << ((epoche_des_pruefpunkts % 2) * 16 + platz)) != 0)
}

/// **Die Teilnahme der Epoche `epoche` urteilen**: Punkte für die Mitglieder
/// ihres Satzes, Ruhen ab `ausschluss_punkte` ([`Regeln`]); danach werden ihre Bits für
/// die übernächste Epoche frei. Ohne jede Stimme im Satz wird nicht
/// geurteilt: Dann fehlt der Beleg, nicht der Wille.
fn teilnahme_urteilen(state: &mut LedgerState, epoche: u64, ausschluss_punkte: u64) {
    let maske = maske(epoche);
    if let Some(satz) = state.stimmsaetze.get(&epoche).cloned() {
        let gestimmt = |state: &LedgerState, m: &MinerId| {
            state.mitwirkung.get(m).map_or(0, |w| (w.stimmen & maske).count_ones())
        };
        let bestes = satz.iter().map(|(m, _, _)| gestimmt(state, m)).max().unwrap_or(0);
        if bestes > 0 {
            for (m, _, _) in &satz {
                if !state.miner.contains_key(m) {
                    continue;
                }
                let erfuellt = 2 * gestimmt(state, m) >= bestes;
                let w = state.mitwirkung.entry(*m).or_default();
                w.punkte = if erfuellt { w.punkte.saturating_sub(1) } else { w.punkte.saturating_add(STRAFPUNKTE) };
                if w.punkte >= ausschluss_punkte && !w.ruht {
                    w.ruht = true;
                    w.ruhepausen = w.ruhepausen.saturating_add(1);
                }
            }
        }
    }
    let belegt: Vec<MinerId> = state.mitwirkung.iter().filter(|(_, w)| w.stimmen & maske != 0).map(|(m, _)| *m).collect();
    for m in belegt {
        if let Some(w) = state.mitwirkung.get_mut(&m) {
            w.stimmen &= !maske;
        }
    }
}

/// ⛔️ **Ausschliessen nach einer Doppelsignatur**: Der Miner ruht für
/// immer, eine Neuanmeldung unter diesem Schlüssel hebt das nicht auf
/// ([`Mitwirkung::getilgt`]). Wer weiter mitwirken will, meldet einen neuen
/// Schlüssel mit neuem Einsatz an.
pub fn ausschliessen(state: &mut LedgerState, miner: &MinerId) {
    if state.miner.contains_key(miner) {
        let w = state.mitwirkung.entry(*miner).or_default();
        w.ruht = true;
        w.getilgt = true;
    }
}

/// **Der nächste Satz vom alten zum Ziel**, mit der Wechselgrenze: Zuwächse
/// zusammen höchstens `gewicht(alt) / WECHSEL_NENNER`, mindestens
/// `mindestbudget`, alle anteilig; Abgänge sofort.
pub fn naechster(alt: &Satz, ziel: &Satz, mindestbudget: u64) -> Satz {
    let budget = (gewicht(alt) / WECHSEL_NENNER).max(mindestbudget);
    let alt_w = |m: &MinerId| alt.iter().find(|(k, _, _)| k == m).map_or(0, |(_, _, w)| *w);
    let zuwachs: u128 = ziel.iter().map(|(m, _, w)| u128::from(w.saturating_sub(alt_w(m)))).sum();
    ziel.iter()
        .filter_map(|(m, k, w)| {
            let vorher = alt_w(m);
            let neu = if *w <= vorher {
                *w
            } else {
                let plus = u128::from(w - vorher);
                let erlaubt = if zuwachs <= u128::from(budget) { plus } else { plus * u128::from(budget) / zuwachs };
                vorher + erlaubt as u64
            };
            (neu > 0).then_some((*m, *k, neu))
        })
        .collect()
}

/// **Am Epochenwechsel**: schreibt die Arbeit der abgelaufenen Epoche fort,
/// urteilt über die Teilnahme der vorvorigen (eine Epoche Nachfrist für
/// Stimmtransaktionen), bestimmt den Satz für `epoche + VORLAUF_EPOCHEN`,
/// legt ihn ab und vergisst Sätze vor der Vorepoche (ein Zertifikat des
/// letzten Prüfpunkts einer Epoche kommt erst in der nächsten an). Ein
/// Übergang, nie beim Lesen.
pub fn fortschreiben(state: &mut LedgerState, epoche: u64, regeln: &Regeln, arbeit: &BTreeMap<MinerId, u64>) {
    if epoche >= 2 {
        teilnahme_urteilen(state, epoche - 2, regeln.ausschluss_punkte);
    }
    let miner: Vec<MinerId> = state.miner.keys().copied().collect();
    for m in &miner {
        let vorher = state.mitwirkung.get(m).copied().unwrap_or_default();
        let mut w = vorher;
        w.arbeit = w.arbeit - w.arbeit / ABKLING_NENNER;
        w.arbeit = w.arbeit.saturating_add(arbeit.get(m).copied().unwrap_or(0));
        if w != vorher {
            state.mitwirkung.insert(*m, w);
        }
    }
    let weg: Vec<MinerId> = state.mitwirkung.keys().filter(|m| !state.miner.contains_key(m)).copied().collect();
    for m in weg {
        state.mitwirkung.remove(&m);
    }

    let fuer = epoche.saturating_add(VORLAUF_EPOCHEN);
    let alt = state.stimmsaetze.range(..fuer).next_back().map(|(_, s)| s.clone());
    let ziel = ziel(state, regeln);
    let neu = match &alt {
        // Die Startphase: erst ein tragfähiges Ziel, dann ganz.
        None => tragfaehig(&ziel, regeln.mindestgewicht).then_some(ziel),
        Some(alt) => Some(naechster(alt, &ziel, regeln.mindesteinsatz)),
    };
    if let Some(neu) = neu {
        state.stimmsaetze.insert(fuer, neu);
    }
    state.stimmsaetze.retain(|e, _| e.saturating_add(1) >= epoche);
}

/// **Neu angemeldet**: Wer ruht, kann wieder hinein, ohne Punkte, aber erst
/// nach der Wartezeit seiner Ruhen ([`wartezeit`], 7.9).
pub fn wieder_bereit(state: &mut LedgerState, miner: &MinerId) {
    let jetzt = state.epoch.0;
    if let Some(w) = state.mitwirkung.get_mut(miner).filter(|w| w.ruht && !w.getilgt) {
        w.ruht = false;
        w.punkte = 0;
        w.wieder_ab = jetzt.saturating_add(wartezeit(w.ruhepausen));
    }
}

/// **Das Gesamtgewicht in Einsatz** für `epoche`: das des geführten Satzes,
/// vor der Startphase aller hinterlegte Einsatz. Nenner der
/// Korrelationsstrafe; ⛔️ nie das Gewicht eines Satzes von aussen, das in
/// anderen Einheiten zählen kann.
pub fn gesamtgewicht(state: &LedgerState, epoche: u64) -> u64 {
    match satz_der_epoche(state, epoche) {
        Some(satz) => gewicht(satz),
        None => state.einsaetze.values().fold(0u64, |s, e| s.saturating_add(*e)),
    }
}

/// Der für `epoche` geführte Satz, wenn es einen gibt.
pub fn satz_der_epoche(state: &LedgerState, epoche: u64) -> Option<&Satz> {
    state.stimmsaetze.get(&epoche)
}

/// ⚑ **Die Bindung eines Pruefpunkts der Epoche `epoche`** (7.3,
/// 2026-11-04): die Saetze fuer `epoche + 1` und `epoche + 2`, wie der
/// Zustand sie fuehrt. Waehrend der ganzen Epoche gleich (beide stehen ab
/// ihrem ersten Block fest) und bis zum Ende der uebernaechsten berechenbar;
/// jede Pruefpunktstimme unterschreibt sie mit.
pub fn bindung(state: &LedgerState, epoche: u64) -> myl_types::Hash {
    bindung_aus(satz_der_epoche(state, epoche.saturating_add(1)), satz_der_epoche(state, epoche.saturating_add(2)))
}

/// `SHA-256("myl-satzbindung-v1" ‖ borsh((naechster, uebernaechster)))`.
pub fn bindung_aus(naechster: Option<&Satz>, uebernaechster: Option<&Satz>) -> myl_types::Hash {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(b"myl-satzbindung-v1");
    h.update(borsh::to_vec(&(naechster, uebernaechster)).expect("Borsh"));
    myl_types::Hash::from_bytes(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(b: u8) -> BlsPublicKey {
        myl_types::bls::BlsSecretKey::key_gen(&[b; 32]).unwrap().public_key().unwrap()
    }
    fn m(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }
    fn gewichte(s: &Satz) -> Vec<u64> {
        s.iter().map(|(_, _, w)| *w).collect()
    }

    const REGELN: Regeln = Regeln { mindesteinsatz: 100, arbeit_anteil: (0, 1), mindestgewicht: 400, ausschluss_punkte: AUSSCHLUSS_PUNKTE };

    /// Meldet einen Miner an und hinterlegt `einsatz`.
    fn miner_mit(s: &mut LedgerState, b: u8, einsatz: u64) -> MinerId {
        let a = myl_types::bls::BlsSecretKey::key_gen(&[b; 32]).unwrap().public_key().unwrap();
        let id = MinerId::aus_schluessel(&a);
        crate::transitions::miner_anmelden(
            s,
            &Address::aus_schluessel(&a),
            &id,
            myl_types::miner::HardwareClass::SmallGpu,
            myl_types::node_metadata::GeoRegion::Europe,
            a,
            myl_types::latency_attest::PeerIdBytes([0; 32]),
        )
        .expect("Anmeldung");
        let konto = Address::new(*id.as_bytes());
        s.account_mut(&konto).balance = einsatz;
        crate::transitions::einsatz_hinterlegen(s, &konto, einsatz).expect("Einsatz");
        id
    }

    /// ⛔️ Die Wechselgrenze: Zuwächse zusammen höchstens ein
    /// Vierundsechzigstel des alten Gewichts, mindestens das Mindestbudget,
    /// alle anteilig; Abgänge sofort.
    #[test]
    fn der_wechsel_ist_begrenzt_und_anteilig() {
        let alt: Satz = vec![(m(1), k(1), 6_400), (m(2), k(2), 3_200)];
        // Budget 150 (9 600 / 64), Zuwachs 1 200: ein Achtel von jedem.
        let ziel: Satz = vec![(m(1), k(1), 6_400), (m(2), k(2), 3_200), (m(3), k(3), 900), (m(4), k(4), 300)];
        let neu = naechster(&alt, &ziel, 100);
        assert_eq!(gewichte(&neu), vec![6_400, 3_200, 112, 37]);
        assert!(gewicht(&neu) - gewicht(&alt) <= gewicht(&alt) / WECHSEL_NENNER);
        // Ein Abgang wirkt sofort, ganz.
        let ohne: Satz = vec![(m(1), k(1), 6_400)];
        assert_eq!(naechster(&alt, &ohne, 100), ohne);
        // Das Mindestbudget gilt auch bei kleinem altem Gewicht.
        let klein: Satz = vec![(m(1), k(1), 64)];
        let mehr: Satz = vec![(m(1), k(1), 64), (m(5), k(5), 500)];
        assert_eq!(gewichte(&naechster(&klein, &mehr, 100)), vec![64, 100]);
        // Unter dem Budget ganz.
        let wenig: Satz = vec![(m(1), k(1), 6_400), (m(2), k(2), 3_200), (m(5), k(5), 100)];
        assert_eq!(naechster(&alt, &wenig, 100), wenig);
    }

    /// Tragfähig: vier Mitglieder, keines mit einem Drittel, das Mindestgewicht.
    #[test]
    fn tragfaehig_braucht_vier_ohne_drittel_und_gewicht() {
        let vier: Satz = (1..=4).map(|b| (m(b), k(b), 100)).collect();
        assert!(tragfaehig(&vier, 400));
        assert!(!tragfaehig(&vier, 401), "Mindestgewicht");
        assert!(!tragfaehig(&vier[..3].to_vec(), 0), "drei sind zu wenig");
        let mut gross = vier.clone();
        gross[0].2 = 150;
        assert!(!tragfaehig(&gross, 0), "150 von 450 ist ein Drittel");
    }

    /// Die Startphase: kein Satz, bis das Ziel tragfähig ist, dann ganz.
    #[test]
    fn die_startphase_endet_mit_dem_ersten_tragfaehigen_ziel() {
        let mut s = LedgerState::genesis(1);
        let leer = BTreeMap::new();
        miner_mit(&mut s, 7, 500);
        fortschreiben(&mut s, 3, &REGELN, &leer);
        assert!(s.stimmsaetze.is_empty(), "ein Mitglied ist nicht tragfähig");
        for b in 8..=10 {
            miner_mit(&mut s, b, 500);
        }
        fortschreiben(&mut s, 4, &REGELN, &leer);
        assert_eq!(satz_der_epoche(&s, 6).map(gewicht), Some(2_000), "das Ziel ganz, ohne Wechselgrenze");
        // Danach mit der Grenze: Ein Grosser kommt nur um das Budget herein.
        miner_mit(&mut s, 11, 100_000);
        fortschreiben(&mut s, 5, &REGELN, &leer);
        assert_eq!(satz_der_epoche(&s, 7).map(gewicht), Some(2_000 + 100), "Budget: Mindesteinsatz");
        // Ältere als die Vorepoche fallen.
        fortschreiben(&mut s, 8, &REGELN, &leer);
        assert!(satz_der_epoche(&s, 6).is_none());
        assert!(satz_der_epoche(&s, 7).is_some(), "die Vorepoche bleibt");
    }

    /// Die Arbeitsschwelle gegen den nach Einsatz gewichteten Median.
    #[test]
    fn ohne_arbeit_kein_platz_im_satz() {
        let mut s = LedgerState::genesis(1);
        let ids: Vec<MinerId> = (20..=24).map(|b| miner_mit(&mut s, b, 500)).collect();
        let arbeit: BTreeMap<MinerId, u64> = ids[..4].iter().map(|m| (*m, 1_000)).collect();
        let regeln = Regeln { arbeit_anteil: (1, 4), ..REGELN };
        fortschreiben(&mut s, 3, &regeln, &arbeit);
        let satz = satz_der_epoche(&s, 5).expect("tragfähig");
        assert_eq!(satz.len(), 4, "der ohne Arbeit fehlt");
        assert!(!satz.iter().any(|(m, _, _)| *m == ids[4]));
        // Viele Scheinidentitäten ohne Arbeit drücken den gewichteten Median
        // erst, wenn sie mehr Einsatz binden als die Arbeitenden.
        assert_eq!(gewichteter_median(vec![(1_000, 2_000), (0, 100), (0, 100), (0, 100)]), 1_000);
    }

    /// Stimmt `wer` in Epoche `e` über `n` Prüfpunkte ab.
    fn stimmen(s: &mut LedgerState, wer: &[MinerId], e: u64, n: u64) {
        for m in wer {
            for platz in 0..n {
                stimme_vermerken(s, m, e, platz, e);
            }
        }
    }

    /// Inaktivität: heraus nach der Frist, ruhend bis zur Neuanmeldung.
    #[test]
    fn wer_nicht_stimmt_faellt_heraus_und_ruht() {
        let mut s = LedgerState::genesis(1);
        let leer = BTreeMap::new();
        let ids: Vec<MinerId> = (30..=34).map(|b| miner_mit(&mut s, b, 500)).collect();
        fortschreiben(&mut s, 1, &REGELN, &leer);
        assert_eq!(satz_der_epoche(&s, 3).map(|x| x.len()), Some(5));
        // Alle ausser dem letzten stimmen in jeder Epoche.
        let mut e = 2;
        while e <= 3 + INAKTIV_EPOCHEN + 2 {
            stimmen(&mut s, &ids[..4], e - 1, 12);
            fortschreiben(&mut s, e, &REGELN, &leer);
            e += 1;
        }
        let zuletzt = satz_der_epoche(&s, e + 1).expect("Satz");
        assert_eq!(zuletzt.len(), 4, "der Stumme ist heraus");
        assert!(s.mitwirkung.get(&ids[4]).is_some_and(|w| w.ruht));
        assert!(ids[..4].iter().all(|m| s.mitwirkung.get(m).is_some_and(|w| w.punkte == 0 && !w.ruht)));
        // Er kommt nicht von selbst zurück, erst nach der Neuanmeldung.
        fortschreiben(&mut s, e, &REGELN, &leer);
        assert_eq!(satz_der_epoche(&s, e + 2).map(|x| x.len()), Some(4));
        wieder_bereit(&mut s, &ids[4]);
        assert!(s.mitwirkung.get(&ids[4]).is_some_and(|w| w.punkte == 0 && !w.ruht && w.ruhepausen == 1));
        // ⚑ 7.9: nach der ersten Ruhe eine Epoche Wartezeit.
        let warten_bis = s.mitwirkung.get(&ids[4]).map(|w| w.wieder_ab).expect("Mitwirkung");
        assert_eq!(warten_bis, s.epoch.0 + 1);
        s.epoch.0 += 1;
        fortschreiben(&mut s, e + 1, &REGELN, &leer);
        assert_eq!(satz_der_epoche(&s, e + 3).map(|x| x.len()), Some(5));
        // ⛔️ Nach einem Doppelbeleg hilft keine Neuanmeldung.
        ausschliessen(&mut s, &ids[3]);
        wieder_bereit(&mut s, &ids[3]);
        assert!(s.mitwirkung.get(&ids[3]).is_some_and(|w| w.ruht && w.getilgt), "getilgt bleibt getilgt");
        fortschreiben(&mut s, e + 2, &REGELN, &leer);
        assert!(satz_der_epoche(&s, e + 4).is_some_and(|x| x.iter().all(|(m, _, _)| *m != ids[3])));
    }

    /// 📌 Wer nur selten stimmt, fällt auch heraus; wer halb so oft wie der
    /// Beste stimmt, nicht.
    #[test]
    fn wer_selten_stimmt_faellt_auch_heraus() {
        let mut s = LedgerState::genesis(1);
        let leer = BTreeMap::new();
        let ids: Vec<MinerId> = (40..=45).map(|b| miner_mit(&mut s, b, 500)).collect();
        fortschreiben(&mut s, 1, &REGELN, &leer);
        let mut e = 2;
        while e < 3 + 80 {
            stimmen(&mut s, &ids[..4], e - 1, 12);
            // Der Halbe stimmt über die Hälfte, der Seltene jede zweite Epoche.
            stimmen(&mut s, &ids[4..5], e - 1, 6);
            if e % 2 == 0 {
                stimmen(&mut s, &ids[5..6], e - 1, 12);
            }
            fortschreiben(&mut s, e, &REGELN, &leer);
            e += 1;
        }
        assert!(s.mitwirkung.get(&ids[5]).is_some_and(|w| w.ruht), "jede zweite Epoche reicht nicht");
        assert!(s.mitwirkung.get(&ids[4]).is_some_and(|w| !w.ruht && w.punkte == 0), "die Hälfte reicht");
    }

    /// 📌 Ohne jede Stimme im Satz wird nicht geurteilt: Sonst fiele bei
    /// einem Ausfall aller Zertifikate der ganze Satz heraus.
    #[test]
    fn ohne_jede_stimme_wird_nicht_geurteilt() {
        let mut s = LedgerState::genesis(1);
        let leer = BTreeMap::new();
        for b in 50..=53 {
            miner_mit(&mut s, b, 500);
        }
        for e in 1..=3 + 2 * INAKTIV_EPOCHEN {
            fortschreiben(&mut s, e, &REGELN, &leer);
        }
        assert!(s.mitwirkung.values().all(|w| w.punkte == 0 && !w.ruht));
        assert_eq!(satz_der_epoche(&s, 5 + 2 * INAKTIV_EPOCHEN).map(|x| x.len()), Some(4));
    }

    /// Eine Stimme zählt je Prüfpunkt einmal und nur in ihrem Fenster;
    /// nach dem Urteil wird ihr Bit frei.
    #[test]
    fn eine_stimme_zaehlt_einmal_im_fenster() {
        let mut s = LedgerState::genesis(1);
        let m = miner_mit(&mut s, 60, 500);
        assert!(stimme_vermerken(&mut s, &m, 5, 3, 6));
        assert!(!stimme_vermerken(&mut s, &m, 5, 3, 6), "zweimal");
        assert!(stimme_vermerkt(&s, &m, 5, 3));
        assert!(!stimme_vermerken(&mut s, &m, 4, 3, 6), "zu alt");
        assert!(!stimme_vermerken(&mut s, &m, 7, 3, 6), "aus der Zukunft");
        assert!(!stimme_vermerken(&mut s, &m, 6, PLAETZE_JE_EPOCHE, 6), "kein Platz");
        assert!(!stimme_vermerken(&mut s, &MinerId::new([9; 32]), 6, 0, 6), "nicht eingetragen");
        assert!(stimme_vermerken(&mut s, &m, 6, 3, 6), "die andere Epoche hat eigene Bits");
        teilnahme_urteilen(&mut s, 5, AUSSCHLUSS_PUNKTE);
        assert!(!stimme_vermerkt(&s, &m, 5, 3));
        assert!(stimme_vermerkt(&s, &m, 6, 3));
    }

    /// ⚑ **Die Wartezeit wächst** (7.9, 2026-11-05): eine, zwei, vier
    /// Epochen, höchstens die Sperrfrist; ohne Ruhe keine.
    #[test]
    fn die_wartezeit_waechst() {
        assert_eq!((wartezeit(0), wartezeit(1), wartezeit(2), wartezeit(3)), (0, 1, 2, 4));
        assert_eq!(wartezeit(40), crate::einsatz::SPERRFRIST_EPOCHEN);
        assert_eq!(wartezeit(u32::MAX), crate::einsatz::SPERRFRIST_EPOCHEN);
    }
}
