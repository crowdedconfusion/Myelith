//! Slash-/Kopfgeld-Entscheidung — Whitepaper Kap. 5.5, 6.6, Anhang A.4.
//!
//! Bestimmt aus dem Bisektionsergebnis, **wer** verloren hat, und
//! übersetzt das in den Schiedsspruch, den der Ledger bucht.
//!
//! ## Warum hier keine Beträge mehr stehen (Fund A9)
//!
//! Bis v0.2.6 hatte dieses Modul eine eigene `SlashConfig` mit **festen
//! Beträgen** (1 MYL Slash, 0,5 MYL Kopfgeld). Das war ein zweites,
//! unvereinbares Slashing-Modell neben dem des Ledgers:
//!
//! - `myl_ledger::apply_verdict` schlachtet einen **Anteil des Stakes**
//!   (`SlashParams` als Zähler/Nenner-Paare) — so wie es Whitepaper
//!   Kap. 5.5 vorgibt (30–100 % des Stakes je nach Vergehen).
//! - `myl-verifier` rechnete mit absoluten Beträgen und hing nicht
//!   einmal an `myl-ledger`, konnte also gar nicht buchen.
//!
//! Ein fester Betrag hat zudem keine Abschreckungswirkung: 1 MYL ist
//! für einen Großstaker nichts, und die gesamte Sicherheitsannahme der
//! Verifikationsarchitektur (Kap. 6.9: Betrug muss teurer sein als der
//! erwartete Gewinn) hängt genau daran.
//!
//! ## ⚑ Und warum eine Schuldzuweisung einen Beleg braucht (2026-08-29)
//!
//! Bis dahin nahm [`create_slash_decision`] den zu schlachtenden Miner
//! als **Aufrufparameter** entgegen. Nichts band ihn an die strittige
//! Arbeit. Wer die Funktion rief, bestimmte, wen es trifft, und dieselbe
//! Gestalt hatte Fund 85 auf der Ledger-Seite: eine Anweisung, die den
//! Absender nannte, ohne ihn zu belegen.
//!
//! Der Beleg lag dabei die ganze Zeit vor: Jeder Shard unterschreibt
//! jeden Übergang, den er rechnet (`myl_types::uebergang::TransitionSig`).
//! Diese Unterschriften wurden erzeugt, eingesammelt, aggregiert und von
//! niemandem geprüft. Sie sind keine Eingabeprüfung, dafür ist der
//! Spur-Hash da; sie sind die **Zuschreibung**, und Zuschreibung ist
//! genau das, was eine Slash-Entscheidung braucht.
//!
//! Seitdem verlangt [`create_slash_decision`] für [`VerdictOutcome::PrimaryLoses`]
//! einen [`Schuldbeleg`] und weist ohne ihn ab.
//!
//! Dieses Modul entscheidet deshalb nur noch über **Schuld**, nicht über
//! Beträge. Die Beträge ergeben sich aus dem Stake und den
//! Governance-Parametern, wenn `myl_ledger::apply_verdict` den
//! Schiedsspruch bucht.
//!
//! **Konsens-Feld:** Die Slash-Logik ist Teil des Konsensvertrags.
//! Änderungen nur über Governance (Kap. 10.3).

use myl_ledger::transitions::{Verdict as LedgerVerdict, VerdictOutcome as LedgerOutcome};
use myl_types::bls::{BlsPublicKey, BlsSignature};
use myl_types::ids::{Address, MinerId, SegmentId};
use myl_types::uebergang::{Rolle, TransitionSig};

/// Ergebnis der Schiedsrunde (wer hat gewonnen/verloren).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictOutcome {
    /// Primärer Pod hat verloren (war fehlerhaft).
    PrimaryLoses,
    /// Redundanter Pod hat verloren (hat falsch challenget).
    RedundantLoses,
}

