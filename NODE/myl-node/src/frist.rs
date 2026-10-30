//! **Die Frist im Knoten** (Stufe 4 des Dispatchers, 6.8, Festlegung des
//! Projektinhabers vom 2026-10-22): messen, bezeugen, sammeln, einreichen.
//!
//! # ⚑ Der Weg
//!
//! 1. Einmal je Epoche stellt jedes Mitglied eines Pods jedem anderen
//!    rechnenden Mitglied eine **Zeitprobe** und misst sie, mehrmals; die
//!    kürzeste zählt, abzüglich einer festen Zugabe für den Weg
//!    ([`gemessen_je_token`]).
//! 2. Liegt die Messung über der Frist ([`frist_des_mitglieds`]), unterschreibt
//!    es ein **Fristzeugnis** und schickt es an die übrigen Mitglieder.
//! 3. Jedes Mitglied **sammelt** Zeugnisse ([`Fristsammlung`]); sobald eine
//!    Mehrheit der übrigen über der Frist liegt und es selbst darunter ist,
//!    reicht es die Meldung ein. Mehrere Einreichungen schaden nicht: Die
//!    Kette nimmt eine und weist die übrigen ab.
//!
//! ⚑ **Jedes Zeugnis ist eine eigene Messung.** Ein Mitglied, das eine
//! fremde Zahl unterschriebe, bezeugte nichts; darum misst jedes selbst.
//!
//! ⚠️ **Zwei Grenzen, beide bekannt.** Eine Meldung gilt nur in ihrer
//! Epoche: Kommt sie erst nach dem Wechsel in einen Block, weist die Kette
//! sie ab, und der Verstoß fällt erst bei der Messung der nächsten Epoche
//! auf. Und die Zugabe für den Weg ist eine Auflösung: Wer je Token weniger
//! als [`ZUGABE_US`] durch die Grösse seiner Probe ([`probe_positionen`])
//! über seiner Frist liegt, fällt nicht auf.

use myl_ledger::state::LedgerState;
use myl_types::fristmeldung::{Fristmeldung, Fristzeugnis};
use myl_types::ids::{EpochId, MinerId};

/// **Die Frist eines Mitglieds in der laufenden Epoche**, in Mikrosekunden
/// je Token: das Doppelte seiner angesagten Zeit über seine Ebenen, samt
/// Kopf in Ebenenzeiten, wenn es ihn trägt. `None`, wenn es keine Position
/// hat oder der Pod nicht machbar ist.
pub fn frist_des_mitglieds(zustand: &LedgerState, pod_index: u32, mitglied: &MinerId) -> Option<u64> {
    let pm = zustand.podmodell?;
    let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
    let pod = zuteilung.pods.iter().find(|p| p.pod_index == pod_index)?;
    let stelle = pod.shards.iter().position(|s| s.miner.miner_id == *mitglied)?;
    let grenzen = crate::kette::Kette::grenzen_des_pods(zustand, pod)?;
    let (von, bis) = grenzen[stelle];
    let kopf = if stelle + 1 == pod.shards.len() { pm.kopf_zeit } else { 0 };
    let zusage = myl_ledger::transitions::modellzusage_der_epoche(zustand, mitglied, zustand.epoch)?;
    Some(zusage.frist_us(bis - von + kopf))
}

/// **Wie lange eine Zeitprobe einen ehrlichen Shard beschäftigt**, in
/// Mikrosekunden: 2 s. Daraus folgt ihre Grösse ([`probe_positionen`]).
pub const PROBE_ZIEL_US: u64 = 2_000_000;

/// Mindestens so viele Positionen rechnet eine Zeitprobe.
pub const PROBE_MINDESTENS: u32 = 64;

/// Höchstens so viele Positionen rechnet eine Zeitprobe; der Shard rechnet
/// nicht mehr (`myl_pod::shard::ZEITPROBE_HOECHSTENS`).
pub const PROBE_HOECHSTENS: u32 = 2_048;

/// **Wie viele Positionen die Zeitprobe eines Mitglieds rechnet**: so
/// viele, dass seine Zusage die Probe [`PROBE_ZIEL_US`] lang beschäftigt,
/// begrenzt auf [`PROBE_MINDESTENS`] bis [`PROBE_HOECHSTENS`]. `frist_us`
/// ist seine Frist je Token, also die Zusage mal `FRIST_FAKTOR`.
///
/// ⚑ **Festlegung des Projektinhabers vom 2026-10-27, nachgeschärft beim
/// Bau.** Mit einer festen Grösse von 64 Positionen fiel ein Lügner erst
/// auf, wenn er je Token mehr als Zugabe/64 ≈ 31 ms über seiner Frist lag;
/// ein Shard rechnet aber 2 bis 8 ms je Token, und der Lügner der Podläufe
/// (50 µs angesagt, rund 8 ms gerechnet) wäre durchgekommen. **Wer schneller
/// ansagt, bekommt die grössere Probe**: Die Lüge vergrössert ihre eigene
/// Prüfung, und ein Lügner fällt auf, sobald er mehr als rund das Dreifache
/// seiner Zusage rechnet (die Frist erlaubt das Doppelte, die Zugabe ein
/// Ziel mehr); an der Obergrenze ist die Schranke absolut, rund
/// Zugabe/2048 ≈ 1 ms über der Frist. Ein Ehrlicher zahlt je Probe ungefähr
/// das Ziel, gleich wie schnell er ist, und sein Spielraum für den Weg ist
/// Zugabe plus Ziel.
///
/// ⚠️ Die späten Positionen einer langen Probe rechnen gegen einen längeren
/// Kontext und sind etwas teurer; das trifft den Ehrlichen innerhalb des
/// Faktors zwei und macht den Lügner nur langsamer.
pub fn probe_positionen(frist_us: u64) -> u32 {
    let k = PROBE_ZIEL_US.saturating_mul(myl_types::zusage::FRIST_FAKTOR) / frist_us.max(1);
    k.clamp(u64::from(PROBE_MINDESTENS), u64::from(PROBE_HOECHSTENS)) as u32
}

/// ⚑ **Ist der eigene Shard zu langsam?** (Selbstprobe, Fund 677, Frage 6
/// vom 2026-10-29): mehr als das Anderthalbfache der Zusage je Token, also
/// mehr als drei Viertel der Frist. Bis zur Frist bleibt so ein Viertel.
pub fn selbstprobe_zu_langsam(je_token_us: u64, frist_us: u64) -> bool {
    let zusage_us = frist_us / myl_types::zusage::FRIST_FAKTOR;
    zusage_us > 0 && je_token_us.saturating_mul(2) > zusage_us.saturating_mul(3)
}

/// **Die angepasste Ebenenzeit**: die jetzige, gestreckt um das Verhältnis
/// von gemessener Zeit zur Zusage, mit einem Viertel Luft. `None`, wenn sie
/// nicht langsamer würde: Schneller wird eine Zusage nie von selbst.
pub fn angepasste_ebenenzeit(jetzt_us: u64, je_token_us: u64, frist_us: u64) -> Option<u64> {
    let zusage_us = frist_us / myl_types::zusage::FRIST_FAKTOR;
    if zusage_us == 0 {
        return None;
    }
    let neu = u128::from(jetzt_us) * u128::from(je_token_us) * 5 / (u128::from(zusage_us) * 4);
    let neu = u64::try_from(neu).unwrap_or(u64::MAX);
    (neu > jetzt_us).then_some(neu)
}

/// **Die feste Zugabe für den Weg**, in Mikrosekunden: 2 s.
///
/// 📌 **Fund 646 (2026-10-26/27): Bis hierher wurde ein Ping abgezogen, und
/// der Gemessene konnte ihn verlängern.** Ping und Zeitprobe sind
/// verschiedene Nachforderungen; wer nur seine Pongs zurückhielt, machte
/// die Probe kürzer als den Ping, und eine solche Messung wurde verworfen
/// und wiederholt (Fund 590), ohne Ende. Auch ohne Absicht wurde im langen
/// Podlauf die Hälfte aller Messungen verworfen, weil der Weg um Hunderte
/// Millisekunden schwankte. ⚑ **Jetzt wird nichts gemessen, was der
/// Gemessene beeinflussen kann, ausser der Probe selbst**, und eine Probe
/// kann er nur verlängern (Festlegung des Projektinhabers vom 2026-10-27).
///
/// Warum 2 s: Im langen Podlauf lag schon die kürzeste von fünf Proben im
/// ungünstigen Zehntel bei 1,5 s; eine falsche Sperre wiegt schwerer als ein
/// Lügner, der knapp durchkommt (Fund 655). Vor dem Genesis verteilt
/// nachzumessen.
pub const ZUGABE_US: u64 = 2_000_000;

/// Wie viele Zeitproben je Messung; die kürzeste zählt.
///
/// 📌 **Fund 639 (2026-10-25, im Podlauf unter `tc netem`): Eine einzige
/// Probe trug jeden Verlust des Netzes als Rechenzeit.** Ein verlorenes
/// Paket kostet eine Neuübertragung, oft über 200 ms. Zwei ehrliche Miner
/// mit je einer Ebene (Frist 40 und 80 ms) wurden zwischen 1 und 770 ms
/// gemessen, drei Zeugen lagen darüber, und die Kette sperrte sie. Verlust
/// und Schwankung verlängern eine Probe nur; die kürzeste von mehreren liegt
/// nahe an Weg und Rechnung, und darunter kommt auch ein Lügner nicht.
pub const PROBEN_JE_MESSUNG: u32 = 5;

/// **Ist eine Messung nach einer Antwort schon entschieden?** Nach
/// [`PROBEN_JE_MESSUNG`] Proben immer; vorher, sobald die kürzeste Probe die
/// Frist hält: Weitere Proben können sie nur verkürzen, das Urteil bliebe
/// [`Urteil::Bestanden`].
///
/// ⚑ (2026-10-28) Ein Ehrlicher belegt seinen einzigen Platz für Zeitproben
/// (Fund 609) so meist für eine Probe statt fünf, und die übrigen Messenden
/// seines Pods kommen früher an die Reihe ([`ABWEISUNGEN_MINDESTENS`]).
pub fn messung_fertig(kuerzeste_us: u64, proben: u32, frist_us: u64, positionen: u32) -> bool {
    proben >= PROBEN_JE_MESSUNG || gemessen_je_token(kuerzeste_us, positionen) <= frist_us
}

/// **So viele Abweisungen in Folge braucht eine Überschreitung mindestens**:
/// vier, und sie müssen zusammen [`abweisungsdauer`] überspannen.
///
/// 📌 **Fund 660 (2026-10-27): Wer jede Zeitprobe mit `Nichts` beantwortete,
/// wurde nie gemessen.** Ein `Nichts` oder eine gescheiterte Anfrage warf
/// die Messung samt ihrer beantworteten Proben weg, und bei der nächsten
/// Aufnahme begann eine neue. ⚑ **Festlegung des Projektinhabers vom
/// 2026-10-28:** Eine Abweisung durch einen Positionsinhaber zählt wie eine
/// Überschreitung, mit denselben zwei Fenstern und derselben Mehrheit; nach
/// einer Abweisung fragt der Messende mit Abstand erneut.
///
/// ⚠️ **Auch ein Ehrlicher weist ab**, wenn sein einziger Platz für
/// Zeitproben gerade belegt ist (Fund 609), und zu Beginn des Messfensters
/// fragen alle Mitglieder zugleich. Eine feste Zahl von Abweisungen trägt
/// das nicht: Simuliert (Probe `ehrliches_gedraenge_ist_keine_verweigerung`)
/// endeten mit vier Abweisungen in Folge bei fünf Messenden rund 18 % der
/// Runden mit einer falschen Überschreitung, bei sieben über 80 %. Das
/// Gedränge wächst mit der Zahl der Messenden, also wächst die Dauer mit.
pub const ABWEISUNGEN_MINDESTENS: u32 = 4;

/// **Wie lange ein Gemessener ohne Antwort abweisen muss, bis es zählt**,
/// in Mikrosekunden: dreimal so lange, wie alle `messende` zusammen seinen
/// Platz mit je einer Probe ([`PROBE_ZIEL_US`]) belegen.
///
/// Warum dreimal: Mit einfacher Dauer blieben in der Simulation bei fünf
/// bis 23 Messenden bis zu 80 % der Runden mit einer falschen
/// Überschreitung, mit dreifacher keine von 72 000 (Startstreuung 2 s und
/// 30 s, Weg 50 bis 600 ms). Ein Verweigerer wird so nach rund 7 s je
/// Messendem erfasst; mit einer Epoche von einer Stunde ist das wenig, mit
/// den 108 s eines gerafften Laufs viel (Fund 664).
pub fn abweisungsdauer(messende: usize) -> u64 {
    3 * (messende.max(1) as u64) * PROBE_ZIEL_US
}