/// Eine Slash-Entscheidung: wer hat verloren, und warum.
///
/// Enthält bewusst **keine** Beträge — siehe Modul-Dokumentation.
/// Für die Buchung liefert [`Self::to_ledger_verdict`] den Schiedsspruch
/// im Ledger-Format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashDecision {
    /// Das strittige Segment.
    pub segment_id: SegmentId,
    /// Miner, der geslasht wird (Verlierer).
    pub slashed_miner: MinerId,
    /// Miner, der das Kopfgeld erhält (Gewinner).
    pub rewarded_miner: MinerId,
    /// Grund der Slash-Entscheidung.
    pub reason: SlashReason,
}

/// Grund der Slash-Entscheidung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashReason {
    /// Primärer Pod hat fehlerhafte Berechnung durchgeführt.
    PrimaryFault {
        /// Position der Abweichung.
        divergence_position: usize,
    },
    /// Redundanter Pod hat falsche Challenge eingereicht.
    RedundantFault,
}

/// Der Beleg, dass ein bestimmter Miner den strittigen Schritt selbst
/// gerechnet hat.
///
/// Er besteht aus dem unterschriebenen Übergang, dem öffentlichen
/// Schlüssel des Unterzeichners und seiner Signatur. Die Kennung wird
/// aus dem Schlüssel **abgeleitet** und nicht mitgeführt: Zwei Quellen
/// für dieselbe Wahrheit widersprechen sich irgendwann.
///
/// # Was er belegt und was nicht
///
/// **Belegt:** Der Inhaber dieses Schlüssels hat für dieses Segment
/// einen Übergang von `prev_hash` nach `next_hash` unterschrieben, in
/// der Rolle [`Rolle::Shard`] und in keiner anderen.
///
/// **Belegt nicht:** dass gerade die strittige *Layer* in seinem
/// Zuständigkeitsbereich lag. Die Signatur ist je Shard und
/// Token-Position, die Bisektion zeigt auf eine Layer-Position, und die
/// Zuordnung Layer zu Shard steht in der Layer-Spanne des Shards, die
/// die Signatur nicht mitführt. Sie einfach gleichzusetzen wäre eine
/// erfundene Prüfung, und eine erfundene Prüfung ist schlimmer als
/// keine, weil ein Leser sie für einen Schutz hält.
///
/// Was der Beleg trotzdem leistet: Geschlachtet werden kann nur noch,
/// wer an diesem Segment unter eigenem Schlüssel gearbeitet hat. Vorher
/// konnte jeder benannt werden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schuldbeleg {
    /// Der unterschriebene Übergang.
    pub uebergang: TransitionSig,
    /// Der öffentliche Schlüssel des Unterzeichners.
    pub schluessel: BlsPublicKey,
    /// Seine Signatur über den Übergang in der Rolle [`Rolle::Shard`].
    pub signatur: BlsSignature,
}

impl Schuldbeleg {
    /// Wen der Beleg belastet: die aus dem Schlüssel abgeleitete Kennung.
    pub fn unterzeichner(&self) -> MinerId {
        MinerId::aus_schluessel(&self.schluessel)
    }

    /// Prüft die Signatur, und zwar ausdrücklich in der Rolle
    /// [`Rolle::Shard`].
    ///
    /// Die Rolle mitzuprüfen ist der Sinn der Rollenbindung: Eine
    /// Unterschrift, die derselbe Miner als Pod-Mitglied oder Validator
    /// abgegeben hat, gilt hier nicht.
    pub fn ist_gueltig(&self) -> bool {
        self.uebergang
            .verify_mit_rolle(&self.schluessel, &self.signatur, Rolle::Shard)
    }
}

/// Fehler bei der Slash-Entscheidung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashError {
    /// Miner-IDs sind identisch (kein sinnvoller Slash).
    IdenticalMiners,
    /// Für den Schuldspruch fehlt der Beleg.
    ///
    /// ⚑ Kein Formfehler: Ohne Beleg ist die Beschuldigung eine
    /// Behauptung des Aufrufers.
    BelegFehlt,
    /// Die Signatur des Belegs geht nicht auf.
    BelegUngueltig,
    /// Der Beleg gehört zu einem anderen Unterzeichner als dem
    /// Beschuldigten.
    BelegAnderenUnterzeichners,
    /// Der Beleg gehört zu einem anderen Segment als dem strittigen.
    BelegAnderenSegments,
}