/// **Wie lange nach der `in_folge`-ten Abweisung erneut gefragt wird**, in
/// Mikrosekunden: nach der ersten das Ziel einer Probe ([`PROBE_ZIEL_US`]),
/// danach das Doppelte, je mit einem Zufallsteil bis zur selben Länge,
/// damit die Messenden auseinanderlaufen. Häufiges Nachfragen trifft eher
/// eine Lücke; die Dauer bis zur Überschreitung setzt
/// [`abweisungsdauer`].
///
/// ⚠️ Gefragt wird bei der nächsten Aufnahme nach dem Abstand; mit einem
/// Takt von 30 s ist der Abstand mindestens das.
pub fn abstand_nach_abweisung(in_folge: u32, zufall: u64) -> u64 {
    let basis = PROBE_ZIEL_US << in_folge.saturating_sub(1).min(1);
    basis + zufall % basis
}

/// Was nach einer Abweisung folgt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NachAbweisung {
    /// Nach so vielen Mikrosekunden erneut fragen.
    Wiederholen(u64),
    /// Die Messung endet als Überschreitung: Ohne beantwortete Probe zählt
    /// sie, als wäre die Probe in ihre Grenze gelaufen
    /// ([`grenze_der_probe`]).
    Ueberschritten,
}

/// **Die Folge einer Abweisung** (Fund 660): die `in_folge`-te in Folge,
/// `seit_us` nach der ersten der Folge, bei `messende` Messenden im Pod.
pub fn nach_abweisung(in_folge: u32, seit_us: u64, messende: usize, zufall: u64) -> NachAbweisung {
    if in_folge >= ABWEISUNGEN_MINDESTENS && seit_us >= abweisungsdauer(messende) {
        NachAbweisung::Ueberschritten
    } else {
        NachAbweisung::Wiederholen(abstand_nach_abweisung(in_folge, zufall))
    }
}

/// **Was eine Messung je Token ergibt**: die kürzeste Probe abzüglich der
/// Zugabe für den Weg ([`ZUGABE_US`]), durch die Positionen der Probe,
/// abgerundet. Eine Probe unter der Zugabe ergibt null.
///
/// ⚑ **Damit gilt: bestanden, wenn die kürzeste Probe ≤ k · Frist + Zugabe.**
/// Die Zahl im Fristzeugnis ist diese Grösse; die Kette hält sie gegen die
/// Frist und rechnet sie nicht nach.
///
/// 📌 Bis zum 2026-10-27 stand hier Probe minus kürzester Ping, und eine
/// Probe, die nicht länger als der Ping dauerte, war keine Messung (Fund
/// 590). Beides entfällt: Ohne Ping gibt es nichts, was negativ werden
/// könnte, und nichts mehr zu verwerfen.
pub fn gemessen_je_token(probe_us: u64, positionen: u32) -> u64 {
    probe_us.saturating_sub(ZUGABE_US) / u64::from(positionen.max(1))
}

/// **Ab wann eine Probe die Frist überschritten hat**, in Mikrosekunden
/// seit sie gestellt wurde: die kleinste Dauer, für die
/// [`gemessen_je_token`] über `frist_us` liegt.
///
/// 📌 **Gefunden beim Bau (2026-10-27, Netzlauf): Wer langsam genug war,
/// entkam.** Die grosse Probe eines Lügners (rund 1 600 Positionen) lief
/// über die Zeitgrenze einer Anfrage im Netz (10 s), die Anfrage scheiterte,
/// und die Messung wurde verworfen statt gewertet. Wer diese Dauer
/// überschreitet, hat die Frist überschritten, gleich ob seine Antwort noch
/// kommt; die Messung endet hier mit der verstrichenen Zeit.
pub fn grenze_der_probe(frist_us: u64, positionen: u32) -> u64 {
    ZUGABE_US.saturating_add(u64::from(positionen.max(1)).saturating_mul(frist_us.saturating_add(1)))
}

/// **Die Zeitprobe für ein Mitglied**: seine Positionen und seine Grenze,
/// aufgerundet auf Millisekunden, damit der Shard dort aufhört.
pub fn probe(frist_us: u64, positionen: u32, saat: [u8; 32]) -> crate::nachschub::Nachforderung {
    let grenze_ms = grenze_der_probe(frist_us, positionen).div_ceil(1_000);
    crate::nachschub::Nachforderung::ZeitprobeMitSaat {
        positionen,
        grenze_ms: u32::try_from(grenze_ms).unwrap_or(u32::MAX),
        saat,
    }
}

/// **Trägt die Antwort einer Zeitprobe?** Die Unterschrift unter `schluessel`
/// über Saat, Positionen und Abdruck (Fund 661).
pub fn antwort_traegt(
    schluessel: &myl_types::bls::BlsPublicKey,
    saat: &[u8; 32],
    positionen: u32,
    abdruck: &[u8; 32],
    signatur: &myl_types::bls::BlsSignature,
) -> bool {
    schluessel.verify(&myl_types::zeitprobe::antwortnachricht(saat, positionen, abdruck), signatur)
}

/// ⚑ **Eine frische Saat je Probe** (2026-10-28, Fund 669): 32 Bytes, für das
/// gemessene Mitglied nicht vorhersehbar, aus `RandomState` der
/// Standardbibliothek, die das Betriebssystem schlüsselt.
pub fn frische_saat() -> [u8; 32] {
    use std::hash::BuildHasher;
    let zufall = std::collections::hash_map::RandomState::new();
    let mut saat = [0u8; 32];
    for (i, teil) in saat.chunks_mut(8).enumerate() {
        teil.copy_from_slice(&zufall.hash_one(i).to_le_bytes());
    }
    saat
}

/// **Ab welchem Teil einer Epoche gemessen wird**, in Vierteln: ab dem
/// ersten Viertel, gezählt in Blöcken.
///
/// 📌 **Fund 655 (2026-10-26, im langen Podlauf mit Tür und Störer): Ein
/// ehrlicher Miner wurde zweimal gesperrt.** Beide Male lagen drei
/// Messungen in den ersten 25 s einer Epoche, 14 bis 25 s nach dem Start
/// seines Shards, der zugleich schon für die Tür rechnete: 130 bis 394 ms
/// bei einer Frist von 100 ms, sonst 2 bis 14 ms. Die Zeitprobe mass den
/// Anlauf mit. Jetzt misst niemand vor dem ersten Viertel der Epoche
/// (Festlegung des Projektinhabers: Anlaufzeit und zwei Fenster).
pub const ANLAUF_TEIL: u64 = 4;