impl std::fmt::Display for SlashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdenticalMiners => write!(f, "Miner-IDs sind identisch"),
            Self::BelegFehlt => write!(
                f,
                "kein Beleg: die Beschuldigung ist eine Behauptung des Aufrufers"
            ),
            Self::BelegUngueltig => write!(f, "die Signatur des Belegs geht nicht auf"),
            Self::BelegAnderenUnterzeichners => {
                write!(f, "der Beleg gehört einem anderen Unterzeichner")
            }
            Self::BelegAnderenSegments => write!(f, "der Beleg gehört zu einem anderen Segment"),
        }
    }
}

impl std::error::Error for SlashError {}

impl SlashDecision {
    /// Übersetzt die Entscheidung in den Schiedsspruch, den
    /// `myl_ledger::apply_verdict` bucht.
    ///
    /// Der Ledger führt Konten unter `Address`
    /// (`Address = SHA-256(komprimierter BLS-Public-Key)`), die
    /// Verifikation arbeitet mit `MinerId`. Die Zuordnung ist ein
    /// Registry-Nachschlag, keine reine Umrechnung — deshalb werden die
    /// beiden Adressen übergeben, statt sie hier zu erraten.
    ///
    /// **Parameter:**
    /// - `slashed_addr`: Ledger-Adresse des Verlierers
    /// - `rewarded_addr`: Ledger-Adresse des Gewinners
    ///
    /// **Returns:** `Verdict` mit `outcome = SlashMiner`; der Verlierer
    /// steht im Feld `miner`, der Gewinner in `checker`. Der Ledger
    /// schlachtet damit den Stake des Verlierers und zahlt dem Gewinner
    /// das Kopfgeld — unabhängig davon, welche Rolle die beiden im
    /// Streit hatten.
    pub fn to_ledger_verdict(
        &self,
        slashed_addr: Address,
        rewarded_addr: Address,
    ) -> LedgerVerdict {
        LedgerVerdict {
            segment_id: self.segment_id,
            miner: slashed_addr,
            checker: rewarded_addr,
            outcome: LedgerOutcome::SlashMiner,
        }
    }
}