/// **Wie weit zwei Messfenster auseinanderliegen müssen**, als Teil der
/// Epoche: ein Achtel, gezählt in Blöcken.
pub const FENSTERABSTAND_TEIL: u64 = 8;

/// Darf auf dieser Höhe gemessen werden? Erst nach der Anlaufzeit.
pub fn darf_messen(hoehe: u64) -> bool {
    let e = myl_consensus::block::BLOECKE_JE_EPOCHE;
    hoehe % e >= e / ANLAUF_TEIL
}

/// Was aus einer Messung folgt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urteil {
    /// Unter der Frist: für diese Epoche erledigt.
    Bestanden,
    /// Über der Frist, aber das erste Fenster: noch kein Zeugnis, später
    /// wird nachgemessen.
    Verdacht,
    /// Zum zweiten Mal über der Frist, in einem getrennten Fenster:
    /// bezeugt wird der kleinere Wert.
    Zeugnis(u64),
}

/// **Das Urteil über eine Messung** (Fund 655, zwei Fenster). `verdacht`
/// ist die erste Überschreitung dieser Epoche, mit Höhe und Wert.
///
/// ⚑ **Eine Überschreitung allein bezeugt nichts.** Anlauf, Last und
/// Netz verlängern eine Probe für Sekunden; ein Lügner aber liegt immer
/// darüber, auch ein Achtel Epoche später. Liegt die zweite Messung zu
/// früh, bleibt es beim Verdacht.
pub fn urteil(gemessen: u64, frist: u64, hoehe: u64, verdacht: Option<(u64, u64)>) -> Urteil {
    if gemessen <= frist {
        return Urteil::Bestanden;
    }
    let abstand = myl_consensus::block::BLOECKE_JE_EPOCHE / FENSTERABSTAND_TEIL;
    match verdacht {
        Some((h0, g0)) if hoehe >= h0.saturating_add(abstand) => Urteil::Zeugnis(g0.min(gemessen)),
        _ => Urteil::Verdacht,
    }
}

/// Ist das zweite Fenster nach einem Verdacht auf Höhe `h0` offen?
pub fn zweites_fenster_offen(hoehe: u64, h0: u64) -> bool {
    hoehe >= h0.saturating_add(myl_consensus::block::BLOECKE_JE_EPOCHE / FENSTERABSTAND_TEIL)
}

/// Die Mitglieder eines Pods, die eine Position haben, ohne `ich`: wen
/// dieser Knoten misst.
pub fn zu_messen(zustand: &LedgerState, ich: &MinerId) -> Vec<(u32, myl_types::miner::MinerRegistration)> {
    let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
    let Some(pod) = zuteilung.pods.iter().find(|p| p.mitglieder().any(|m| m.miner_id == *ich)) else {
        return Vec::new();
    };
    pod.shards.iter().filter(|s| s.miner.miner_id != *ich).map(|s| (pod.pod_index, s.miner)).collect()
}

/// **Wer diesen Knoten misst**: alle anderen Mitglieder seines Pods, die
/// Reserve eingeschlossen (sie rechnet nicht und kann gerade deshalb
/// bezeugen). Nur ihnen beantwortet er eine Zeitprobe (Fund 609).
pub fn messende(zustand: &LedgerState, ich: &MinerId) -> Vec<myl_types::miner::MinerRegistration> {
    let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
    let Some(pod) = zuteilung.pods.iter().find(|p| p.mitglieder().any(|m| m.miner_id == *ich)) else {
        return Vec::new();
    };
    pod.mitglieder().filter(|m| m.miner_id != *ich).copied().collect()
}

/// **Darf `von` diesem Knoten eine Zeitprobe stellen?** Nur, wenn es ein
/// anderes Mitglied seines Pods ist (Fund 609).
pub fn misst_mich(zustand: &LedgerState, ich: &MinerId, von: &libp2p::PeerId) -> bool {
    messende(zustand, ich)
        .iter()
        .any(|m| myl_net::peer_id_aus_bytes(&m.netzadresse).is_ok_and(|p| p == *von))
}

/// Der Schlüssel einer Meldung: Epoche, Pod, Gemeldeter.
type Meldungsschluessel = (EpochId, u32, MinerId);

/// **Sammelt Fristzeugnisse**, bis eine Meldung trägt.
#[derive(Debug, Default)]
pub struct Fristsammlung {
    zeugnisse: std::collections::BTreeMap<Meldungsschluessel, Vec<Fristzeugnis>>,
    eingereicht: std::collections::BTreeSet<Meldungsschluessel>,
}

impl Fristsammlung {
    /// **Nimmt ein Zeugnis auf** und liefert die Meldung, sobald sie trägt
    /// und noch nicht eingereicht wurde.
    ///
    /// Geprüft wird vor der Aufnahme, was die Kette prüfen wird: die
    /// laufende Epoche, die Unterschrift unter dem registrierten Schlüssel,
    /// ein Zeuge aus dem Pod und nicht der Gemeldete, keiner doppelt. Ein
    /// Zeugnis, das die Kette verwürfe, belegt hier keinen Platz.
    pub fn aufnehmen(
        &mut self,
        zustand: &LedgerState,
        epoche: EpochId,
        pod: u32,
        gemeldeter: MinerId,
        zeugnis: Fristzeugnis,
    ) -> Option<Fristmeldung> {
        if epoche != zustand.epoch {
            return None;
        }
        let jetzt = zustand.epoch;
        self.zeugnisse.retain(|(e, _, _), _| *e == jetzt);
        self.eingereicht.retain(|(e, _, _)| *e == jetzt);
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
        let p = zuteilung.pods.iter().find(|x| x.pod_index == pod)?;
        let mitglieder: Vec<MinerId> = p.mitglieder().map(|m| m.miner_id).collect();
        if zeugnis.zeuge == gemeldeter || !mitglieder.contains(&zeugnis.zeuge) {
            return None;
        }
        let reg = zustand.miner.get(&zeugnis.zeuge)?;
        let probe = Fristmeldung { epoche, pod, gemeldeter, zeugnisse: Vec::new() };
        if !zeugnis.ist_vom_zeugen(&reg.schluessel, &probe) {
            return None;
        }
        let frist = frist_des_mitglieds(zustand, pod, &gemeldeter)?;
        let schluessel = (epoche, pod, gemeldeter);
        let liste = self.zeugnisse.entry(schluessel).or_default();
        if liste.iter().any(|z| z.zeuge == zeugnis.zeuge) {
            return None;
        }
        liste.push(zeugnis);
        let ueber: Vec<Fristzeugnis> = liste.iter().filter(|z| z.gemessen_us > frist).cloned().collect();
        if ueber.len() < myl_types::gegenzeichnung::mindestzeichner(mitglieder.len())
            || self.eingereicht.contains(&schluessel)
        {
            return None;
        }
        self.eingereicht.insert(schluessel);
        Some(Fristmeldung { epoche, pod, gemeldeter, zeugnisse: ueber })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_consensus::block::BLOECKE_JE_EPOCHE;

    /// Fund 669/661: Nur die Unterschrift des Mitglieds über genau diese
    /// Saat, diese Positionen und diesen Abdruck trägt; die Saat ist je
    /// Probe neu.
    #[test]
    fn nur_die_unterschrift_ueber_die_eigene_saat_traegt() {
        let k = crate::schluessel::Konsensschluessel::probe("alpha").expect("Schlüssel");
        let fremd = crate::schluessel::Konsensschluessel::probe("beta").expect("Schlüssel");
        let saat = frische_saat();
        assert_ne!(saat, frische_saat(), "je Probe eine neue Saat");
        let abdruck = [9u8; 32];
        let nachricht = myl_types::zeitprobe::antwortnachricht(&saat, 64, &abdruck);
        let sig = k.signiere(&nachricht).expect("signieren");
        assert!(antwort_traegt(&k.oeffentlich(), &saat, 64, &abdruck, &sig));
        // Gegenproben: andere Saat, andere Positionen, anderer Abdruck,
        // anderer Schlüssel.
        assert!(!antwort_traegt(&k.oeffentlich(), &frische_saat(), 64, &abdruck, &sig));
        assert!(!antwort_traegt(&k.oeffentlich(), &saat, 65, &abdruck, &sig));
        assert!(!antwort_traegt(&k.oeffentlich(), &saat, 64, &[8u8; 32], &sig));
        assert!(!antwort_traegt(&fremd.oeffentlich(), &saat, 64, &abdruck, &sig));
        assert!(matches!(probe(1_000, 64, saat), crate::nachschub::Nachforderung::ZeitprobeMitSaat { saat: s, .. } if s == saat));
    }

    #[test]
    fn die_messung_zieht_die_zugabe_ab_und_rundet_ab() {
        assert_eq!(gemessen_je_token(ZUGABE_US + 8_000, 4), 2_000);
        assert_eq!(gemessen_je_token(ZUGABE_US + 8_003, 4), 2_000, "abgerundet, zugunsten des Gemessenen");
        assert_eq!(gemessen_je_token(ZUGABE_US, 4), 0, "die Zugabe allein ist keine Rechnung");
        assert_eq!(gemessen_je_token(0, 4), 0);
        assert_eq!(gemessen_je_token(ZUGABE_US + 900, 0), 900, "null Positionen zählt als eine");
    }

    /// ⚑ **Fund 655: Anlaufzeit und zwei Fenster.** Vor dem ersten Viertel
    /// misst niemand; eine Überschreitung ist ein Verdacht; erst eine zweite,
    /// ein Achtel Epoche später, ergibt ein Zeugnis über den kleineren Wert;
    /// eine zweite Messung unter der Frist erledigt den Verdacht.
    #[test]
    fn erst_nach_dem_anlauf_und_in_zwei_fenstern() {
        let e = BLOECKE_JE_EPOCHE;
        let abstand = e / FENSTERABSTAND_TEIL;
        // Die Anlaufzeit: in jeder Epoche dasselbe Viertel.
        assert!(!darf_messen(0) && !darf_messen(e / 4 - 1) && !darf_messen(7 * e + 3));
        assert!(darf_messen(e / 4) && darf_messen(7 * e + e / 2));
        // Ein ehrlicher Miner im Anlauf: einmal über der Frist, danach darunter.
        let h0 = 3 * e + e / 4;
        assert_eq!(urteil(394_000, 100_000, h0, None), Urteil::Verdacht);
        assert_eq!(urteil(9_000, 100_000, h0 + abstand, Some((h0, 394_000))), Urteil::Bestanden);
        // Zu früh nachgemessen: noch kein Zeugnis.
        assert!(!zweites_fenster_offen(h0 + abstand - 1, h0));
        assert_eq!(urteil(300_000, 100_000, h0 + abstand - 1, Some((h0, 394_000))), Urteil::Verdacht);
        // ⛔️ Gegenprobe: Der Lügner liegt immer darüber und wird bezeugt,
        // mit dem kleineren der beiden Werte.
        assert!(zweites_fenster_offen(h0 + abstand, h0));
        assert_eq!(urteil(5_000, 1, h0 + abstand, Some((h0, 7_000))), Urteil::Zeugnis(5_000));
        assert_eq!(urteil(9_000, 1, h0 + abstand, Some((h0, 7_000))), Urteil::Zeugnis(7_000));
    }

    /// ⚑ **Fund 646: Wer seine Pongs zurückhält, entkommt nicht mehr**, und
    /// die Probe richtet sich nach der Zusage. Ein Lügner sagt 50 µs je
    /// Token an (Frist 100 µs) und rechnet 8 ms; sein Weg ist 300 ms.
    #[test]
    fn der_luegner_bekommt_die_groessere_probe() {
        let frist_luegner = 100;
        let k = probe_positionen(frist_luegner);
        assert_eq!(k, PROBE_HOECHSTENS, "die Lüge vergrössert die Probe");
        let probe = 300_000 + u64::from(k) * 8_000;
        assert!(gemessen_je_token(probe, k) > frist_luegner, "der Lügner liegt über der Frist");
        // ⛔️ Gegenprobe 1: Mit 64 Positionen bestand er.
        let probe_64 = 300_000 + 64 * 8_000;
        assert!(gemessen_je_token(probe_64, 64) <= frist_luegner, "mit fester Grösse 64 käme er durch");
        // ⛔️ Gegenprobe 2: Mit der alten Regel (vier Positionen, Probe minus
        //    kürzester Ping) hielt er jeden Pong 5 s zurück und war unmessbar.
        let alte_probe: u64 = 300_000 + 4 * 8_000;
        assert!(alte_probe.checked_sub(300_000 + 5_000_000).filter(|r| *r > 0).is_none());
        // An der Obergrenze ist die Auflösung Zugabe/2048, rund 1 ms: 0,5 ms
        // gerechnet besteht, 1,5 ms nicht.
        assert!(gemessen_je_token(300_000 + u64::from(k) * 500, k) <= frist_luegner);
        assert!(gemessen_je_token(300_000 + u64::from(k) * 1_500, k) > frist_luegner);
        // Unterhalb der Obergrenze ist die Schranke relativ: Wer 4 ms ansagt
        // (Frist 8 ms), besteht mit dem 2,75-Fachen und fällt mit dem
        // 3,25-Fachen auf.
        let k8 = probe_positionen(8_000);
        assert_eq!(k8, 500);
        assert!(gemessen_je_token(u64::from(k8) * 11_000, k8) <= 8_000);
        assert!(gemessen_je_token(u64::from(k8) * 13_000, k8) > 8_000);
    }

    /// 📌 Wer die Grenze überschreitet, liegt über der Frist, eine
    /// Mikrosekunde früher nicht.
    #[test]
    fn die_grenze_der_probe_ist_die_frist() {
        for (frist, k) in [(100u64, 2_048u32), (16_000, 250), (100_000, 64)] {
            let g = grenze_der_probe(frist, k);
            assert!(gemessen_je_token(g, k) > frist, "Frist {frist}, k {k}");
            assert!(gemessen_je_token(g - 1, k) <= frist, "Frist {frist}, k {k}");
        }
        // Der Lügner des Netzlaufs (25 Ebenen, Frist 2,5 ms): Grenze unter
        // der Zeitgrenze einer Anfrage, also gewertet, bevor sie scheitert.
        let k = probe_positionen(2_500);
        assert!(grenze_der_probe(2_500, k) < 10_000_000, "{}", grenze_der_probe(2_500, k));
    }

    /// Ein Ehrlicher zahlt je Probe ungefähr das Ziel, gleich wie schnell
    /// er ist, und besteht auch mit 3 s Weg.
    #[test]
    fn ein_ehrlicher_besteht_mit_langem_weg() {
        for zusage in [1_000u64, 8_000, 50_000] {
            let frist = zusage * myl_types::zusage::FRIST_FAKTOR;
            let k = probe_positionen(frist);
            let rechnung = u64::from(k) * zusage;
            assert!(rechnung <= 2 * PROBE_ZIEL_US || k == PROBE_MINDESTENS, "Zusage {zusage}: {rechnung} µs Rechnung");
            let probe = 3_000_000 + rechnung;
            assert!(gemessen_je_token(probe, k) <= frist, "Zusage {zusage} µs, k = {k}: ehrlich gemeldet");
        }
        assert_eq!(probe_positionen(u64::MAX), PROBE_MINDESTENS);
        assert_eq!(probe_positionen(0), PROBE_HOECHSTENS);
        assert_eq!(gemessen_je_token(ZUGABE_US - 1, 64), 0, "unter der Zugabe ist nichts gemessen");
    }

    /// ⚑ **Die kürzeste Probe trägt einen Verlust** (Fund 639): Die einzelne
    /// Probe mit Rückstau hätte einen Ehrlichen angeschuldigt, die kürzeste
    /// von fünf nicht.
    #[test]
    fn die_kuerzeste_probe_traegt_einen_verlust() {
        let frist_ehrlich = 80_000;
        let k = probe_positionen(frist_ehrlich);
        // Weg 100 ms, Rechnung 40 ms je Token; eine Probe trifft einen
        // Verlust (+200 ms Neuübertragung) und einen Rückstau (+6 s).
        let gut = 100_000 + u64::from(k) * 40_000;
        let proben = [gut + 6_000_000, gut + 4_000, gut + 9_500, gut + 200_000, gut + 7_900];
        assert_eq!(proben.len() as u32, PROBEN_JE_MESSUNG);
        let einzeln = gemessen_je_token(proben[0], k);
        assert!(einzeln > frist_ehrlich, "die alte Messung mit einer Probe schuldigte an: {einzeln}");
        let kuerzeste = gemessen_je_token(*proben.iter().min().unwrap(), k);
        assert!(kuerzeste <= frist_ehrlich, "{kuerzeste}");
    }

    /// Ein Pod mit einem Platz für Zeitproben beim Gemessenen: `messende`
    /// fragen ihn, jeder zu einem Zeitpunkt bis `streuung_us` nach dem
    /// Beginn; jede Probe belegt den Platz `belegt_us`, der Weg hin und
    /// zurück kostet `weg_us`. Abweisungen folgen [`nach_abweisung`], nur
    /// die Dauer bis zur Überschreitung ist `dauer_us`. Liefert je
    /// Messendem, ob seine Messung als Überschreitung endete.
    ///
    /// `frueh_fertig`: Der Gemessene hält die Frist, die Messung endet nach
    /// der ersten Antwort ([`messung_fertig`]); sonst nach fünf.
    /// `verweigert`: Er antwortet nie.
    #[allow(clippy::too_many_arguments)]
    fn gedraenge(
        messende: usize,
        streuung_us: u64,
        belegt_us: u64,
        weg_us: u64,
        saat: u64,
        frueh_fertig: bool,
        dauer_us: u64,
        verweigert: bool,
    ) -> Vec<bool> {
        let mut zufall = saat | 1;
        let mut naechster = move || {
            zufall ^= zufall << 13;
            zufall ^= zufall >> 7;
            zufall ^= zufall << 17;
            zufall
        };
        // Je Messendem: wann er fragt, Abweisungen in Folge seit wann,
        // Antworten, Ende.
        let mut senden: Vec<Option<u64>> = (0..messende).map(|_| Some(naechster() % streuung_us)).collect();
        let mut in_folge = vec![0u32; messende];
        let mut erste = vec![0u64; messende];
        let mut proben = vec![0u32; messende];
        let mut ueber = vec![false; messende];
        let mut frei_ab = 0u64;
        while let Some((i, t)) = senden.iter().enumerate().filter_map(|(i, s)| s.map(|t| (i, t))).min_by_key(|(_, t)| *t) {
            let an = t + weg_us / 2;
            if verweigert || an < frei_ab {
                in_folge[i] += 1;
                if in_folge[i] == 1 {
                    erste[i] = t;
                }
                senden[i] = if in_folge[i] >= ABWEISUNGEN_MINDESTENS && t - erste[i] >= dauer_us {
                    ueber[i] = true;
                    None
                } else {
                    Some(t + weg_us + abstand_nach_abweisung(in_folge[i], naechster()))
                };
                continue;
            }
            frei_ab = an + belegt_us;
            in_folge[i] = 0;
            proben[i] += 1;
            senden[i] = (!frueh_fertig && proben[i] < PROBEN_JE_MESSUNG).then_some(frei_ab + weg_us / 2);
        }
        ueber
    }

    /// ⚑ **Ehrliches Gedränge ist keine Verweigerung** (Fund 660, Gegenprobe
    /// der Festlegung vom 2026-10-28): Vier bis 23 Messende fragen denselben
    /// Ehrlichen, alle innerhalb von 2 s oder von 30 s; eine Probe belegt
    /// seinen Platz 2 s, der Weg kostet 50 bis 600 ms. Keine Runde endet mit
    /// einer Überschreitung.
    #[test]
    fn ehrliches_gedraenge_ist_keine_verweigerung() {
        let runden = |dauer: &dyn Fn(usize) -> u64, frueh: bool| {
            let mut falsch = 0usize;
            for messende in [4usize, 5, 7, 11, 23] {
                for streuung in [2_000_000u64, 30_000_000] {
                    for weg in [50_000u64, 300_000, 600_000] {
                        for saat in 1..=200u64 {
                            let ueber = gedraenge(messende, streuung, PROBE_ZIEL_US, weg, saat, frueh, dauer(messende), false);
                            falsch += usize::from(ueber.contains(&true));
                        }
                    }
                }
            }
            falsch
        };
        assert_eq!(runden(&abweisungsdauer, true), 0);
        // ⛔️ Gegenproben: die einfache Dauer statt der dreifachen, oder fünf
        //    Proben je Messung statt des frühen Endes, und Gedränge zählt als
        //    Verweigerung.
        let einfach = |m: usize| m as u64 * PROBE_ZIEL_US;
        assert!(runden(&einfach, true) > 0, "einfache Dauer");
        assert!(runden(&abweisungsdauer, false) > 0, "ohne frühes Ende");
        // Ein Verweigerer erreicht bei jedem Messenden die Überschreitung.
        for messende in [4usize, 23] {
            let ueber = gedraenge(messende, 2_000_000, PROBE_ZIEL_US, 300_000, 7, true, abweisungsdauer(messende), true);
            assert!(ueber.iter().all(|u| *u), "{messende} Messende");
        }
    }

    /// ⚑ **Fund 660: Eine Verweigerung zählt wie eine Überschreitung**,
    /// erst nach vier Abweisungen in Folge über die ganze Dauer, und dann
    /// mit denselben zwei Fenstern wie eine gemessene.
    #[test]
    fn eine_lange_verweigerung_ist_eine_ueberschreitung() {
        let d = abweisungsdauer(5);
        assert_eq!(d, 30_000_000);
        assert_eq!(nach_abweisung(1, 0, 5, 0), NachAbweisung::Wiederholen(PROBE_ZIEL_US));
        assert_eq!(nach_abweisung(2, 0, 5, 0), NachAbweisung::Wiederholen(2 * PROBE_ZIEL_US));
        assert_eq!(nach_abweisung(9, 0, 5, 0), NachAbweisung::Wiederholen(2 * PROBE_ZIEL_US), "gedeckelt");
        assert_eq!(nach_abweisung(ABWEISUNGEN_MINDESTENS, d, 5, 0), NachAbweisung::Ueberschritten);
        // ⛔️ Gegenproben: eine zu wenig, oder eine Mikrosekunde zu kurz.
        assert!(matches!(nach_abweisung(ABWEISUNGEN_MINDESTENS - 1, d, 5, 0), NachAbweisung::Wiederholen(_)));
        assert!(matches!(nach_abweisung(ABWEISUNGEN_MINDESTENS, d - 1, 5, 0), NachAbweisung::Wiederholen(_)));
        // Der Zufallsteil bleibt unter der Basis.
        assert_eq!(abstand_nach_abweisung(1, PROBE_ZIEL_US - 1), 2 * PROBE_ZIEL_US - 1);
        assert_eq!(abstand_nach_abweisung(1, PROBE_ZIEL_US), PROBE_ZIEL_US);
        // Ohne beantwortete Probe gemessen wie die Grenze, also über der
        // Frist, und im ersten Fenster ein Verdacht.
        let (frist, k) = (8_000u64, probe_positionen(8_000));
        let gilt = gemessen_je_token(grenze_der_probe(frist, k), k);
        assert!(gilt > frist);
        let h0 = BLOECKE_JE_EPOCHE / 4;
        assert_eq!(urteil(gilt, frist, h0, None), Urteil::Verdacht);
        assert_eq!(urteil(gilt, frist, h0 + BLOECKE_JE_EPOCHE / 8, Some((h0, gilt))), Urteil::Zeugnis(gilt));
    }

    /// Das frühe Ende ändert kein Urteil: Wer die Frist schon hält, bleibt
    /// darunter; wer darüber liegt, bekommt alle fünf Proben.
    #[test]
    fn eine_bestandene_probe_beendet_die_messung() {
        let (frist, k) = (8_000u64, probe_positionen(8_000));
        let grenze = grenze_der_probe(frist, k);
        assert!(messung_fertig(grenze - 1, 1, frist, k), "unter der Grenze: bestanden");
        assert!(!messung_fertig(grenze, 1, frist, k), "an der Grenze: weiter messen");
        assert!(!messung_fertig(grenze, PROBEN_JE_MESSUNG - 1, frist, k));
        assert!(messung_fertig(grenze, PROBEN_JE_MESSUNG, frist, k), "nach fünf immer");
    }

    /// Eine Probekette mit einem Pod aus sechs Probekonten in Epoche 1.
    fn kette_mit_pod() -> crate::kette::Kette {
        let mut k = crate::kette::Kette::probestand();
        for w in 0..6u8 {
            k.aufnehmen(crate::kette::probeanmeldung(w));
            k.aufnehmen(crate::kette::probezusage(w, EpochId(1), 1));
        }
        k.baue_block();
        while k.zustand().epoch.0 < 3 {
            k.baue_block();
        }
        let _ = BLOECKE_JE_EPOCHE;
        k
    }

    /// ⚑ **Eine Zeitprobe beantwortet nur, wer zum eigenen Pod gehört**
    /// (Fund 609): jedes andere Mitglied ja, die Reserve eingeschlossen,
    /// ein Fremder nicht, und ein Knoten ohne Pod niemandem.
    #[test]
    fn eine_zeitprobe_stellen_nur_die_mitglieder_des_eigenen_pods() {
        let k = kette_mit_pod();
        let mut z = k.zustand().clone();
        // Eigene Netzadressen statt der Null der Probeanmeldung, sonst
        // waeren alle dieselbe Gegenstelle.
        let ids: Vec<MinerId> = z.miner.keys().copied().collect();
        for (i, id) in ids.iter().enumerate() {
            let schluessel = libp2p::identity::Keypair::ed25519_from_bytes([i as u8 + 1; 32]).expect("Schluessel");
            let roh = schluessel.public().try_into_ed25519().expect("ed25519").to_bytes();
            if let Some(m) = z.miner.get_mut(id) {
                m.netzadresse = myl_types::latency_attest::PeerIdBytes(roh);
            }
        }
        let peer = |m: &myl_types::miner::MinerRegistration| myl_net::peer_id_aus_bytes(&m.netzadresse).expect("PeerId");
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(&z);
        let pod = &zuteilung.pods[0];
        let ich = pod.shards[0].miner.miner_id;
        let mitglieder: Vec<_> = pod.mitglieder().copied().collect();
        assert!(mitglieder.len() >= 2);
        for m in mitglieder.iter().filter(|m| m.miner_id != ich) {
            let aktuell = z.miner[&m.miner_id];
            assert!(misst_mich(&z, &ich, &peer(&aktuell)), "ein Mitglied darf messen");
        }
        let eigen = z.miner[&ich];
        assert!(!misst_mich(&z, &ich, &peer(&eigen)), "sich selbst misst niemand");
        let fremd = libp2p::PeerId::random();
        assert!(!misst_mich(&z, &ich, &fremd), "ein Fremder darf keine Zeitprobe stellen");
        let ohne_pod = MinerId::new([0xEE; 32]);
        assert!(!misst_mich(&z, &ohne_pod, &peer(&eigen)), "ohne Pod beantwortet niemand eine Probe");
    }

    /// ⚑ **Vom Zeugnis zur Meldung**: Die Frist steht aus der Kette (300 µs
    /// je Ebene, doppelt); Zeugnisse unter der Frist zählen nicht, ein
    /// Fremder und eine falsche Unterschrift belegen keinen Platz, und
    /// sobald drei von fünf übrigen über der Frist liegen, kommt die
    /// Meldung, einmal.
    #[test]
    fn eine_mehrheit_ueber_der_frist_ergibt_eine_meldung() {
        let k = kette_mit_pod();
        let z = k.zustand();
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(z);
        let pod = &zuteilung.pods[0];
        let gemeldeter = pod.shards[1].miner.miner_id;
        let frist = frist_des_mitglieds(z, pod.pod_index, &gemeldeter).expect("Frist");
        let grenzen = crate::kette::Kette::grenzen_des_pods(z, pod).expect("Grenzen");
        assert_eq!(frist, 300 * (grenzen[1].1 - grenzen[1].0) as u64 * 2);
        // Wer zeugt: alle übrigen Mitglieder, mit ihrem Probeschlüssel.
        let nummer = |m: &MinerId| (0..6u8).find(|w| MinerId::new(*crate::kette::probekonto(*w).as_bytes()) == *m).unwrap();
        let zeugen: Vec<MinerId> = pod.mitglieder().map(|m| m.miner_id).filter(|m| *m != gemeldeter).collect();
        let zeugnis = |m: &MinerId, us: u64| {
            Fristzeugnis::neu(&crate::kette::probeschluessel(nummer(m)), *m, z.epoch, pod.pod_index, &gemeldeter, us)
                .expect("Zeugnis")
        };
        let mut s = Fristsammlung::default();
        let mut auf = |zg: Fristzeugnis| s.aufnehmen(z, z.epoch, pod.pod_index, gemeldeter, zg);
        assert!(auf(zeugnis(&zeugen[0], frist)).is_none(), "genau die Frist ist kein Verstoss");
        assert!(auf(zeugnis(&zeugen[1], frist + 1)).is_none());
        let mut falsch = zeugnis(&zeugen[2], frist + 9);
        falsch.gemessen_us += 1;
        assert!(auf(falsch).is_none(), "falsche Unterschrift");
        assert!(auf(zeugnis(&zeugen[2], frist + 2)).is_none(), "zwei ueber der Frist reichen nicht");
        let meldung = auf(zeugnis(&zeugen[3], frist + 3)).expect("drei von fuenf");
        assert_eq!(meldung.zeugnisse.len(), 3);
        assert!(auf(zeugnis(&zeugen[4], frist + 4)).is_none(), "einmal eingereicht");
        // Und die Kette nimmt sie an.
        let mut st = z.clone();
        let mitglieder: Vec<MinerId> = pod.mitglieder().map(|m| m.miner_id).collect();
        let kontext = myl_ledger::transitions::Fristkontext {
            mitglieder,
            ebenen: grenzen[1].1 - grenzen[1].0,
            mit_kopf: false,
        };
        let einreicher = myl_types::ids::Address::new(*meldung.zeugnisse[0].zeuge.as_bytes());
        myl_ledger::transitions::fristverstoss_einreichen(&mut st, &einreicher, &meldung, &kontext).expect("Kette nimmt an");
    }

    /// ⚑ Die Selbstprobe: Bis zum Anderthalbfachen der Zusage bleibt alles,
    /// darüber wird die Zusage mit einem Viertel Luft langsamer, nie
    /// schneller.
    #[test]
    fn die_selbstprobe_verlangsamt_erst_ueber_dem_anderthalbfachen() {
        // Zusage 10 ms je Token, Frist 20 ms.
        assert!(!selbstprobe_zu_langsam(15_000, 20_000), "genau anderthalb: noch gut");
        assert!(selbstprobe_zu_langsam(15_001, 20_000));
        // Doppelt so langsam wie zugesagt: Ebenenzeit 300 µs wird 750 µs.
        assert_eq!(angepasste_ebenenzeit(300, 20_000, 20_000), Some(750));
        // Schneller als zugesagt: keine Änderung.
        assert_eq!(angepasste_ebenenzeit(300, 5_000, 20_000), None);
        assert_eq!(angepasste_ebenenzeit(300, 5_000, 0), None);
    }
}