/// Erstellt eine Slash-Entscheidung basierend auf dem Verdict.
///
/// **Parameter:**
/// - `outcome`: Ergebnis der Schiedsrunde
/// - `segment_id`: das strittige Segment
/// - `primary_miner`: Miner des primären Pods
/// - `redundant_miner`: Miner des redundanten Pods
/// - `divergence_position`: Position der Abweichung (nur bei `PrimaryLoses`)
/// - `beleg`: der unterschriebene Übergang des Beschuldigten; **für
///   [`VerdictOutcome::PrimaryLoses`] erforderlich**
///
/// **Returns:** `SlashDecision` bei erfolgreicher Erstellung.
///
/// **Fehler:**
/// - [`SlashError::IdenticalMiners`], wenn beide Seiten derselbe Miner
///   sind; dann gibt es nichts zu entscheiden.
/// - [`SlashError::BelegFehlt`], [`SlashError::BelegUngueltig`],
///   [`SlashError::BelegAnderenUnterzeichners`],
///   [`SlashError::BelegAnderenSegments`] bei einem Schuldspruch gegen
///   den primären Pod ohne tragenden Beleg.
///
/// # ⚑ Warum der Beleg nur die eine Richtung deckt
///
/// Bei [`VerdictOutcome::PrimaryLoses`] wird der primäre Pod dafür
/// geschlachtet, dass er **falsch gerechnet** hat, und die Unterschrift
/// unter dem Übergang belegt, dass er gerechnet hat.
///
/// Bei [`VerdictOutcome::RedundantLoses`] wird der Herausforderer dafür
/// geschlachtet, dass er **falsch beschuldigt** hat. Der Beleg dafür
/// wäre eine unterschriebene Herausforderung, und
/// `myl_types::Challenge` trägt heute keine Signatur: Sie nennt beide
/// Miner als Felder, wie es hier zuvor die Parameter taten. Diese
/// Richtung ist also weiterhin unbelegt.
///
/// **Warum trotzdem jetzt und nicht beides auf einmal:** Für die eine
/// Richtung liegt der Beleg seit Monaten vor und wurde nur nicht
/// befragt; für die andere gibt es ihn nicht. Die vorhandene Prüfung
/// zurückzuhalten, bis die fehlende gebaut ist, ließe eine Tür offen,
/// die man heute schließen kann. Sich für die zweite Richtung einen
/// Beleg auszudenken, der nichts belegt, wäre schlimmer als sie offen zu
/// benennen.
pub fn create_slash_decision(
    outcome: VerdictOutcome,
    segment_id: SegmentId,
    primary_miner: MinerId,
    redundant_miner: MinerId,
    divergence_position: Option<usize>,
    beleg: Option<&Schuldbeleg>,
) -> Result<SlashDecision, SlashError> {
    if primary_miner == redundant_miner {
        return Err(SlashError::IdenticalMiners);
    }

    if outcome == VerdictOutcome::PrimaryLoses {
        let beleg = beleg.ok_or(SlashError::BelegFehlt)?;
        // Billig vor teuer: Zuordnung erst, Kryptografie zuletzt. Eine
        // Signaturprüfung als erste Hürde wäre eine Rechenlast, die
        // jeder mit einem falsch adressierten Beleg auslösen kann.
        if beleg.uebergang.segment_id != segment_id {
            return Err(SlashError::BelegAnderenSegments);
        }
        if beleg.unterzeichner() != primary_miner {
            return Err(SlashError::BelegAnderenUnterzeichners);
        }
        if !beleg.ist_gueltig() {
            return Err(SlashError::BelegUngueltig);
        }
    }

    let (slashed_miner, rewarded_miner, reason) = match outcome {
        VerdictOutcome::PrimaryLoses => (
            primary_miner,
            redundant_miner,
            SlashReason::PrimaryFault {
                divergence_position: divergence_position.unwrap_or(0),
            },
        ),
        VerdictOutcome::RedundantLoses => {
            (redundant_miner, primary_miner, SlashReason::RedundantFault)
        }
    };

    Ok(SlashDecision {
        segment_id,
        slashed_miner,
        rewarded_miner,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_ledger::state::LedgerState;
    use myl_ledger::transitions::{apply_verdict, SlashParams};

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    fn geheim(b: u8) -> myl_types::bls::BlsSecretKey {
        myl_types::bls::BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schlüssel")
    }

    /// Eine Kennung, die zu einem **wirklich vorhandenen** Schlüssel
    /// gehört. `miner(b)` tut das nicht, und genau darum geht es: Wer
    /// geschlachtet werden soll, muss unterschrieben haben.
    fn kennung(b: u8) -> MinerId {
        MinerId::aus_schluessel(&geheim(b).public_key().expect("Punkt"))
    }

    fn beleg_fuer(b: u8, seg: SegmentId) -> Schuldbeleg {
        let sk = geheim(b);
        let uebergang = TransitionSig {
            segment_id: seg,
            shard_index: 0,
            position: 0,
            prev_hash: [0u8; 32],
            next_hash: [7u8; 32],
        };
        let signatur = uebergang.sign(&sk).expect("signieren");
        Schuldbeleg {
            uebergang,
            schluessel: sk.public_key().expect("Punkt"),
            signatur,
        }
    }

    fn addr(b: u8) -> Address {
        Address::new([b; 32])
    }

    fn segment() -> SegmentId {
        SegmentId::new([1u8; 32])
    }

    #[test]
    fn primaerer_pod_verliert() {
        let d = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            segment(),
            kennung(1),
            kennung(2),
            Some(7),
            Some(&beleg_fuer(1, segment())),
        )
        .unwrap();

        assert_eq!(d.slashed_miner, kennung(1));
        assert_eq!(d.rewarded_miner, kennung(2));
        assert_eq!(
            d.reason,
            SlashReason::PrimaryFault {
                divergence_position: 7
            }
        );
    }

    #[test]
    fn redundanter_pod_verliert() {
        let d = create_slash_decision(
            VerdictOutcome::RedundantLoses,
            segment(),
            miner(1),
            miner(2),
            None,
            None,
        )
        .unwrap();

        assert_eq!(d.slashed_miner, miner(2));
        assert_eq!(d.rewarded_miner, miner(1));
        assert_eq!(d.reason, SlashReason::RedundantFault);
    }

    #[test]
    fn identische_miner_werden_abgelehnt() {
        assert_eq!(
            create_slash_decision(
                VerdictOutcome::PrimaryLoses,
                segment(),
                miner(1),
                miner(1),
                None,
                None,
            ),
            Err(SlashError::IdenticalMiners)
        );
    }

    /// Der Kern von Fund A9: Die Entscheidung muss beim Ledger ankommen.
    /// Vorher rechnete dieses Modul mit festen Beträgen und hing nicht
    /// einmal an `myl-ledger` — es konnte gar nicht buchen.
    #[test]
    fn entscheidung_wird_vom_ledger_gebucht() {
        let d = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            segment(),
            kennung(1),
            kennung(2),
            Some(3),
            Some(&beleg_fuer(1, segment())),
        )
        .unwrap();

        let mut state = LedgerState::genesis(1);
        state.account_mut(&addr(1)).staked = 1_000_000;

        let params = SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 2, // 50 % des Stakes
            bounty_fraction_num: 1,
            bounty_fraction_den: 10, // 10 % davon als Kopfgeld
        };

        let verdict = d.to_ledger_verdict(addr(1), addr(2));
        let effect = apply_verdict(&mut state, &verdict, &params).unwrap();

        assert_eq!(effect.slashed, 500_000);
        assert_eq!(effect.bounty, 50_000);
        assert_eq!(state.account(&addr(1)).staked, 500_000);
        assert_eq!(state.account(&addr(2)).balance, 50_000);
    }

    /// Der Slash ist ein **Anteil des Stakes** — ein Großstaker verliert
    /// entsprechend mehr. Mit dem alten Festbetrag (1 MYL) hätte er
    /// unabhängig von seiner Größe immer dasselbe verloren.
    #[test]
    fn slash_skaliert_mit_dem_stake() {
        let d = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            segment(),
            kennung(1),
            kennung(2),
            None,
            Some(&beleg_fuer(1, segment())),
        )
        .unwrap();
        let params = SlashParams {
            slash_fraction_num: 3,
            slash_fraction_den: 10, // 30 %, Whitepaper Kap. 5.5 untere Grenze
            bounty_fraction_num: 1,
            bounty_fraction_den: 10,
        };

        let mut klein = LedgerState::genesis(1);
        klein.account_mut(&addr(1)).staked = 10_000_000;
        let e_klein =
            apply_verdict(&mut klein, &d.to_ledger_verdict(addr(1), addr(2)), &params).unwrap();

        let mut gross = LedgerState::genesis(1);
        gross.account_mut(&addr(1)).staked = 10_000_000_000;
        let e_gross =
            apply_verdict(&mut gross, &d.to_ledger_verdict(addr(1), addr(2)), &params).unwrap();

        assert_eq!(e_klein.slashed, 3_000_000);
        assert_eq!(e_gross.slashed, 3_000_000_000);
        assert!(e_gross.slashed > e_klein.slashed);
    }

    // ── Der Beleg (⚑ Fund 85 an zweiter Stelle) ─────────────────────

    /// ⚑ **Ohne Beleg kein Schuldspruch.**
    ///
    /// Das ist der ganze Punkt: Bis zum 2026-08-29 kam genau dieser
    /// Aufruf durch, und der Aufrufer bestimmte allein, wen es trifft.
    #[test]
    fn ohne_beleg_wird_niemand_geschlachtet() {
        assert_eq!(
            create_slash_decision(
                VerdictOutcome::PrimaryLoses,
                segment(),
                kennung(1),
                kennung(2),
                Some(7),
                None,
            ),
            Err(SlashError::BelegFehlt)
        );
    }

    /// Der Beleg eines anderen belastet nicht den Beschuldigten.
    ///
    /// Ohne diese Prüfung genügte **irgendein** gültiger Beleg, und
    /// gültige Belege hat jeder ehrliche Shard massenhaft erzeugt.
    #[test]
    fn ein_fremder_beleg_belastet_nicht() {
        assert_eq!(
            create_slash_decision(
                VerdictOutcome::PrimaryLoses,
                segment(),
                kennung(1),
                kennung(2),
                Some(7),
                Some(&beleg_fuer(2, segment())),
            ),
            Err(SlashError::BelegAnderenUnterzeichners)
        );
    }

    /// Ein Beleg aus einem anderen Segment gilt hier nicht.
    ///
    /// Sonst ließe sich die ehrliche Arbeit eines Miners an Segment A
    /// als Schuldbeleg für Segment B einsetzen.
    #[test]
    fn ein_beleg_aus_einem_anderen_segment_gilt_nicht() {
        let anderes = SegmentId::new([9u8; 32]);
        assert_eq!(
            create_slash_decision(
                VerdictOutcome::PrimaryLoses,
                segment(),
                kennung(1),
                kennung(2),
                Some(7),
                Some(&beleg_fuer(1, anderes)),
            ),
            Err(SlashError::BelegAnderenSegments)
        );
    }

    /// Eine gefälschte Unterschrift trägt nicht.
    #[test]
    fn ein_beleg_mit_falscher_unterschrift_traegt_nicht() {
        let mut b = beleg_fuer(1, segment());
        // Der Inhalt wird nach dem Unterschreiben verändert.
        b.uebergang.next_hash = [8u8; 32];
        assert_eq!(
            create_slash_decision(
                VerdictOutcome::PrimaryLoses,
                segment(),
                kennung(1),
                kennung(2),
                Some(7),
                Some(&b),
            ),
            Err(SlashError::BelegUngueltig)
        );
    }

    /// ⚑ **Eine Unterschrift aus einer anderen Rolle gilt nicht.**
    ///
    /// Ein Miner benutzt denselben Schlüssel als Shard, als
    /// Pod-Mitglied und möglicherweise als Validator. Ohne Rollenbindung
    /// ließe sich seine Zustimmung zu einem PoI-Bündel als Geständnis
    /// über einen Rechenschritt einsetzen.
    #[test]
    fn eine_unterschrift_aus_anderer_rolle_gilt_nicht() {
        let sk = geheim(1);
        let uebergang = TransitionSig {
            segment_id: segment(),
            shard_index: 0,
            position: 0,
            prev_hash: [0u8; 32],
            next_hash: [7u8; 32],
        };
        let b = Schuldbeleg {
            signatur: sk
                .sign(&uebergang.to_sign_bytes_mit_rolle(Rolle::PodMitglied))
                .expect("signieren"),
            uebergang,
            schluessel: sk.public_key().expect("Punkt"),
        };
        assert_eq!(
            create_slash_decision(
                VerdictOutcome::PrimaryLoses,
                segment(),
                kennung(1),
                kennung(2),
                Some(7),
                Some(&b),
            ),
            Err(SlashError::BelegUngueltig)
        );
    }

    /// Die Kennung wird aus dem Schlüssel abgeleitet, nicht mitgeführt.
    ///
    /// Gegenprobe zur Ableitung in `myl_types`: Sie wird hier
    /// ausgeschrieben und nicht über denselben Helfer gerechnet, sonst
    /// prüfte der Test sich selbst.
    #[test]
    fn der_beleg_leitet_die_kennung_aus_dem_schluessel_ab() {
        let b = beleg_fuer(3, segment());
        let erwartet = myl_types::hash::Hash::sha256(&b.schluessel.0);
        assert_eq!(b.unterzeichner().as_bytes(), erwartet.as_bytes());
    }

    #[test]
    fn ledger_verdict_traegt_die_segment_id() {
        let d = create_slash_decision(
            VerdictOutcome::PrimaryLoses,
            segment(),
            kennung(1),
            kennung(2),
            None,
            Some(&beleg_fuer(1, segment())),
        )
        .unwrap();
        let v = d.to_ledger_verdict(addr(1), addr(2));
        assert_eq!(v.segment_id, segment());
        assert_eq!(v.miner, addr(1));
        assert_eq!(v.checker, addr(2));
    }
}
