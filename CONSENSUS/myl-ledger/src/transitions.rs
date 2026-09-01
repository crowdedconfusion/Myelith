//! Zustandsübergänge des Ledgers (Anhang A.5).
//!
//! Jeder Übergang ist eine reine Funktion `(State, …) → State` ohne
//! versteckten globalen Zustand; Fehler lassen den Zustand unverändert
//! (Übergänge prüfen zuerst und ändern erst dann).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::gegenstand::{Ablage, Manifest};
use myl_types::ids::{Address, EpochId, MerkleRoot, MinerId, SegmentId, SitzungId};
use myl_types::miner::{HardwareClass, MinerRegistration};
use myl_types::node_metadata::GeoRegion;
use myl_types::sitzung::{pruefe, Befund, Sitzungskontrakt, Sitzungszustand, Vorhaben, Waehrung};
use myl_types::arbeitsverteilung::Arbeitsverteilung;
use myl_types::{InferenceCredit, PoIBundle};

use crate::state::{LedgerState, Sitzung, SITZUNG_NACHFRIST};

/// Fehler eines Zustandsübergangs. Übergänge sind atomar: Tritt ein
/// Fehler auf, wurde der Zustand nicht verändert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionError {
    /// Betrag wäre 0 — sinnfreie Übergänge werden abgelehnt.
    ZeroAmount,
    /// Das Konto hat nicht genug verfügbare MYL.
    InsufficientBalance { available: u64, required: u64 },
    /// Das Konto hat nicht genug Credits (oder nur verfallene).
    InsufficientCredits { available: u64, required: u64 },
    /// Das Konto hat keinen Stake (z. B. Verdict gegen einen
    /// Beteiligten ohne Hinterlegung).
    NoStake,
    /// Eine Buchung würde den `u64`-Bereich überschreiten.
    Overflow,
    /// Übergangs-Parameter sind ungültig (z. B. Bruch mit Nenner 0
    /// oder Zähler größer als Nenner).
    InvalidParameters,
    /// Unter dieser Adresse steht keine Session.
    SitzungUnbekannt,
    /// Unter dieser Adresse steht schon eine.
    SitzungExistiert,
    /// Der Kontrakt ist abgelaufen, bevor er eröffnet wurde.
    SitzungAbgelaufen,
    /// Nur der Inhaber darf widerrufen oder eröffnen, nicht der Agent.
    NichtDerInhaber,
    /// ⚑ Diese Gegenstandsart gehört nicht einzeln in den Zustand.
    ///
    /// Die Wissensdatenbank wächst mit der Nutzung, und
    /// [`LedgerState::commitment`] serialisiert den ganzen Zustand: Jeder
    /// Block zahlte sonst für die ganze Datenbank. Solche Gegenstände
    /// laufen über eine Wurzel (κ_v), und dieser Weg ist noch nicht
    /// gebaut. **Der Fehler benennt das, statt sie stillschweigend
    /// aufzunehmen** (STORAGE D7).
    GegenstandUeberWurzel,
    /// Unter dieser Wurzel steht schon ein Gegenstand.
    GegenstandExistiert,
    /// Unter dieser Wurzel steht keiner.
    GegenstandUnbekannt,
    /// ⚑ Wer das Auszahlungskonto ändern will, ist nicht das Konto.
    ///
    /// Die **erste** Eintragung darf der Miner selbst vornehmen; jede
    /// weitere gehört dem eingetragenen Konto. Damit leitet ein
    /// gestohlener heißer Schlüssel den Ertrag nicht um.
    NichtDasAuszahlungskonto,
    /// Der Unterzeichner ist nicht der Miner.
    NichtDerMiner,
    /// Der Miner ist nicht angemeldet.
    MinerUnbekannt,
    /// Der Schlüssel gehört nicht zu dieser Kennung.
    SchluesselPasstNicht,
    /// Das Bündel gehört zu einer anderen Epoche als der laufenden.
    FremdeEpoche {
        /// Die Epoche im Bündel.
        buendel: EpochId,
        /// Die laufende Epoche des Zustands.
        laufend: EpochId,
    },
    /// Für diesen Pod liegt in dieser Epoche schon ein Bündel vor.
    BuendelExistiert,
    /// Für diesen Pipeline-Stand steht schon eine Arbeitsverteilung.
    VerteilungExistiert,
    /// Das Manifest nennt eine andere Wurzel, als unter der es abgelegt
    /// werden soll.
    ///
    /// Kann nicht vorkommen, solange der Aufrufer die Wurzel aus dem
    /// Manifest nimmt, und genau deshalb steht die Prüfung hier: Ein
    /// Eintrag, dessen Schlüssel nicht zu seinem Inhalt passt, wäre
    /// später von außen nicht mehr auffindbar.
    WurzelPasstNicht,
    /// ⚑ Der Einreicher ist nicht der Agent des Vorhabens.
    ///
    /// `myl_types::sitzung::pruefe` prüft, ob der **im Vorhaben
    /// genannte** Handelnde der Agent des Kontrakts ist. Wer das
    /// Vorhaben eingereicht hat, weiß es nicht. Ohne diese zweite
    /// Prüfung könnte jeder ein Vorhaben hinschreiben, in dem der echte
    /// Agent als Handelnder steht, und unter fremdem Kontrakt zahlen.
    NichtDerHandelnde,
    /// ⚑ Eine Überweisung an sich selbst.
    ///
    /// Sie bewegt nichts und ist deshalb sinnlos, aber sie wird nicht
    /// aus Ordnungsliebe abgewiesen: **Der naheliegende Weg, eine
    /// Überweisung zu schreiben, ist „vom Absender abziehen, beim
    /// Empfänger addieren", und bei gleichem Konto verdoppelt das den
    /// Betrag**, wenn der Absenderstand vorher gelesen wurde. Ein
    /// abgewiesener Sonderfall kann nicht falsch gerechnet werden.
    SelbstUeberweisung,
    /// Die Transaktionsnummer passt nicht zum Konto.
    FalscheNonce {
        /// Was das Konto erwartet.
        erwartet: u64,
        /// Was in der Transaktion stand.
        hatte: u64,
    },
    /// Der Kontrakt lässt dieses Vorhaben nicht zu (Whitepaper
    /// Kap. 8.2).
    ///
    /// ⚑ **Der Befund ist der reiche**, mit Zahlen, für den Knoten und
    /// den Client des Inhabers. Was der Agent erfährt, ist
    /// [`myl_types::sitzung::Befund::fuer_agenten`], und das ist ein
    /// Bit. Wer diesen Fehler unbesehen an den Agenten durchreicht,
    /// hebt die Anforderung aus Kap. 8.2 auf.
    KontraktVerbietet(Befund),
}

impl std::fmt::Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroAmount => write!(f, "Übergang mit Betrag 0 abgelehnt"),
            Self::InsufficientBalance { available, required } => write!(
                f,
                "Kontostand reicht nicht: verfügbar {}, benötigt {}",
                available, required
            ),
            Self::InsufficientCredits { available, required } => write!(
                f,
                "Credit-Guthaben reicht nicht: verfügbar {}, benötigt {}",
                available, required
            ),
            Self::NoStake => write!(f, "Konto hat keinen Stake"),
            Self::Overflow => write!(f, "Buchung würde den Wertebereich überschreiten"),
            Self::InvalidParameters => write!(f, "Übergangs-Parameter sind ungültig"),
            Self::SitzungUnbekannt => write!(f, "keine Session unter dieser Adresse"),
            Self::SitzungExistiert => write!(f, "unter dieser Adresse steht schon eine Session"),
            Self::SitzungAbgelaufen => write!(f, "Kontrakt ist bereits abgelaufen"),
            Self::NichtDerInhaber => write!(f, "nur der Inhaber darf eröffnen und widerrufen"),
            Self::NichtDerHandelnde => {
                write!(f, "Einreicher ist nicht der Handelnde des Vorhabens")
            }
            Self::SelbstUeberweisung => write!(f, "Überweisung an das eigene Konto"),
            Self::FalscheNonce { erwartet, hatte } => {
                write!(f, "Transaktionsnummer {hatte}, erwartet {erwartet}")
            }
            Self::KontraktVerbietet(b) => write!(f, "Session-Kontrakt: {b}"),
            Self::GegenstandUeberWurzel => write!(
                f,
                "diese Gegenstandsart läuft über eine Wurzel und gehört \
                 nicht einzeln in den Zustand"
            ),
            Self::GegenstandExistiert => {
                write!(f, "unter dieser Wurzel steht schon ein Gegenstand")
            }
            Self::GegenstandUnbekannt => {
                write!(f, "unter dieser Wurzel steht kein Gegenstand")
            }
            Self::NichtDasAuszahlungskonto => write!(
                f,
                "nur der Miner selbst darf erstmals eintragen, danach nur \
                 das eingetragene Konto"
            ),
            Self::NichtDerMiner => {
                f.write_str("nur der Miner selbst darf sich an- und abmelden")
            }
            Self::MinerUnbekannt => f.write_str("dieser Miner ist nicht angemeldet"),
            Self::SchluesselPasstNicht => {
                f.write_str("der Schluessel gehoert nicht zu dieser Kennung")
            }
            Self::FremdeEpoche { buendel, laufend } => write!(
                f,
                "das Buendel gilt fuer Epoche {}, laufend ist {}",
                buendel.0, laufend.0
            ),
            Self::BuendelExistiert => {
                f.write_str("fuer diesen Pod liegt in dieser Epoche schon ein Buendel vor")
            }
            Self::VerteilungExistiert => {
                f.write_str("fuer diesen Pipeline-Stand steht schon eine Arbeitsverteilung")
            }
            Self::WurzelPasstNicht => write!(
                f,
                "das Manifest nennt eine andere Wurzel als die, unter der \
                 es abgelegt werden soll"
            ),
        }
    }
}

impl std::error::Error for TransitionError {}

/// Ausgang eines Bisektions-Schiedsspruchs (Whitepaper Kap. 6.6,
/// Anhang A.4: `Verdict::SlashMiner` / `Verdict::SlashChecker`).
///
/// Minimaler Zwischen-Typ in `myl-ledger`, bis VERIFICATION den vollen
/// Challenge-/Verdict-Typ definiert (dokumentierte Übergangslösung).
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum VerdictOutcome {
    /// Der Miner hat falsch gerechnet: sein Stake wird geschlachtet,
    /// der Checker erhält das Kopfgeld.
    SlashMiner,
    /// Der Checker hat falsch beschuldigt: sein Stake wird geschlachtet,
    /// der Miner erhält das Kopfgeld.
    SlashChecker,
}

/// Schiedsspruch über ein Segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Verdict {
    /// Das strittige Segment.
    pub segment_id: SegmentId,
    /// Der beteiligte Shard-Miner.
    pub miner: Address,
    /// Der beteiligte Checker.
    pub checker: Address,
    /// Wer wurde für schuldig befunden.
    pub outcome: VerdictOutcome,
}

/// Slash-Parameter (Brüche als Ganzzahl-Paare — keine Gleitkomma).
/// Die Werte sind Start-/Testparameter; die endgültigen Werte legt
/// TOKENOMICS (Kap. 5.5) fest, die Verwaltung übernimmt GOVERNANCE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashParams {
    /// Anteil des Stakes, der geschlachtet wird (Zähler/Nenner, ≤ 1).
    pub slash_fraction_num: u64,
    /// Nenner des Slash-Anteils (> 0).
    pub slash_fraction_den: u64,
    /// Anteil des geschlachteten Betrags, der als Kopfgeld an die
    /// Gegenpartei ausgezahlt wird (Zähler/Nenner, ≤ 1). Der Rest
    /// verbleibt unverteilt (= faktisch verbrannt).
    pub bounty_fraction_num: u64,
    /// Nenner des Kopfgeld-Anteils (> 0).
    pub bounty_fraction_den: u64,
}

/// Ergebnis eines angewendeten Verdicts (für die weitere Abrechnung,
/// z. B. die vTFE-Rückbuchung beim Epochenabschluss in Phase 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerdictEffect {
    /// Geschlachteter Stake-Betrag.
    pub slashed: u64,
    /// Ausgezahltes Kopfgeld.
    pub bounty: u64,
    /// Verstöße des Schuldigen **vor** diesem Urteil, innerhalb von
    /// [`crate::state::VERSTOSS_FENSTER`] Epochen.
    ///
    /// **Der Stand, gegen den der Satz gilt**, nicht der danach. Die
    /// Staffelung der Slashing-Matrix zählt Vorverstöße: `0` ist der
    /// erste Verstoß. Wer den Wert nach dem Buchen abfragte, bekäme
    /// einen zu hohen und schlüge den nächsten Satz zu früh auf.
    ///
    /// Er steht hier, damit der Aufrufer belegen kann, welcher Satz zu
    /// gelten hatte — der Übergang selbst prüft das nicht, er bekommt
    /// die Anteile fertig.
    pub vorverstoesse: u64,
}

/// `apply_verdict(v) → Stake slashen, Kopfgeld auszahlen, vTFE
/// rückbuchen` (Anhang A.5).
///
/// Schlachtet den Stake der schuldigen Partei (Anteil gemäß
/// `slash_fraction`), zahlt der Gegenpartei das Kopfgeld (Anteil des
/// geschlachteten Betrags gemäß `bounty_fraction`) und lässt den Rest
/// unverteilt (faktisch verbrannt). Die vTFE-Rückbuchung des Segments
/// erfolgt beim Epochenabschluss (Phase 4); dieser Übergang liefert die
/// dafür nötigen Beträge im [`VerdictEffect`].
///
/// ⚑ **Er vermerkt außerdem einen Verstoß beim Schuldigen** (seit dem
/// 2026-08-27). Das ist die Vorgeschichte, aus der die Slashing-Matrix
/// ihre Staffelung zieht; sie steht im Konsenszustand, weil zwei Knoten
/// mit verschiedenen Vorgeschichten verschieden hoch schlachten und
/// damit auseinanderlaufen.
///
/// **Die Anteile bleiben Eingabe.** Dieser Übergang entscheidet nicht,
/// wie hoch geschlachtet wird — das tut `myl-tokenomics`, und
/// `myl_tokenomics::slashing::satz_aus_ledger` liest die Vorgeschichte
/// dafür aus genau diesem Zustand. Die Arbeitsteilung „wer / wie viel /
/// wie gebucht" bleibt unberührt.
pub fn apply_verdict(
    state: &mut LedgerState,
    verdict: &Verdict,
    params: &SlashParams,
) -> Result<VerdictEffect, TransitionError> {
    // Parameter-Prüfung.
    if params.slash_fraction_den == 0
        || params.bounty_fraction_den == 0
        || params.slash_fraction_num > params.slash_fraction_den
        || params.bounty_fraction_num > params.bounty_fraction_den
    {
        return Err(TransitionError::InvalidParameters);
    }
    let (guilty, innocent) = match verdict.outcome {
        VerdictOutcome::SlashMiner => (verdict.miner, verdict.checker),
        VerdictOutcome::SlashChecker => (verdict.checker, verdict.miner),
    };
    if guilty == innocent {
        return Err(TransitionError::InvalidParameters);
    }

    // Prüfphase: Stake vorhanden?
    let staked = state.account(&guilty).staked;
    if staked == 0 {
        return Err(TransitionError::NoStake);
    }
    let slashed = staked
        .checked_mul(params.slash_fraction_num)
        .and_then(|v| v.checked_div(params.slash_fraction_den))
        .ok_or(TransitionError::Overflow)?;
    let bounty = slashed
        .checked_mul(params.bounty_fraction_num)
        .and_then(|v| v.checked_div(params.bounty_fraction_den))
        .ok_or(TransitionError::Overflow)?;
    let innocent_balance = state.account(&innocent).balance;
    innocent_balance
        .checked_add(bounty)
        .ok_or(TransitionError::Overflow)?;

    // Vorgeschichte **vor** dem Vermerk, damit der Aufrufer im Ergebnis
    // sieht, gegen welchen Stand der Satz gegolten hat. Danach zu lesen
    // wäre um eins zu hoch.
    let vorverstoesse = state.verstoesse_im_fenster(&guilty, crate::state::VERSTOSS_FENSTER);

    // Änderungsphase.
    state.account_mut(&guilty).staked = staked - slashed;
    state.account_mut(&innocent).balance = innocent_balance + bounty;
    // **Der Vermerk gehört hierher und nirgendwo sonst.** Ein Urteil,
    // das gebucht wird, ohne gezählt zu werden, macht die Staffelung zu
    // einer Absichtserklärung: Der nächste Verstoß wäre wieder der
    // erste. Weil er im selben Übergang steht, kann er nicht vergessen
    // werden.
    state.verstoss_vermerken(&guilty);
    Ok(VerdictEffect { slashed, bounty, vorverstoesse })
}

/// `burn(addr, syn) → mint_credits(addr, syn / preis_e)` (Anhang A.5).
///
/// Verbrennt `syn` MYL-Kleinstbeträge vom Konto und prägt dafür
/// Inferenz-Credits: `floor(syn / credit_price)` vTFE-Einheiten,
/// gültig bis einschließlich `credit_expiry` (Protokoll-Parameter,
/// später Governance). Die Division rundet abwärts — es werden niemals
/// mehr Credits geprägt, als der Burn deckt.
///
/// Liefert die Anzahl geprägter Credits zurück.
pub fn burn_to_credits(
    state: &mut LedgerState,
    addr: &Address,
    syn: u64,
    credit_expiry: EpochId,
) -> Result<u64, TransitionError> {
    if syn == 0 {
        return Err(TransitionError::ZeroAmount);
    }
    // Prüfphase: Deckung vorhanden?
    let available = state.account(addr).balance;
    if available < syn {
        return Err(TransitionError::InsufficientBalance {
            available,
            required: syn,
        });
    }
    let credit_price = state.credit_price;
    if credit_price == 0 {
        // Ein Credit-Preis von 0 ist ein Protokollfehler (Division durch
        // null); zu Genesis muss der Preis gesetzt sein.
        return Err(TransitionError::Overflow);
    }
    let minted = syn / credit_price;

    // Änderungsphase.
    let account = state.account_mut(addr);
    account.balance = available - syn;
    if minted > 0 {
        account.credits.push(InferenceCredit {
            owner: *addr,
            vtfe: minted,
            expiry: credit_expiry,
        });
        // Ausgabereihenfolge-Invariante: Credits stehen aufsteigend nach
        // Verfalls-Epoche (siehe `credit_spend`).
        account.credits.sort_by_key(|c| c.expiry);
    }

    // ⚑ **Was verbrannt wurde, wird gezählt** (2026-08-31). Bis dahin
    // zerstörte dieser Übergang Münzen und vergaß sofort, wie viele es
    // waren. Kap. 5.2 leitet die Prägung aus dem geglätteten Burn ab und
    // den geglätteten aus dem Burn je Epoche; ohne diese Zeile hat die
    // Prägungsformel keine Eingabe im Zustand.
    //
    // Gezählt wird `syn`, das Verbrannte, nicht `minted`: Der
    // Rundungsrest bei der Umrechnung in Credits ist ebenfalls
    // vernichtet und gehört dazu.
    state.burn_epoche = state.burn_epoche.saturating_add(syn);

    // ⚑ **Was verbrannt wurde, wird gezählt** (2026-08-31). Bis dahin
    // zerstörte dieser Übergang Münzen und vergaß sofort, wie viele es
    // waren. Kap. 5.2 leitet die Prägung aus dem geglätteten Burn ab und
    // den geglätteten aus dem Burn je Epoche; ohne diese Zeile hat die
    // Prägungsformel keine Eingabe im Zustand.
    //
    // Gezählt wird `syn`, das Verbrannte, nicht `minted`: Der
    // Rundungsrest bei der Umrechnung in Credits ist ebenfalls
    // vernichtet und gehört dazu.
    Ok(minted)
}

/// `credit_spend(session, vtfe) → Session-Budget abbuchen` (Anhang A.5).
///
/// Bucht `vtfe` Einheiten vom Credit-Guthaben des Kontos ab.
/// Ausgabe-Regeln (deterministisch und konsensrelevant):
/// - Verfallene Credits (`expiry` < aktuelle Epoche) sind unbrauchbar
///   und werden beim Abbuchen entsorgt.
/// - Verbrauch in Reihenfolge des frühesten Verfalls (Credits stehen
///   aufsteigend sortiert), Teilverbrauch kürzt den betroffenen Credit.
/// - Reicht das Guthaben nicht, wird der Übergang abgelehnt und der
///   Zustand nicht verändert (Session-Kontrakte behandeln das als
///   „Budget erschöpft").
///
/// Die Session-Zuordnung (welche Session zu welchem Konto bucht) ist
/// Aufgabe der AGENT_LAYER-Kontrakte; der Ledger kennt nur das Konto.
pub fn credit_spend(
    state: &mut LedgerState,
    owner: &Address,
    vtfe: u64,
) -> Result<(), TransitionError> {
    if vtfe == 0 {
        return Err(TransitionError::ZeroAmount);
    }
    let epoch = state.epoch;

    // Prüfphase: verfügbares (nicht verfallenes) Guthaben.
    let account = state.account(owner);
    let available = account
        .credits
        .iter()
        .filter(|c| c.expiry >= epoch)
        .fold(0u64, |sum, c| sum.saturating_add(c.vtfe));
    if available < vtfe {
        return Err(TransitionError::InsufficientCredits {
            available,
            required: vtfe,
        });
    }

    // Änderungsphase: verfallene entsorgen, dann FIFO verbrauchen.
    let account = state.account_mut(owner);
    let alt = std::mem::take(&mut account.credits);
    let mut remaining = vtfe;
    for credit in alt {
        if credit.expiry < epoch {
            continue; // verfallen — entsorgt
        }
        if remaining == 0 {
            account.credits.push(credit);
            continue;
        }
        if credit.vtfe <= remaining {
            remaining -= credit.vtfe; // vollständig verbraucht
        } else {
            account.credits.push(InferenceCredit {
                owner: credit.owner,
                vtfe: credit.vtfe - remaining,
                expiry: credit.expiry,
            });
            remaining = 0;
        }
    }
    Ok(())
}

/// Überweist MYL-Kleinstbeträge von einem Konto auf ein anderes.
///
/// ⚑ **Bis zum 2026-08-28 gab es diesen Übergang nicht** (Fund 83),
/// obwohl Whitepaper Kap. 8.2 ihn voraussetzt: Ein Session-Kontrakt
/// begrenzt Zahlungen, und ohne Zahlung ist die Grenze gegenstandslos.
///
/// **Gestaktes MYL ist nicht verfügbar.** Nur `balance` wird bewegt;
/// wer überweisen will, was er hinterlegt hat, muss es erst lösen.
///
/// **Die Überweisung an sich selbst wird abgewiesen**, siehe
/// [`TransitionError::SelbstUeberweisung`].
///
/// **Erst prüfen, dann ändern.** Ein Überlauf auf der Empfängerseite
/// wird vor jeder Buchung erkannt, sonst wäre beim Fehlschlag Geld
/// abgezogen und nicht angekommen.
pub fn transfer(
    state: &mut LedgerState,
    von: &Address,
    nach: &Address,
    betrag: u64,
) -> Result<(), TransitionError> {
    if betrag == 0 {
        return Err(TransitionError::ZeroAmount);
    }
    if von == nach {
        return Err(TransitionError::SelbstUeberweisung);
    }

    // Prüfphase.
    let absender = state.account(von);
    if absender.balance < betrag {
        return Err(TransitionError::InsufficientBalance {
            available: absender.balance,
            required: betrag,
        });
    }
    let empfaenger_neu = state
        .account(nach)
        .balance
        .checked_add(betrag)
        .ok_or(TransitionError::Overflow)?;

    // Änderungsphase.
    state.account_mut(von).balance -= betrag;
    state.account_mut(nach).balance = empfaenger_neu;
    Ok(())
}

/// Prüft und verbraucht die Transaktionsnummer eines Kontos.
///
/// ⚑ **Verbraucht wird sie auch dann, wenn die Anweisung danach
/// scheitert.** Sonst wäre eine Transaktion, die an fehlender Deckung
/// scheitert, unverändert gültig und beliebig oft einreichbar. Der
/// Aufrufer ruft dies deshalb **vor** der Anweisung und wertet deren
/// Ergebnis getrennt aus.
///
/// Nur der Kontoinhaber kann eine gültige Unterschrift leisten, also
/// kann auch nur er die eigene Nummer verbrauchen.
pub fn nonce_verbrauchen(
    state: &mut LedgerState,
    konto: &Address,
    nonce: u64,
) -> Result<(), TransitionError> {
    let erwartet = state.account(konto).nonce;
    if nonce != erwartet {
        return Err(TransitionError::FalscheNonce { erwartet, hatte: nonce });
    }
    let neu = erwartet.checked_add(1).ok_or(TransitionError::Overflow)?;
    state.account_mut(konto).nonce = neu;
    Ok(())
}

/// Eröffnet eine Agenten-Session (Whitepaper Kap. 8.2).
///
/// Der Kontrakt wird unter seiner eigenen Adresse abgelegt. **Er wird
/// hier nicht mehr geprüft**, weil [`Sitzungskontrakt::neu`] die
/// Normalform bereits erzwungen hat und ein Kontrakt, der es an dieser
/// Stelle noch einmal versuchte, ohnehin eine andere Adresse hätte.
///
/// **Kein Betrag wird reserviert.** Das Budget ist eine Obergrenze und
/// keine Hinterlegung; ob der Inhaber die Credits hat, entscheidet sich
/// beim Ausgeben.
///
/// **Ein bereits abgelaufener Kontrakt wird abgelehnt**, statt als
/// unbrauchbarer Eintrag Zustand zu belegen.
///
/// ⚑ **`wer` ist der Einreicher, und er muss der Inhaber sein.** Ohne
/// diese Prüfung eröffnete jeder eine Session, die ein fremdes Konto
/// belastet: Der Kontrakt nennt den Inhaber selbst, und
/// [`sitzung_ausgeben`] bucht bei ihm. Die Prüfung steht hier und nicht
/// im Aufrufer, damit kein zweiter Aufrufer sie vergessen kann.
pub fn sitzung_eroeffnen(
    state: &mut LedgerState,
    wer: &Address,
    kontrakt: Sitzungskontrakt,
) -> Result<SitzungId, TransitionError> {
    if kontrakt.inhaber != *wer {
        return Err(TransitionError::NichtDerInhaber);
    }
    let adresse = kontrakt.adresse();
    if state.sitzungen.contains_key(&adresse) {
        return Err(TransitionError::SitzungExistiert);
    }
    if kontrakt.gueltig_bis.0 < state.epoch.0 {
        return Err(TransitionError::SitzungAbgelaufen);
    }
    state
        .sitzungen
        .insert(adresse, Sitzung { kontrakt, zustand: Sitzungszustand::neu() });
    Ok(adresse)
}

/// Beendet eine Session vorzeitig. Nur der Inhaber.
///
/// ⚑ **Der Widerruf steht nicht im Whitepaper**, und er gehört
/// trotzdem hierher: Ohne ihn ist das Zeitfenster das einzige Mittel
/// gegen einen Agenten, der sich falsch verhält, und dieses Mittel
/// heißt warten.
///
/// **Wiederholter Widerruf ist kein Fehler.** Zwei Blöcke, die
/// denselben Widerruf tragen, dürfen nicht dazu führen, dass der zweite
/// ungültig wird; und ein Übergang, der ausschließlich Rechte entzieht,
/// schadet auch beim zweiten Mal nicht.
pub fn sitzung_widerrufen(
    state: &mut LedgerState,
    adresse: &SitzungId,
    wer: &Address,
) -> Result<(), TransitionError> {
    let sitzung = state
        .sitzungen
        .get_mut(adresse)
        .ok_or(TransitionError::SitzungUnbekannt)?;
    if sitzung.kontrakt.inhaber != *wer {
        return Err(TransitionError::NichtDerInhaber);
    }
    sitzung.zustand.widerrufen = true;
    Ok(())
}

/// Gibt unter einem Session-Kontrakt aus, in Credits oder in MYL
/// (Whitepaper Kap. 8.2, Durchsetzung beim Ausführen).
///
/// ⚑ **Das ist die Stelle, an der der Kontrakt etwas bedeutet.** Ein
/// Client, der die Grenzen selbst prüft, prüft sie freiwillig; hier
/// prüft sie jeder Knoten, bevor er den Zustand fortschreibt.
///
/// **Belastet wird das Konto des Inhabers**, nicht das des Agenten. Der
/// Agent ist ein Schlüssel mit einer Vollmacht, kein Kontoinhaber.
///
/// **Erst prüfen, dann ändern**, und der Verbrauchszähler wächst erst,
/// wenn wirklich etwas geflossen ist: Ein Kontrakt, dessen Budget an
/// einer fehlgeschlagenen Ausgabe schrumpfte, wäre über wiederholte
/// Fehlschläge leerzuräumen.
///
/// ⚑ **Die beiden Währungen sind hier nicht symmetrisch, und das ist
/// kein Versehen.** MYL wechselt den Besitzer: Der Empfänger aus dem
/// Vorhaben bekommt sie gutgeschrieben. **Credits werden verbraucht und
/// nicht übertragen** — sie sind ein Anrecht auf Inferenzarbeit, kein
/// Zahlungsmittel, und `credit_spend` löscht sie ersatzlos. Die
/// Empfängerliste gilt trotzdem für beide: Bei MYL sagt sie, wohin
/// gezahlt werden darf, bei Credits, wessen Dienst bezogen werden darf.
///
/// ⚑ **`wer` ist der Einreicher, und er muss der Handelnde sein.**
/// `pruefe` vergleicht den *im Vorhaben genannten* Handelnden mit dem
/// Agenten des Kontrakts; wer das Vorhaben tatsächlich eingereicht hat,
/// steht dort nicht. Ohne diesen Vergleich schriebe ein Fremder den
/// echten Agenten ins Feld und zahlte unter dessen Kontrakt.
pub fn sitzung_ausgeben(
    state: &mut LedgerState,
    wer: &Address,
    vorhaben: &Vorhaben,
) -> Result<(), TransitionError> {
    if vorhaben.handelnder != *wer {
        return Err(TransitionError::NichtDerHandelnde);
    }
    // Prüfphase.
    let (inhaber, befund, neu_verbraucht) = {
        let sitzung = state
            .sitzungen
            .get(&vorhaben.sitzung)
            .ok_or(TransitionError::SitzungUnbekannt)?;
        let befund = pruefe(&sitzung.kontrakt, &sitzung.zustand, state.epoch, vorhaben);
        let neu = sitzung
            .zustand
            .verbraucht(vorhaben.waehrung)
            .checked_add(vorhaben.betrag)
            .ok_or(TransitionError::Overflow)?;
        (sitzung.kontrakt.inhaber, befund, neu)
    };
    if !befund.erlaubt() {
        return Err(TransitionError::KontraktVerbietet(befund));
    }

    // Änderungsphase.
    match vorhaben.waehrung {
        Waehrung::Credits => credit_spend(state, &inhaber, vorhaben.betrag)?,
        Waehrung::Myl => transfer(state, &inhaber, &vorhaben.empfaenger, vorhaben.betrag)?,
    }
    let sitzung = state
        .sitzungen
        .get_mut(&vorhaben.sitzung)
        .expect("in der Prüfphase war sie da, und keiner der Übergänge rührt sie an");
    match vorhaben.waehrung {
        Waehrung::Credits => sitzung.zustand.verbraucht_credits = neu_verbraucht,
        Waehrung::Myl => sitzung.zustand.verbraucht_myl = neu_verbraucht,
    }
    Ok(())
}

/// Trägt das Auszahlungskonto eines Miners ein oder ändert es.
///
/// # ⚑ Wer darf
///
/// | Lage | Wer unterschreibt |
/// |---|---|
/// | noch kein Eintrag | der **Miner selbst** |
/// | Eintrag vorhanden | das **eingetragene Konto** |
///
/// Die erste Eintragung darf der Miner vornehmen, er hat nichts zu
/// verlieren. Jede weitere gehört dem kalten Konto, damit ein
/// gestohlener Konsensschlüssel den Ertrag nicht umleiten kann. Es ist
/// Filecoins Trennung von `owner` und `worker`, nur ohne die Wartefrist,
/// über die sonst jemand streiten könnte.
///
/// `unterzeichner` ist die Adresse, die aus dem Schlüssel der
/// Transaktion folgt; `miner` ist die Kennung, für die eingetragen wird.
pub fn auszahlungskonto_eintragen(
    state: &mut LedgerState,
    unterzeichner: &Address,
    miner: &MinerId,
    konto: Address,
) -> Result<(), TransitionError> {
    let darf = match state.auszahlung.get(miner) {
        // Erste Eintragung: der Miner selbst. Kennung und Adresse sind
        // verschiedene Typen über denselben Bytes, deshalb der Vergleich
        // über die Bytes und nicht über den Typ.
        None => unterzeichner.as_bytes() == miner.as_bytes(),
        // Jede weitere: das eingetragene Konto.
        Some(bisher) => unterzeichner == bisher,
    };
    if !darf {
        return Err(TransitionError::NichtDasAuszahlungskonto);
    }
    state.auszahlung.insert(*miner, konto);
    Ok(())
}

/// Meldet einen Miner an oder ändert seine Hardware-Klasse (Punkt 40, 3a).
///
/// # ⚑ Wer darf
///
/// Nur der Miner selbst: Der Unterzeichner muss der Schlüssel hinter der
/// Kennung sein. Kennung und Adresse sind verschiedene Typen über
/// denselben Bytes, deshalb der Vergleich über die Bytes; dasselbe
/// Muster wie bei [`auszahlungskonto_eintragen`].
///
/// **Anders als beim Auszahlungskonto gibt es hier kein kaltes Konto.**
/// Eine Anmeldung ist keine Vermögensverfügung: Wer den heißen Schlüssel
/// hat, rechnet ohnehin unter dieser Kennung, und ihn daran zu hindern,
/// sich anzumelden, schützte nichts.
///
/// # ⚑ Die Zone ist eine Angabe, und sie steht hier statt im Gossip
///
/// Sie entscheidet, in welchem Topf ein Miner gemischt wird
/// (Entscheidung 3b vom 2026-09-01). Wer eine falsche nennt, wird nicht
/// ertappt; **vorwärts bestraft es sich selbst**, denn er bremst die
/// Zone, in die er sich setzt, und die Vergütung folgt der Arbeit.
/// Rückwärts nicht, und das ist Fund 108.
///
/// **Anders als `NodeMetadata::region` steht sie im Konsenszustand**,
/// ist also für jeden Leser dieselbe. Das ist der eigentliche Gewinn:
/// nicht Wahrheit, sondern Gleichheit.
///
/// # ⚑ Die Registrierungsepoche setzt die Kette, nicht der Antragsteller
///
/// Sie ist die **laufende** Epoche des Zustands und kein Feld aus der
/// Anweisung. Ein selbst gewähltes Datum wäre eine Behauptung, und der
/// Registrierungsschluss (Epoche `e-2`, Anhang A.2) soll gerade
/// verhindern, dass sich jemand kurzfristig anmeldet, um eine Zuteilung
/// zu beeinflussen. Wer sein Datum selbst schriebe, hätte den Schluss
/// aufgehoben.
///
/// # Eine bestehende Anmeldung behält ihr Datum
///
/// Wer die Hardware-Klasse ändert, bleibt so alt, wie er ist.
/// ⚑ **Sonst wäre die Klassenänderung ein Weg, den Registrierungsschluss
/// zu umgehen**, und zwar in die falsche Richtung: Ein Wechsel machte
/// den Miner **jünger** und damit für die nächste Zuteilung
/// unqualifiziert; ein Angreifer könnte so einen ehrlichen Miner nicht
/// treffen, wohl aber sich selbst aus einer Zuteilung nehmen, in die er
/// nicht wollte.
pub fn miner_anmelden(
    state: &mut LedgerState,
    unterzeichner: &Address,
    miner: &MinerId,
    hardware: HardwareClass,
    zone: GeoRegion,
    schluessel: myl_types::bls::BlsPublicKey,
) -> Result<(), TransitionError> {
    if unterzeichner.as_bytes() != miner.as_bytes() {
        return Err(TransitionError::NichtDerMiner);
    }
    // ⚑ **Der Schlüssel muss zur Kennung passen.** Sonst trüge das
    // Register einen fremden Schlüssel unter dieser Kennung, und die
    // Aggregatprüfung eines Pods liefe gegen den falschen.
    if MinerId::aus_schluessel(&schluessel) != *miner {
        return Err(TransitionError::SchluesselPasstNicht);
    }
    let seit = state
        .miner
        .get(miner)
        .map(|r| r.registration_epoch)
        .unwrap_or(state.epoch.0);
    state.miner.insert(
        *miner,
        MinerRegistration {
            miner_id: *miner,
            hardware_class: hardware,
            registration_epoch: seit,
            zone,
            schluessel,
        },
    );
    Ok(())
}

/// Meldet einen Miner ab.
///
/// ⚑ **Und die Abmeldung wirkt sofort, nicht erst zur nächsten
/// Zuteilung.** Wer geht, geht; ihn bis zum Epochenwechsel in der Liste
/// zu führen hieße, ihn in Pods zu setzen, die er nicht mehr besetzt.
/// Der Registrierungsschluss schützt die Zuteilung vor **Zugängen**, die
/// sie beeinflussen wollen, nicht vor Abgängen.
pub fn miner_abmelden(
    state: &mut LedgerState,
    unterzeichner: &Address,
    miner: &MinerId,
) -> Result<(), TransitionError> {
    if unterzeichner.as_bytes() != miner.as_bytes() {
        return Err(TransitionError::NichtDerMiner);
    }
    if state.miner.remove(miner).is_none() {
        return Err(TransitionError::MinerUnbekannt);
    }
    Ok(())
}

/// Die angemeldeten Miner in kanonischer Ordnung.
///
/// ⚑ **Kanonisch, weil daran die Pod-Bildung hängt.** Eine
/// `BTreeMap` liefert ihre Schlüssel sortiert; eine `HashMap` täte es
/// nicht, und zwei Knoten kämen zu verschiedenen Zuteilungen, ohne dass
/// etwas kaputt wäre.
pub fn angemeldete_miner(state: &LedgerState) -> Vec<MinerRegistration> {
    state.miner.values().copied().collect()
}

/// Setzt die Arbeitsverteilung der Pod-Positionen.
///
/// # ⚑ Eine Verteilung je Pipeline-Stand, und nicht zwei
///
/// Steht für **denselben** Pipeline-Stand schon eine Verteilung, wird
/// abgelehnt. Die Gewichte folgen aus dem Stand; **dieselbe Pipeline
/// zweimal verschieden zu gewichten hieße, dass sie nicht aus ihr
/// folgen**, und dann wären sie frei wählbar. Wer anders gewichten will,
/// wechselt den Stand, und der Wechsel ist sichtbar.
///
/// # ⚑ Wer setzen darf, ist noch nicht durchgesetzt
///
/// Das ist ein **Governance-Akt**, und der Draht von einem angenommenen
/// Beschluss hierher fehlt, wie bei der Belastung der Treasury. Bis
/// dahin prüft diese Funktion die Form und nicht die Befugnis, **und das
/// steht hier, statt eine Sicherheit vorzugeben, die es nicht gibt**.
pub fn arbeitsverteilung_setzen(
    state: &mut LedgerState,
    verteilung: Arbeitsverteilung,
) -> Result<(), TransitionError> {
    if let Some(bisher) = &state.arbeitsverteilung {
        if bisher.pipeline() == verteilung.pipeline() {
            return Err(TransitionError::VerteilungExistiert);
        }
    }
    state.arbeitsverteilung = Some(verteilung);
    Ok(())
}

/// Nimmt ein PoI-Bündel in die laufende Epoche auf (Punkt 40, Glied 1).
///
/// # Was hier geprüft wird
///
/// - Der Einreichende ist ein **angemeldeter Miner**.
/// - Das Bündel gehört zur **laufenden** Epoche.
/// - Für diesen Pod liegt in dieser Epoche **noch keines** vor.
///
/// # ⚑ Was hier ausdrücklich **nicht** geprüft wird, und warum
///
/// **Die Aggregatsignatur gegen die Pod-Mitglieder.** Sie ist die
/// eigentliche Prüfung, und sie setzt voraus, dass der Zustand weiß, wer
/// in diesem Pod sitzt. Das tut er noch nicht: Die Ableitung der
/// Zuteilung ist Glied 3c und steht aus.
///
/// **Solange sie fehlt, ist „angemeldeter Miner" eine schwache
/// Schranke**, und das gehört gesagt statt verschwiegen: Ein
/// angemeldeter Miner kann heute ein Bündel für **irgendeinen** Pod
/// einreichen. Was ihn bremst, ist allein, dass die Zuschreibung ohne
/// Besetzung ohnehin nichts ausschüttet; **die Lücke ist bekannt,
/// begrenzt und benannt**, und sie schließt sich mit 3c.
///
/// Sie ist kein Grund, das Glied nicht zu bauen: Ohne einen Weg in die
/// Kette gibt es nichts zu prüfen.
pub fn buendel_einreichen(
    state: &mut LedgerState,
    unterzeichner: &Address,
    buendel: PoIBundle,
) -> Result<(), TransitionError> {
    let kennung = MinerId::new(*unterzeichner.as_bytes());
    if !state.miner.contains_key(&kennung) {
        return Err(TransitionError::MinerUnbekannt);
    }
    if buendel.epoch != state.epoch {
        return Err(TransitionError::FremdeEpoche {
            buendel: buendel.epoch,
            laufend: state.epoch,
        });
    }
    if state.buendel.contains_key(&buendel.pod) {
        return Err(TransitionError::BuendelExistiert);
    }
    state.buendel.insert(buendel.pod, buendel);
    Ok(())
}

/// Die Bündel der laufenden Epoche in kanonischer Ordnung.
pub fn buendel_der_epoche(state: &LedgerState) -> Vec<PoIBundle> {
    state.buendel.values().cloned().collect()
}

/// Leert die Bündel, weil die Epoche abgerechnet ist.
///
/// ⚑ **Ohne diesen Aufruf wächst der Zustand unbegrenzt**, und
/// Entscheidung D7 wäre gebrochen. Er gehört in denselben Zug wie der
/// Epochenabschluss; **die Historie steht in den Blöcken**, nicht hier.
///
/// Gibt zurück, wie viele geleert wurden, damit ein Aufrufer die Zahl
/// protokollieren kann statt sie zu raten.
pub fn buendel_leeren(state: &mut LedgerState) -> usize {
    let n = state.buendel.len();
    state.buendel.clear();
    n
}

/// Schreibt geprägte MYL einem Konto gut.
///
/// # ⚑ Die einzige Stelle, an der MYL entsteht
///
/// Jeder andere Übergang schiebt Guthaben oder vernichtet es;
/// hier allein wächst die Menge. Das ist der Grund, aus dem diese
/// Funktion so klein ist und keine Bedingung selbst prüft: **Wer prägen
/// darf und wie viel, entscheidet die Wirtschaftsrechnung**
/// (`myl_tokenomics::ausschuettung`), nicht das Kontenbuch. Ein Ledger,
/// das die Prägeformel kennte, hätte zwei Orte für dieselbe Wahrheit.
///
/// Bis zum 2026-08-31 gab es sie nicht. Der Burn wurde gezählt, der
/// geglättete Wert fortgeschrieben, die Prägung gerechnet, und dann
/// endete der Weg: **Es gab keinen Übergang, der ein Konto erhöht.**
///
/// # Was geprüft wird
///
/// Ein Betrag von null ist ein Fehler, kein Nichtstun: Wer null prägt,
/// hat sich verrechnet, und ein stiller Erfolg verdeckt das. Ein
/// Überlauf gibt [`TransitionError::Overflow`] zurück, statt zu sättigen;
/// gesättigte Prägung wäre stillschweigend eine andere Geldmenge.
pub fn praegen(
    state: &mut LedgerState,
    konto: &Address,
    betrag: u64,
) -> Result<(), TransitionError> {
    if betrag == 0 {
        return Err(TransitionError::ZeroAmount);
    }
    let neu = state
        .account(konto)
        .balance
        .checked_add(betrag)
        .ok_or(TransitionError::Overflow)?;
    state.account_mut(konto).balance = neu;
    Ok(())
}

/// Wohin dieser Miner bezahlt wird, falls er es gesagt hat.
///
/// ⚑ **`None` heißt: kein Anteil.** Wer nichts eingetragen hat, wird bei
/// der Verteilung übergangen und sein Gewicht zählt nicht. So sammelt
/// sich nie ein Ertrag unter einem heißen Schlüssel an, und der Fehler
/// fällt sofort auf, weil nichts ankommt.
pub fn auszahlungskonto(state: &LedgerState, miner: &MinerId) -> Option<Address> {
    state.auszahlung.get(miner).copied()
}

/// Nimmt einen Gegenstand in das Speicherregister auf.
///
/// # Was geprüft wird, und in welcher Reihenfolge
///
/// Erst die **Ablage**: Wissensklassen gehören nicht einzeln in den
/// Zustand (STORAGE D7). Dann die **Wurzel**, dann die **Doppelung**.
/// Die Reihenfolge benennt den grundsätzlichen Einwand zuerst; ein
/// Wissensstück soll nicht erst an einer Doppelung scheitern und dann
/// beim zweiten Versuch an der Ablage.
///
/// # ⚑ Kein Guthaben, und das ist kein Vergessen
///
/// Ein Eintrag trägt hier **kein** Speicherguthaben. Jede Art mit
/// [`myl_types::gegenstand::Ablage::Direkt`] ist
/// [`myl_types::gegenstand::Finanzierung::Treasury`]: Was unmittelbar im
/// Zustand steht, trägt die Allgemeinheit, und was ein Einleger
/// bezahlt, läuft über die Wurzel. Ein Test in `myl-types` hält diesen
/// Zusammenhang fest, damit eine künftig ergänzte Art nicht still in die
/// Lücke fällt.
pub fn speicher_aufnehmen(
    state: &mut LedgerState,
    manifest: Manifest,
    wurzel: MerkleRoot,
) -> Result<(), TransitionError> {
    if manifest.art.ablage() != Ablage::Direkt {
        return Err(TransitionError::GegenstandUeberWurzel);
    }
    if manifest.wurzel != wurzel {
        return Err(TransitionError::WurzelPasstNicht);
    }
    if state.speicher.contains_key(&wurzel) {
        return Err(TransitionError::GegenstandExistiert);
    }
    state.speicher.insert(wurzel, manifest);
    Ok(())
}

/// Nimmt einen Gegenstand aus dem Register.
///
/// Für ausgemusterte Modellfassungen. **Ein Governance-Akt**, kein
/// Vorgang eines einzelnen Halters: Was hier verschwindet, findet ein
/// beitretender Miner nicht mehr.
pub fn speicher_entfernen(
    state: &mut LedgerState,
    wurzel: &MerkleRoot,
) -> Result<Manifest, TransitionError> {
    state
        .speicher
        .remove(wurzel)
        .ok_or(TransitionError::GegenstandUnbekannt)
}

/// Räumt Sessions weg, deren Fenster länger als
/// [`SITZUNG_NACHFRIST`] Epochen zurückliegt, und liefert deren Anzahl.
///
/// **Ein Übergang und kein Nebenher.** Würde beim Lesen aufgeräumt,
/// hinge der Zustand daran, wer wann gelesen hat; dieselbe Lehre wie
/// bei der Verstoßhistorie.
pub fn sitzung_aufraeumen(state: &mut LedgerState) -> usize {
    let jetzt = state.epoch.0;
    let vorher = state.sitzungen.len();
    state
        .sitzungen
        .retain(|_, s| jetzt <= s.kontrakt.gueltig_bis.0.saturating_add(SITZUNG_NACHFRIST));
    vorher - state.sitzungen.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::VERSTOSS_FENSTER;
    use myl_types::ids::EpochId;

    fn adresse(byte: u8) -> Address {
        Address::new([byte; 32])
    }

    fn state_mit_guthaben(addr: &Address, balance: u64, credit_price: u64) -> LedgerState {
        let mut state = LedgerState::genesis(credit_price);
        state.account_mut(addr).balance = balance;
        state
    }

    #[test]
    fn burn_praegt_credits_mit_boden_division() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 1_000, 7);
        // 1000 / 7 = 142 (abgerundet), Rest 6 verbrannt ohne Gegenwert.
        let minted = burn_to_credits(&mut state, &addr, 1_000, EpochId(10)).expect("burn");
        assert_eq!(minted, 142);
        assert_eq!(state.account(&addr).balance, 0);
        assert_eq!(state.account(&addr).credits.len(), 1);
        assert_eq!(state.account(&addr).credits[0].vtfe, 142);
        assert_eq!(state.account(&addr).credits[0].expiry, EpochId(10));
        assert_eq!(state.account(&addr).credits[0].owner, addr);
    }

    #[test]
    fn burn_ohne_deckung_wird_abgelehnt() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 50, 7);
        let davor = state.commitment();
        assert_eq!(
            burn_to_credits(&mut state, &addr, 100, EpochId(10)),
            Err(TransitionError::InsufficientBalance {
                available: 50,
                required: 100,
            })
        );
        // Zustand unverändert (atomarer Übergang).
        assert_eq!(state.commitment(), davor);
    }

    #[test]
    fn burn_mit_null_wird_abgelehnt() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 50, 7);
        assert_eq!(
            burn_to_credits(&mut state, &addr, 0, EpochId(10)),
            Err(TransitionError::ZeroAmount)
        );
    }

    #[test]
    fn burn_unter_einem_credit_preis_praegt_null() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 6, 7);
        // 6 / 7 = 0: verbrannt, aber kein Credit geprägt.
        let minted = burn_to_credits(&mut state, &addr, 6, EpochId(10)).expect("burn");
        assert_eq!(minted, 0);
        assert!(state.account(&addr).credits.is_empty());
        assert_eq!(state.account(&addr).balance, 0);
    }

    #[test]
    fn mehrere_burns_ansammlung_in_ablaufreihenfolge() {
        let addr = adresse(1);
        let mut state = state_mit_guthaben(&addr, 300, 10);
        burn_to_credits(&mut state, &addr, 100, EpochId(30)).expect("burn 1");
        burn_to_credits(&mut state, &addr, 100, EpochId(20)).expect("burn 2");
        burn_to_credits(&mut state, &addr, 100, EpochId(25)).expect("burn 3");
        let credits = &state.account(&addr).credits;
        assert_eq!(credits.len(), 3);
        // Aufsteigend nach Verfall sortiert (Invariante).
        assert_eq!(credits[0].expiry, EpochId(20));
        assert_eq!(credits[1].expiry, EpochId(25));
        assert_eq!(credits[2].expiry, EpochId(30));
        assert_eq!(state.account(&addr).balance, 0);
    }

    // --- apply_verdict ------------------------------------------------

    fn standard_params() -> SlashParams {
        // 50 % des Stakes slashen, davon 100 % als Kopfgeld.
        SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 2,
            bounty_fraction_num: 1,
            bounty_fraction_den: 1,
        }
    }

    fn verdict(outcome: VerdictOutcome) -> Verdict {
        Verdict {
            segment_id: SegmentId::new([9u8; 32]),
            miner: adresse(1),
            checker: adresse(2),
            outcome,
        }
    }

    #[test]
    fn verdict_slasht_miner_und_zahlt_kopfgeld() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(1)).staked = 1_000;
        let davor_checker = state.account(&adresse(2)).balance;
        let effect =
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &standard_params())
                .expect("Verdict");
        assert_eq!(effect, VerdictEffect { slashed: 500, bounty: 500, vorverstoesse: 0 });
        assert_eq!(state.account(&adresse(1)).staked, 500);
        assert_eq!(state.account(&adresse(2)).balance, davor_checker + 500);
    }

    #[test]
    fn verdict_slasht_checker_symmetrisch() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(2)).staked = 800;
        let effect =
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashChecker), &standard_params())
                .expect("Verdict");
        assert_eq!(effect, VerdictEffect { slashed: 400, bounty: 400, vorverstoesse: 0 });
        assert_eq!(state.account(&adresse(2)).staked, 400);
        assert_eq!(state.account(&adresse(1)).balance, 400);
    }

    #[test]
    fn verdict_rest_verbleibt_unverteilt() {
        // Kopfgeld nur 50 % des geschlachteten Betrags.
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(1)).staked = 1_000;
        let params = SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 1,
            bounty_fraction_num: 1,
            bounty_fraction_den: 2,
        };
        let effect =
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &params)
                .expect("Verdict");
        assert_eq!(effect, VerdictEffect { slashed: 1_000, bounty: 500, vorverstoesse: 0 });
        assert_eq!(state.account(&adresse(1)).staked, 0);
        assert_eq!(state.account(&adresse(2)).balance, 500);
        // Die übrigen 500 sind nirgends gutgeschrieben (faktisch verbrannt).
    }

    /// **Gebucht heisst gezaehlt, und zwar beim Schuldigen.**
    ///
    /// Wuerde der Unschuldige mitgezaehlt, waere ein erfolgreicher
    /// Checker nach drei gewonnenen Anfechtungen ein Wiederholungstaeter
    /// — die Staffelung traefe genau den, der sie ausgeloest hat.
    #[test]
    fn ein_gebuchtes_urteil_zaehlt_beim_schuldigen() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(1)).staked = 1_000;
        state.epoch = EpochId(4);

        let effekt =
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &standard_params())
                .expect("Verdict");
        assert_eq!(effekt.vorverstoesse, 0, "der erste Verstoss hat keine Vorgeschichte");
        assert_eq!(state.verstoesse_im_fenster(&adresse(1), VERSTOSS_FENSTER), 1);
        assert_eq!(
            state.verstoesse_im_fenster(&adresse(2), VERSTOSS_FENSTER),
            0,
            "der Unschuldige wurde mitgezaehlt"
        );
    }

    /// `vorverstoesse` ist der Stand **vor** dem Urteil.
    ///
    /// Der Satz der Slashing-Matrix haengt daran: `0` ist der erste
    /// Verstoss. Waere es der Stand danach, begaenne die Staffelung eine
    /// Stufe zu hoch, und der erste Ausfall waere schon der zweite.
    #[test]
    fn vorverstoesse_zaehlt_den_stand_vor_dem_urteil() {
        let mut state = LedgerState::genesis(10);
        state.epoch = EpochId(2);
        let params = SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 100,
            bounty_fraction_num: 0,
            bounty_fraction_den: 1,
        };
        for erwartet in 0..4u64 {
            state.account_mut(&adresse(1)).staked = 1_000_000;
            let effekt =
                apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &params)
                    .expect("Verdict");
            assert_eq!(
                effekt.vorverstoesse, erwartet,
                "beim {}. Urteil wurden {} Vorverstoesse gemeldet",
                erwartet + 1,
                effekt.vorverstoesse
            );
        }
    }

    /// **Ein abgelehntes Urteil zaehlt nicht.**
    ///
    /// Sonst waere „ohne Deckung anklagen" ein Weg, die Vorgeschichte
    /// eines anderen zu fuellen, ohne dass je etwas geschlachtet wurde.
    /// Die Pruefphase liegt deshalb vollstaendig vor der Aenderungsphase,
    /// und der Vermerk gehoert zur Aenderungsphase.
    #[test]
    fn ein_abgelehntes_urteil_zaehlt_nicht() {
        let mut state = LedgerState::genesis(10);
        state.epoch = EpochId(7);
        // Kein Stake: der Uebergang scheitert.
        assert!(apply_verdict(
            &mut state,
            &verdict(VerdictOutcome::SlashMiner),
            &standard_params()
        )
        .is_err());
        assert_eq!(state.verstoesse_im_fenster(&adresse(1), VERSTOSS_FENSTER), 0);

        // Und mit unbrauchbaren Anteilen ebenso wenig.
        state.account_mut(&adresse(1)).staked = 1_000;
        let kaputt = SlashParams {
            slash_fraction_num: 2,
            slash_fraction_den: 1,
            bounty_fraction_num: 1,
            bounty_fraction_den: 1,
        };
        assert!(apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &kaputt).is_err());
        assert_eq!(state.verstoesse_im_fenster(&adresse(1), VERSTOSS_FENSTER), 0);
    }

    /// Ein geschlachteter Checker ist ebenso ein Wiederholungstaeter.
    #[test]
    fn auch_der_geschlachtete_checker_wird_gezaehlt() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(2)).staked = 1_000;
        let _ = apply_verdict(
            &mut state,
            &verdict(VerdictOutcome::SlashChecker),
            &standard_params(),
        )
        .expect("Verdict");
        assert_eq!(state.verstoesse_im_fenster(&adresse(2), VERSTOSS_FENSTER), 1);
        assert_eq!(state.verstoesse_im_fenster(&adresse(1), VERSTOSS_FENSTER), 0);
    }

    #[test]
    fn verdict_ohne_stake_wird_abgelehnt() {
        let mut state = LedgerState::genesis(10);
        let davor = state.commitment();
        assert_eq!(
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &standard_params()),
            Err(TransitionError::NoStake)
        );
        assert_eq!(state.commitment(), davor);
    }

    #[test]
    fn verdict_mit_selbstbeteiligung_wird_abgelehnt() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(1)).staked = 100;
        let selbst = Verdict {
            segment_id: SegmentId::new([9u8; 32]),
            miner: adresse(1),
            checker: adresse(1),
            outcome: VerdictOutcome::SlashMiner,
        };
        assert_eq!(
            apply_verdict(&mut state, &selbst, &standard_params()),
            Err(TransitionError::InvalidParameters)
        );
    }

    #[test]
    fn verdict_mit_ungueltigen_bruechen_wird_abgelehnt() {
        let mut state = LedgerState::genesis(10);
        state.account_mut(&adresse(1)).staked = 100;
        let null_nenner = SlashParams {
            slash_fraction_num: 1,
            slash_fraction_den: 0,
            bounty_fraction_num: 1,
            bounty_fraction_den: 1,
        };
        assert_eq!(
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &null_nenner),
            Err(TransitionError::InvalidParameters)
        );
        let ueber_eins = SlashParams {
            slash_fraction_num: 3,
            slash_fraction_den: 2,
            bounty_fraction_num: 1,
            bounty_fraction_den: 1,
        };
        assert_eq!(
            apply_verdict(&mut state, &verdict(VerdictOutcome::SlashMiner), &ueber_eins),
            Err(TransitionError::InvalidParameters)
        );
    }

    // --- credit_spend -------------------------------------------------

    fn state_mit_credits(owner: &Address, credits: &[(u64, u64)], epoch: u64) -> LedgerState {
        // (vtfe, expiry)-Paare.
        let mut state = LedgerState::genesis(10);
        state.epoch = EpochId(epoch);
        let account = state.account_mut(owner);
        for &(vtfe, expiry) in credits {
            account.credits.push(InferenceCredit {
                owner: *owner,
                vtfe,
                expiry: EpochId(expiry),
            });
        }
        account.credits.sort_by_key(|c| c.expiry);
        state
    }

    #[test]
    fn credit_spend_verbraucht_fifo_nach_verfall() {
        let addr = adresse(1);
        // Credits: 10 bis Epoche 20, 30 bis Epoche 30.
        let mut state = state_mit_credits(&addr, &[(10, 20), (30, 30)], 5);
        credit_spend(&mut state, &addr, 15).expect("spend");
        let credits = &state.account(&addr).credits;
        // Die ersten 10 sind vollständig verbraucht, von den zweiten 5.
        assert_eq!(credits.len(), 1);
        assert_eq!(credits[0].vtfe, 25);
        assert_eq!(credits[0].expiry, EpochId(30));
    }

    #[test]
    fn credit_spend_verfallene_credits_sind_unbrauchbar() {
        let addr = adresse(1);
        // 100 Einheiten, aber in Epoche 50 bereits verfallen (expiry 20).
        let mut state = state_mit_credits(&addr, &[(100, 20)], 50);
        let davor = state.commitment();
        assert_eq!(
            credit_spend(&mut state, &addr, 1),
            Err(TransitionError::InsufficientCredits {
                available: 0,
                required: 1,
            })
        );
        // Zustand unverändert (Prüfphase lehnt vor Änderung ab).
        assert_eq!(state.commitment(), davor);
    }

    #[test]
    fn credit_spend_exakt_verfuegbar() {
        let addr = adresse(1);
        let mut state = state_mit_credits(&addr, &[(7, 20), (8, 30)], 5);
        credit_spend(&mut state, &addr, 15).expect("spend");
        assert!(state.account(&addr).credits.is_empty());
    }

    #[test]
    fn credit_spend_unzureichend_wird_abgelehnt() {
        let addr = adresse(1);
        let mut state = state_mit_credits(&addr, &[(5, 20)], 5);
        assert_eq!(
            credit_spend(&mut state, &addr, 6),
            Err(TransitionError::InsufficientCredits {
                available: 5,
                required: 6,
            })
        );
    }

    #[test]
    fn credit_spend_null_wird_abgelehnt() {
        let addr = adresse(1);
        let mut state = state_mit_credits(&addr, &[(5, 20)], 5);
        assert_eq!(credit_spend(&mut state, &addr, 0), Err(TransitionError::ZeroAmount));
    }

    #[test]
    fn credit_spend_teilverbrauch_erhaelt_reihenfolge() {
        let addr = adresse(1);
        let mut state = state_mit_credits(&addr, &[(10, 20), (10, 30), (10, 40)], 5);
        credit_spend(&mut state, &addr, 5).expect("spend");
        let credits = &state.account(&addr).credits;
        assert_eq!(credits.len(), 3);
        assert_eq!(credits[0].vtfe, 5);
        assert_eq!(credits[0].expiry, EpochId(20));
        assert_eq!(credits[1].vtfe, 10);
        assert_eq!(credits[2].vtfe, 10);
    }

    // ---- Session-Kontrakte (Whitepaper Kap. 8.2) ----

    fn grenzen(budget: u64, einzel: u64, schwelle: u64) -> myl_types::sitzung::Grenzen {
        myl_types::sitzung::Grenzen {
            budget,
            einzellimit: einzel,
            schwelle,
            zeugenleiter: Vec::new(),
        }
    }

    /// Inhaber 1, Agent 2, 1000 Credits Budget, 300 je Vorgang, keine
    /// Bestätigungsschwelle, Empfänger 10, Epochen 0 bis 100.
    fn kontrakt() -> Sitzungskontrakt {
        Sitzungskontrakt::neu(
            adresse(1),
            adresse(2),
            grenzen(1_000, 300, u64::MAX),
            myl_types::sitzung::Grenzen::gesperrt(),
            vec![adresse(10)],
            EpochId(0),
            EpochId(100),16,
        )
        .expect("gültiger Kontrakt")
    }

    fn vorhaben(sitzung: SitzungId, betrag: u64) -> Vorhaben {
        Vorhaben {
            sitzung,
            handelnder: adresse(2),
            waehrung: Waehrung::Credits,
            betrag,
            empfaenger: adresse(10),
            bestaetigt_ausgeliefert: false,
        }
    }

    /// Zustand mit Credits beim Inhaber und einer offenen Session.
    fn state_mit_sitzung(credits: u64) -> (LedgerState, SitzungId) {
        let mut state = LedgerState::genesis(1);
        state.account_mut(&adresse(1)).credits.push(InferenceCredit {
            owner: adresse(1),
            vtfe: credits,
            expiry: EpochId(1_000),
        });
        let id = sitzung_eroeffnen(&mut state, &adresse(1), kontrakt()).expect("eröffnen");
        (state, id)
    }

    #[test]
    fn eine_session_bucht_vom_konto_des_inhabers() {
        let (mut state, id) = state_mit_sitzung(500);
        sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 200)).expect("erlaubt");

        // Die Credits kommen vom Inhaber, nicht vom Agenten.
        assert_eq!(state.account(&adresse(1)).credits[0].vtfe, 300);
        assert!(state.account(&adresse(2)).credits.is_empty());
        assert_eq!(state.sitzung(&id).expect("da").zustand.verbraucht_credits, 200);
    }

    #[test]
    fn eine_session_kann_nicht_zweimal_unter_derselben_adresse_stehen() {
        let (mut state, _) = state_mit_sitzung(500);
        assert_eq!(
            sitzung_eroeffnen(&mut state, &adresse(1), kontrakt()),
            Err(TransitionError::SitzungExistiert)
        );
    }

    #[test]
    fn ein_abgelaufener_kontrakt_wird_gar_nicht_erst_eroeffnet() {
        let mut state = LedgerState::genesis(1);
        state.epoch = EpochId(101);
        assert_eq!(
            sitzung_eroeffnen(&mut state, &adresse(1), kontrakt()),
            Err(TransitionError::SitzungAbgelaufen)
        );
        assert!(state.sitzungen.is_empty());
    }

    /// ⚑ **Der zentrale Test der Phase.** Der Agent darf zahlen, aber
    /// nur innerhalb der Grenzen, und er kann die Grenzen nicht
    /// bewegen: Ein Kontrakt mit anderen Zahlen ist ein anderer
    /// Kontrakt und steht unter einer anderen Adresse.
    #[test]
    fn ein_agent_kommt_ueber_die_grenzen_nicht_hinaus() {
        let (mut state, id) = state_mit_sitzung(10_000);

        // Über dem Einzellimit.
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 301)),
            Err(TransitionError::KontraktVerbietet(
                Befund::EinzellimitUeberschritten { limit: 300 }
            ))
        );
        // An einen Empfänger, der nicht gelistet ist.
        let woanders = Vorhaben { empfaenger: adresse(99), ..vorhaben(id, 100) };
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &woanders),
            Err(TransitionError::KontraktVerbietet(Befund::EmpfaengerNichtGelistet))
        );
        // ⚑ Drei Wege, unter fremdem Namen zu handeln, und alle drei
        // sind zu. Erstens: Der Agent reicht ein Vorhaben ein, das
        // jemand anderen als Handelnden nennt.
        let fremder_name = Vorhaben { handelnder: adresse(3), ..vorhaben(id, 100) };
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &fremder_name),
            Err(TransitionError::NichtDerHandelnde)
        );
        // Zweitens, und das ist die Luecke, die der Einreicher-Vergleich
        // schliesst: Ein Fremder schreibt den **echten** Agenten ins
        // Feld. Der Kontrakt allein saehe daran nichts.
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(3), &vorhaben(id, 100)),
            Err(TransitionError::NichtDerHandelnde)
        );
        // Drittens: Einreicher und Feld stimmen ueberein, nur ist der
        // Genannte nicht der Agent dieses Kontrakts.
        let konsequent = Vorhaben { handelnder: adresse(3), ..vorhaben(id, 100) };
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(3), &konsequent),
            Err(TransitionError::KontraktVerbietet(Befund::FalscherHandelnder))
        );

        // Und der Inhaber kann keine Session eroeffnen, die ein fremdes
        // Konto belastet.
        let auf_fremde_rechnung = Sitzungskontrakt::neu(
            adresse(1),
            adresse(9),
            grenzen(1_000, 300, u64::MAX),
            myl_types::sitzung::Grenzen::gesperrt(),
            vec![adresse(10)],
            EpochId(0),
            EpochId(100),16,
        )
        .expect("gueltig");
        assert_eq!(
            sitzung_eroeffnen(&mut state, &adresse(9), auf_fremde_rechnung),
            Err(TransitionError::NichtDerInhaber)
        );

        // Und der Zustand ist durch all das unberührt geblieben.
        assert_eq!(state.account(&adresse(1)).credits[0].vtfe, 10_000);
        assert_eq!(state.sitzung(&id).expect("da").zustand.verbraucht_credits, 0);

        // Das Budget ist nach vier vollen Vorgängen erschöpft, obwohl
        // das Konto noch reichlich Credits trägt.
        for _ in 0..3 {
            sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 300)).expect("erlaubt");
        }
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 300)),
            Err(TransitionError::KontraktVerbietet(Befund::BudgetErschoepft { rest: 100 }))
        );
        sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 100)).expect("die letzten 100");
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 1)),
            Err(TransitionError::KontraktVerbietet(Befund::BudgetErschoepft { rest: 0 }))
        );
        assert_eq!(state.account(&adresse(1)).credits[0].vtfe, 9_000);
    }

    /// ⚑ Gegenprobe: Ein Kontrakt mit weiteren Grenzen ist kein
    /// geweiteter Kontrakt, sondern ein zweiter. Er kann eröffnet
    /// werden, aber er greift nicht auf die Session zu, unter der der
    /// Agent schon läuft.
    #[test]
    fn ein_zweiter_kontrakt_weitet_den_ersten_nicht() {
        let (mut state, id) = state_mit_sitzung(10_000);
        let weit = Sitzungskontrakt::neu(
            adresse(1),
            adresse(2),
            grenzen(1_000_000, 1_000_000, u64::MAX),
            myl_types::sitzung::Grenzen::gesperrt(),
            vec![adresse(10)],
            EpochId(0),
            EpochId(100),16,
        )
        .expect("gültig");
        let id2 = sitzung_eroeffnen(&mut state, &adresse(1), weit).expect("eröffnen");
        assert_ne!(id, id2);

        // Unter der alten Adresse gelten weiter die alten Grenzen.
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 5_000)),
            Err(TransitionError::KontraktVerbietet(
                Befund::EinzellimitUeberschritten { limit: 300 }
            ))
        );
        // Und der zweite Kontrakt braucht seine eigene Adresse im
        // Vorhaben; die alte zu nennen hilft nicht.
        assert_eq!(state.sitzung(&id).expect("da").kontrakt.credits.budget, 1_000);
    }

    #[test]
    fn nur_der_inhaber_widerruft_und_zweimal_schadet_nicht() {
        let (mut state, id) = state_mit_sitzung(500);
        assert_eq!(
            sitzung_widerrufen(&mut state, &id, &adresse(2)),
            Err(TransitionError::NichtDerInhaber)
        );
        sitzung_widerrufen(&mut state, &id, &adresse(1)).expect("Inhaber");
        sitzung_widerrufen(&mut state, &id, &adresse(1)).expect("nochmal, und das ist kein Fehler");
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 10)),
            Err(TransitionError::KontraktVerbietet(Befund::Widerrufen))
        );
        assert_eq!(
            sitzung_widerrufen(&mut state, &SitzungId::new([7u8; 32]), &adresse(1)),
            Err(TransitionError::SitzungUnbekannt)
        );
    }

    /// Der Kontrakt erlaubt 1000, das Konto trägt nur 50. Der Kontrakt
    /// ist eine Obergrenze und keine Deckungszusage.
    #[test]
    fn ein_erlaubtes_vorhaben_scheitert_trotzdem_an_leeren_credits() {
        let (mut state, id) = state_mit_sitzung(50);
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 100)),
            Err(TransitionError::InsufficientCredits { available: 50, required: 100 })
        );
        // ⚑ Und das Budget ist dabei **nicht** geschrumpft: Sonst wäre
        // ein Kontrakt über wiederholte Fehlschläge leerzuräumen.
        assert_eq!(state.sitzung(&id).expect("da").zustand.verbraucht_credits, 0);
    }

    #[test]
    fn eine_unbekannte_sitzung_zahlt_nicht() {
        let mut state = LedgerState::genesis(1);
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(SitzungId::new([9u8; 32]), 1)),
            Err(TransitionError::SitzungUnbekannt)
        );
    }

    // ---- Ueberweisung (Fund 83, seit 2026-08-28) ----

    #[test]
    fn eine_ueberweisung_bewegt_und_erzeugt_nichts() {
        let mut state = LedgerState::genesis(1);
        state.account_mut(&adresse(1)).balance = 1_000;
        state.account_mut(&adresse(2)).balance = 5;

        transfer(&mut state, &adresse(1), &adresse(2), 400).expect("gedeckt");
        assert_eq!(state.account(&adresse(1)).balance, 600);
        assert_eq!(state.account(&adresse(2)).balance, 405);

        // Die Summe ist erhalten: Kein Uebergang praegt hier MYL.
        let summe: u64 = state.accounts.values().map(|k| k.balance).sum();
        assert_eq!(summe, 1_005);
    }

    /// ⚑ Der Sonderfall, der still Geld erzeugte, wenn man ihn nicht
    /// abweist: erst lesen, dann beim Empfaenger addieren, dann beim
    /// Absender schreiben. Bei gleichem Konto verdoppelt das.
    #[test]
    fn eine_ueberweisung_an_sich_selbst_wird_abgewiesen() {
        let mut state = LedgerState::genesis(1);
        state.account_mut(&adresse(1)).balance = 100;
        assert_eq!(
            transfer(&mut state, &adresse(1), &adresse(1), 40),
            Err(TransitionError::SelbstUeberweisung)
        );
        assert_eq!(state.account(&adresse(1)).balance, 100);
    }

    #[test]
    fn eine_ungedeckte_ueberweisung_aendert_nichts() {
        let mut state = LedgerState::genesis(1);
        state.account_mut(&adresse(1)).balance = 100;
        state.account_mut(&adresse(1)).staked = 900; // gestaktes zaehlt nicht
        assert_eq!(
            transfer(&mut state, &adresse(1), &adresse(2), 500),
            Err(TransitionError::InsufficientBalance { available: 100, required: 500 })
        );
        assert_eq!(state.account(&adresse(1)).balance, 100);
        assert!(!state.accounts.contains_key(&adresse(2)));

        assert_eq!(
            transfer(&mut state, &adresse(1), &adresse(2), 0),
            Err(TransitionError::ZeroAmount)
        );
    }

    /// Der Ueberlauf wird **vor** jeder Buchung erkannt, sonst waere
    /// beim Fehlschlag Geld abgezogen und nicht angekommen.
    #[test]
    fn ein_ueberlauf_beim_empfaenger_nimmt_nichts_weg() {
        let mut state = LedgerState::genesis(1);
        state.account_mut(&adresse(1)).balance = 10;
        state.account_mut(&adresse(2)).balance = u64::MAX;
        assert_eq!(
            transfer(&mut state, &adresse(1), &adresse(2), 10),
            Err(TransitionError::Overflow)
        );
        assert_eq!(state.account(&adresse(1)).balance, 10);
        assert_eq!(state.account(&adresse(2)).balance, u64::MAX);
    }

    /// ⚑ Die Transaktionsnummer wird auch dann verbraucht, wenn die
    /// Anweisung danach scheitert. Sonst waere eine ungedeckte
    /// Ueberweisung unveraendert gueltig und beliebig oft einreichbar.
    #[test]
    fn die_nonce_geht_streng_der_reihe_nach() {
        let mut state = LedgerState::genesis(1);
        assert_eq!(state.account(&adresse(1)).nonce, 0);

        assert_eq!(
            nonce_verbrauchen(&mut state, &adresse(1), 1),
            Err(TransitionError::FalscheNonce { erwartet: 0, hatte: 1 })
        );
        nonce_verbrauchen(&mut state, &adresse(1), 0).expect("die erste");
        assert_eq!(state.account(&adresse(1)).nonce, 1);
        assert_eq!(
            nonce_verbrauchen(&mut state, &adresse(1), 0),
            Err(TransitionError::FalscheNonce { erwartet: 1, hatte: 0 }),
            "dieselbe Nummer darf nicht zweimal gelten"
        );
        nonce_verbrauchen(&mut state, &adresse(1), 1).expect("die zweite");
        assert_eq!(state.account(&adresse(1)).nonce, 2);

        // Konten zaehlen getrennt.
        assert_eq!(state.account(&adresse(2)).nonce, 0);
        nonce_verbrauchen(&mut state, &adresse(2), 0).expect("fremdes Konto faengt bei 0 an");
    }

    /// ⚑ Unter einem Kontrakt zahlt der Inhaber, und der Empfaenger
    /// bekommt es wirklich gutgeschrieben. Bis zum 2026-08-28 wies der
    /// Kontrakt jedes MYL-Vorhaben ab, weil es keine Ueberweisung gab.
    #[test]
    fn eine_session_zahlt_myl_an_den_gelisteten_empfaenger() {
        let mut state = LedgerState::genesis(1);
        state.account_mut(&adresse(1)).balance = 10_000;
        let k = Sitzungskontrakt::neu(
            adresse(1),
            adresse(2),
            myl_types::sitzung::Grenzen::gesperrt(),
            grenzen(1_000, 300, u64::MAX),
            vec![adresse(10)],
            EpochId(0),
            EpochId(100),16,
        )
        .expect("gueltig");
        let id = sitzung_eroeffnen(&mut state, &adresse(1), k).expect("eroeffnen");

        let zahlung =
            Vorhaben { waehrung: Waehrung::Myl, ..vorhaben(id, 250) };
        sitzung_ausgeben(&mut state, &adresse(2), &zahlung).expect("erlaubt");
        assert_eq!(state.account(&adresse(1)).balance, 9_750);
        assert_eq!(state.account(&adresse(10)).balance, 250);
        assert_eq!(state.sitzung(&id).expect("da").zustand.verbraucht_myl, 250);
        assert_eq!(state.sitzung(&id).expect("da").zustand.verbraucht_credits, 0);

        // Credits sind unter diesem Kontrakt gesperrt.
        assert_eq!(
            sitzung_ausgeben(&mut state, &adresse(2), &vorhaben(id, 1)),
            Err(TransitionError::KontraktVerbietet(
                Befund::EinzellimitUeberschritten { limit: 0 }
            ))
        );
    }

    #[test]
    fn aufgeraeumt_wird_erst_nach_der_nachfrist() {
        let (mut state, id) = state_mit_sitzung(500);
        state.epoch = EpochId(100 + SITZUNG_NACHFRIST);
        assert_eq!(sitzung_aufraeumen(&mut state), 0);
        assert!(state.sitzung(&id).is_some());

        state.epoch = EpochId(100 + SITZUNG_NACHFRIST + 1);
        assert_eq!(sitzung_aufraeumen(&mut state), 1);
        assert!(state.sitzung(&id).is_none());
        assert_eq!(sitzung_aufraeumen(&mut state), 0);
    }

    /// Sessions gehen in die Zustandsverpflichtung ein — sonst könnten
    /// zwei Knoten verschiedene Grenzen führen und trotzdem denselben
    /// Zustand behaupten.
    #[test]
    fn sitzungen_gehen_in_das_commitment_ein() {
        let (mit, _) = state_mit_sitzung(500);
        let mut ohne = LedgerState::genesis(1);
        ohne.account_mut(&adresse(1)).credits.push(InferenceCredit {
            owner: adresse(1),
            vtfe: 500,
            expiry: EpochId(1_000),
        });
        assert_ne!(mit.commitment(), ohne.commitment());

        // Und der Verbrauch zählt mit.
        let mut nachher = mit.clone();
        let id = *nachher.sitzungen.keys().next().expect("eine");
        sitzung_ausgeben(&mut nachher, &adresse(2), &vorhaben(id, 10)).expect("erlaubt");
        assert_ne!(mit.commitment(), nachher.commitment());
    }

    // ── Speicherregister (STORAGE D7) ───────────────────────────────

    fn manifest_mit(art: myl_types::gegenstand::Gegenstandsart) -> Manifest {
        use myl_types::gegenstand::{teile_bilden, Redundanzform};
        let teile = teile_bilden(b"ein kleiner Gegenstand").expect("Teile");
        Manifest::neu(art, 1, &teile, Redundanzform::Kopien { anzahl: 3 }).expect("Manifest")
    }

    #[test]
    fn ein_shardgewicht_kommt_in_den_zustand() {
        use myl_types::gegenstand::Gegenstandsart;
        let mut st = LedgerState::genesis(1);
        let m = manifest_mit(Gegenstandsart::Shardgewichte);
        let w = m.wurzel;
        assert_eq!(speicher_aufnehmen(&mut st, m.clone(), w), Ok(()));
        assert_eq!(st.speicher.get(&w), Some(&m));
    }

    /// ⚑ **Die Grenze aus D7, und sie wird erzwungen statt beschrieben.**
    ///
    /// Beide Wissensklassen gehören über eine Wurzel, und zwar aus
    /// verschiedenen Gründen dieselbe Folge: Ein Wissensstück zahlt sein
    /// Einleger, Netzwerkwissen trägt die Allgemeinheit, **aber beide
    /// wachsen mit der Nutzung**. `commitment()` serialisiert den ganzen
    /// Zustand; jeder Block zahlte sonst für die ganze Datenbank.
    #[test]
    fn wissen_gehoert_nicht_einzeln_in_den_zustand() {
        use myl_types::gegenstand::Gegenstandsart;
        for art in [Gegenstandsart::Wissensstueck, Gegenstandsart::Netzwerkwissen] {
            let mut st = LedgerState::genesis(1);
            let m = manifest_mit(art);
            let w = m.wurzel;
            assert_eq!(
                speicher_aufnehmen(&mut st, m, w),
                Err(TransitionError::GegenstandUeberWurzel),
                "{art:?} kam in den Zustand"
            );
            assert!(st.speicher.is_empty(), "{art:?} hinterliess einen Eintrag");
        }
    }

    #[test]
    fn derselbe_gegenstand_zweimal_wird_abgewiesen() {
        use myl_types::gegenstand::Gegenstandsart;
        let mut st = LedgerState::genesis(1);
        let m = manifest_mit(Gegenstandsart::Skalenpaket);
        let w = m.wurzel;
        speicher_aufnehmen(&mut st, m.clone(), w).expect("erste Aufnahme");
        assert_eq!(
            speicher_aufnehmen(&mut st, m, w),
            Err(TransitionError::GegenstandExistiert)
        );
    }

    /// Ein Eintrag, dessen Schlüssel nicht zu seinem Inhalt passt, wäre
    /// später von außen nicht mehr auffindbar.
    #[test]
    fn eine_fremde_wurzel_wird_abgewiesen() {
        use myl_types::gegenstand::Gegenstandsart;
        let mut st = LedgerState::genesis(1);
        let m = manifest_mit(Gegenstandsart::Shardgewichte);
        let fremd = MerkleRoot::new([9u8; 32]);
        assert_ne!(m.wurzel, fremd);
        assert_eq!(
            speicher_aufnehmen(&mut st, m, fremd),
            Err(TransitionError::WurzelPasstNicht)
        );
    }

    #[test]
    fn entfernen_geht_nur_was_dasteht() {
        use myl_types::gegenstand::Gegenstandsart;
        let mut st = LedgerState::genesis(1);
        let m = manifest_mit(Gegenstandsart::Sonstiges);
        let w = m.wurzel;
        assert_eq!(
            speicher_entfernen(&mut st, &w),
            Err(TransitionError::GegenstandUnbekannt)
        );
        speicher_aufnehmen(&mut st, m.clone(), w).expect("Aufnahme");
        assert_eq!(speicher_entfernen(&mut st, &w), Ok(m));
        assert!(st.speicher.is_empty());
    }

    /// ⚑ **Das Register ist Konsensgegenstand, nicht Beiwerk.**
    ///
    /// Änderte es den Zustandshash nicht, wären sich zwei Knoten über
    /// den Inhalt einig, ohne es zu sein: Der eine hätte das
    /// Shardgewicht, der andere nicht, und beide meldeten dieselbe
    /// Zustandswurzel.
    #[test]
    fn das_register_geht_in_den_zustandshash_ein() {
        use myl_types::gegenstand::Gegenstandsart;
        let leer = LedgerState::genesis(1);
        let mut mit = LedgerState::genesis(1);
        let m = manifest_mit(Gegenstandsart::Shardgewichte);
        let w = m.wurzel;
        speicher_aufnehmen(&mut mit, m, w).expect("Aufnahme");
        assert_ne!(leer.commitment(), mit.commitment());

        speicher_entfernen(&mut mit, &w).expect("Entfernen");
        assert_eq!(leer.commitment(), mit.commitment(), "nicht rueckstandsfrei");
    }

    // ── Auszahlungskonto (Entscheidung 2026-08-31) ──────────────────

    fn miner(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }

    #[test]
    fn der_miner_traegt_sich_erstmals_selbst_ein() {
        let mut st = LedgerState::genesis(1);
        let m = miner(1);
        let selbst = Address::new([1u8; 32]);
        let kalt = Address::new([9u8; 32]);
        assert_eq!(auszahlungskonto_eintragen(&mut st, &selbst, &m, kalt), Ok(()));
        assert_eq!(auszahlungskonto(&st, &m), Some(kalt));
    }

    /// ⚑ **Ein Fremder trägt nicht für einen Miner ein.**
    ///
    /// Ohne diese Prüfung könnte jeder das Auszahlungskonto eines
    /// anderen setzen und dessen Ertrag zu sich lenken.
    #[test]
    fn ein_fremder_traegt_nicht_ein() {
        let mut st = LedgerState::genesis(1);
        let m = miner(1);
        let fremd = Address::new([7u8; 32]);
        assert_eq!(
            auszahlungskonto_eintragen(&mut st, &fremd, &m, Address::new([9u8; 32])),
            Err(TransitionError::NichtDasAuszahlungskonto)
        );
        assert_eq!(auszahlungskonto(&st, &m), None);
    }

    /// ⚑ **Der Kern der Entscheidung: Nach der ersten Eintragung
    /// gehört die Änderung dem kalten Konto.**
    ///
    /// Ein gestohlener Konsensschlüssel kann den Ertrag damit nicht
    /// umleiten. Das ist der Fehler, den Ethereum als
    /// Auszahlungsnachweis `0x00` gemacht und später ökosystemweit
    /// korrigiert hat.
    #[test]
    fn nach_der_ersten_eintragung_darf_nur_noch_das_konto() {
        let mut st = LedgerState::genesis(1);
        let m = miner(1);
        let selbst = Address::new([1u8; 32]);
        let kalt = Address::new([9u8; 32]);
        auszahlungskonto_eintragen(&mut st, &selbst, &m, kalt).expect("erste Eintragung");

        // Der heisse Schluessel des Miners darf jetzt nicht mehr.
        assert_eq!(
            auszahlungskonto_eintragen(&mut st, &selbst, &m, Address::new([6u8; 32])),
            Err(TransitionError::NichtDasAuszahlungskonto),
            "der heisse Schluessel konnte den Ertrag umleiten"
        );
        assert_eq!(auszahlungskonto(&st, &m), Some(kalt));

        // Das kalte Konto darf.
        let neu = Address::new([5u8; 32]);
        assert_eq!(auszahlungskonto_eintragen(&mut st, &kalt, &m, neu), Ok(()));
        assert_eq!(auszahlungskonto(&st, &m), Some(neu));
    }

    /// Ohne Eintrag kein Konto, und damit kein Anteil.
    #[test]
    fn ohne_eintrag_gibt_es_kein_konto() {
        let st = LedgerState::genesis(1);
        assert_eq!(auszahlungskonto(&st, &miner(3)), None);
    }

    /// Das Register geht in den Zustandshash ein, sonst wären sich zwei
    /// Knoten über die Empfänger einig, ohne es zu sein.
    #[test]
    fn das_auszahlungsregister_geht_in_den_zustandshash_ein() {
        let leer = LedgerState::genesis(1);
        let mut mit = LedgerState::genesis(1);
        auszahlungskonto_eintragen(
            &mut mit,
            &Address::new([1u8; 32]),
            &miner(1),
            Address::new([9u8; 32]),
        )
        .expect("Eintragung");
        assert_ne!(leer.commitment(), mit.commitment());
    }

    /// Prägung erhöht das Guthaben, und zwar genau um den Betrag.
    #[test]
    fn praegung_erreicht_das_konto() {
        let mut state = LedgerState::genesis(1);
        let konto = adresse(7);
        praegen(&mut state, &konto, 4_200).expect("Praegung");
        assert_eq!(state.account(&konto).balance, 4_200);
        praegen(&mut state, &konto, 800).expect("zweite Praegung");
        assert_eq!(state.account(&konto).balance, 5_000);
    }

    /// Gegenprobe: null zu prägen ist ein Fehler und kein stilles
    /// Nichtstun. Wer null prägt, hat sich verrechnet.
    #[test]
    fn null_zu_praegen_ist_ein_fehler() {
        let mut state = LedgerState::genesis(1);
        assert_eq!(
            praegen(&mut state, &adresse(1), 0),
            Err(TransitionError::ZeroAmount)
        );
    }

    /// Gegenprobe: ein Überlauf wird gemeldet, nicht gesättigt. Eine
    /// gesättigte Prägung wäre stillschweigend eine andere Geldmenge.
    #[test]
    fn ueberlauf_wird_gemeldet_und_nicht_gesaettigt() {
        let mut state = LedgerState::genesis(1);
        let konto = adresse(2);
        state.account_mut(&konto).balance = u64::MAX - 5;
        assert_eq!(
            praegen(&mut state, &konto, 6),
            Err(TransitionError::Overflow)
        );
        assert_eq!(
            state.account(&konto).balance,
            u64::MAX - 5,
            "der Zustand wurde trotz Fehler veraendert"
        );
    }

    /// Die Prägung geht in die Zustandsverpflichtung ein.
    #[test]
    fn praegung_veraendert_das_commitment() {
        let vorher = LedgerState::genesis(1);
        let mut nachher = LedgerState::genesis(1);
        praegen(&mut nachher, &adresse(3), 1).expect("Praegung");
        assert_ne!(vorher.commitment(), nachher.commitment());
    }

    // --- Punkt 40, Glied 3a: das Miner-Register ---

    /// ⚑ **Echte Schlüssel, nicht erfundene Bytes.**
    ///
    /// Die Anmeldung prüft seit dem 2026-09-01, dass die Kennung aus dem
    /// Schlüssel folgt. `MinerId::new([b; 32])` konnte das nie
    /// erfüllen, denn zu einem gewürfelten Hash gibt es kein Urbild;
    /// **ein Test damit hätte belegt, dass die Prüfung nicht greift**.
    fn probeschluessel(b: u8) -> myl_types::bls::BlsPublicKey {
        let sk = myl_types::bls::BlsSecretKey::key_gen(&[b.wrapping_add(1); 32])
            .expect("Schluessel");
        sk.public_key().expect("pk")
    }

    fn kennung(b: u8) -> MinerId {
        MinerId::aus_schluessel(&probeschluessel(b))
    }

    /// Die Adresse desselben Schlüssels. Kennung und Adresse sind
    /// verschiedene Typen über denselben Bytes.
    fn miner_adresse(b: u8) -> Address {
        Address::aus_schluessel(&probeschluessel(b))
    }

    /// Eine Anmeldung steht danach im Register, mit der laufenden Epoche.
    #[test]
    fn eine_anmeldung_steht_im_register() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(7);
        miner_anmelden(&mut st, &miner_adresse(3), &kennung(3), HardwareClass::MediumGpu, GeoRegion::Europe, probeschluessel(3))
            .expect("Anmeldung");
        let eintrag = st.miner.get(&kennung(3)).expect("eingetragen");
        assert_eq!(eintrag.hardware_class, HardwareClass::MediumGpu);
        assert_eq!(eintrag.registration_epoch, 7);
    }

    /// ⚑ **Die Registrierungsepoche setzt die Kette.** Ein selbst
    /// gewähltes Datum hübe den Registrierungsschluss auf.
    #[test]
    fn die_epoche_kommt_aus_dem_zustand() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(42);
        miner_anmelden(&mut st, &miner_adresse(1), &kennung(1), HardwareClass::SmallGpu, GeoRegion::Europe, probeschluessel(1))
            .expect("Anmeldung");
        assert_eq!(st.miner[&kennung(1)].registration_epoch, 42);
    }

    /// ⚑ **Eine Klassenänderung behält das Datum.** Sonst machte sie den
    /// Miner jünger und damit für die nächste Zuteilung unqualifiziert.
    #[test]
    fn eine_klassenaenderung_behaelt_das_datum() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(5);
        miner_anmelden(&mut st, &miner_adresse(1), &kennung(1), HardwareClass::SmallGpu, GeoRegion::Europe, probeschluessel(1))
            .expect("erste");
        st.epoch = EpochId(50);
        miner_anmelden(&mut st, &miner_adresse(1), &kennung(1), HardwareClass::LargeGpu, GeoRegion::Europe, probeschluessel(1))
            .expect("zweite");
        let eintrag = st.miner[&kennung(1)];
        assert_eq!(eintrag.hardware_class, HardwareClass::LargeGpu);
        assert_eq!(eintrag.registration_epoch, 5, "das Datum wanderte mit");
    }

    /// Gegenprobe: Ein Fremder meldet niemanden an.
    #[test]
    fn ein_fremder_meldet_niemanden_an() {
        let mut st = LedgerState::genesis(1);
        assert_eq!(
            miner_anmelden(&mut st, &miner_adresse(9), &kennung(1), HardwareClass::SmallGpu, GeoRegion::Europe, probeschluessel(1)),
            Err(TransitionError::NichtDerMiner)
        );
        assert!(st.miner.is_empty(), "der Fremde hat eingetragen");
    }

    /// Die Abmeldung wirkt sofort.
    #[test]
    fn eine_abmeldung_wirkt_sofort() {
        let mut st = LedgerState::genesis(1);
        miner_anmelden(&mut st, &miner_adresse(1), &kennung(1), HardwareClass::SmallGpu, GeoRegion::Europe, probeschluessel(1))
            .expect("Anmeldung");
        miner_abmelden(&mut st, &miner_adresse(1), &kennung(1)).expect("Abmeldung");
        assert!(st.miner.is_empty());
    }

    /// Gegenprobe: Ein Fremder meldet niemanden ab.
    #[test]
    fn ein_fremder_meldet_niemanden_ab() {
        let mut st = LedgerState::genesis(1);
        miner_anmelden(&mut st, &miner_adresse(1), &kennung(1), HardwareClass::SmallGpu, GeoRegion::Europe, probeschluessel(1))
            .expect("Anmeldung");
        assert_eq!(
            miner_abmelden(&mut st, &miner_adresse(9), &kennung(1)),
            Err(TransitionError::NichtDerMiner)
        );
        assert_eq!(st.miner.len(), 1, "der Fremde hat abgemeldet");
    }

    /// Wer nicht angemeldet ist, kann sich nicht abmelden, und das wird
    /// benannt statt stillschweigend hingenommen.
    #[test]
    fn ein_unbekannter_kann_sich_nicht_abmelden() {
        let mut st = LedgerState::genesis(1);
        assert_eq!(
            miner_abmelden(&mut st, &miner_adresse(1), &kennung(1)),
            Err(TransitionError::MinerUnbekannt)
        );
    }

    /// ⚑ **Die Liste ist kanonisch geordnet**, denn daran hängt die
    /// Pod-Bildung: Zwei Knoten mit verschiedener Reihenfolge kämen zu
    /// verschiedenen Zuteilungen, ohne dass etwas kaputt wäre.
    #[test]
    fn die_minerliste_ist_kanonisch_geordnet() {
        let mut st = LedgerState::genesis(1);
        for b in [9u8, 2, 7, 1] {
            miner_anmelden(&mut st, &miner_adresse(b), &kennung(b), HardwareClass::SmallGpu, GeoRegion::Europe, probeschluessel(b))
                .expect("Anmeldung");
        }
        let ids: Vec<MinerId> = angemeldete_miner(&st).iter().map(|r| r.miner_id).collect();
        let mut sortiert = ids.clone();
        sortiert.sort();
        assert_eq!(ids, sortiert);
    }

    /// Das Register geht in die Zustandsverpflichtung ein.
    #[test]
    fn das_register_veraendert_das_commitment() {
        let leer = LedgerState::genesis(1);
        let mut mit = LedgerState::genesis(1);
        miner_anmelden(&mut mit, &miner_adresse(1), &kennung(1), HardwareClass::SmallGpu, GeoRegion::Europe, probeschluessel(1))
            .expect("Anmeldung");
        assert_ne!(leer.commitment(), mit.commitment());
    }

    // --- Punkt 40, Glied 1: das Bündel in der Kette ---

    fn pod(b: u8) -> myl_types::ids::PodId {
        myl_types::ids::PodId::new([b; 32])
    }

    fn buendel(p: u8, epoche: u64, vtfe: u64) -> PoIBundle {
        PoIBundle {
            epoch: EpochId(epoche),
            pod: pod(p),
            segments_root: MerkleRoot::new([7; 32]),
            vtfe_claimed: vtfe,
            aggregate_sig: myl_types::bls::BlsSignature([0; 96]),
        }
    }

    fn angemeldet(st: &mut LedgerState, b: u8) {
        miner_anmelden(
            st,
            &miner_adresse(b),
            &kennung(b),
            HardwareClass::MediumGpu,
            GeoRegion::Europe,
            probeschluessel(b),
        )
            .expect("Anmeldung");
    }

    /// Ein Bündel eines angemeldeten Miners für die laufende Epoche
    /// wird aufgenommen.
    #[test]
    fn ein_buendel_wird_aufgenommen() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(4);
        angemeldet(&mut st, 1);
        buendel_einreichen(&mut st, &miner_adresse(1), buendel(9, 4, 500)).expect("Einreichung");
        assert_eq!(buendel_der_epoche(&st).len(), 1);
        assert_eq!(st.buendel[&pod(9)].vtfe_claimed, 500);
    }

    /// Gegenprobe: Wer nicht angemeldet ist, reicht nichts ein.
    #[test]
    fn ein_unangemeldeter_reicht_nichts_ein() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(4);
        assert_eq!(
            buendel_einreichen(&mut st, &miner_adresse(1), buendel(9, 4, 500)),
            Err(TransitionError::MinerUnbekannt)
        );
        assert!(st.buendel.is_empty());
    }

    /// ⚑ Gegenprobe: Ein Bündel einer anderen Epoche gilt nicht. Ohne
    /// diese Schranke ließe sich Arbeit aus einer alten Epoche in einer
    /// neuen abrechnen.
    #[test]
    fn ein_buendel_fremder_epoche_gilt_nicht() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(4);
        angemeldet(&mut st, 1);
        assert_eq!(
            buendel_einreichen(&mut st, &miner_adresse(1), buendel(9, 3, 500)),
            Err(TransitionError::FremdeEpoche {
                buendel: EpochId(3),
                laufend: EpochId(4)
            })
        );
    }

    /// ⚑ Gegenprobe: Zweimal für denselben Pod geht nicht. Sonst
    /// rechnete ein Pod seine Arbeit mehrfach ab.
    #[test]
    fn zweimal_fuer_denselben_pod_geht_nicht() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(4);
        angemeldet(&mut st, 1);
        buendel_einreichen(&mut st, &miner_adresse(1), buendel(9, 4, 500)).expect("erste");
        assert_eq!(
            buendel_einreichen(&mut st, &miner_adresse(1), buendel(9, 4, 900)),
            Err(TransitionError::BuendelExistiert)
        );
        assert_eq!(st.buendel[&pod(9)].vtfe_claimed, 500, "das zweite hat ueberschrieben");
    }

    /// Verschiedene Pods dürfen nebeneinander stehen.
    #[test]
    fn verschiedene_pods_stehen_nebeneinander() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(4);
        angemeldet(&mut st, 1);
        for p in [1u8, 5, 9] {
            buendel_einreichen(&mut st, &miner_adresse(1), buendel(p, 4, 100)).expect("Einreichung");
        }
        assert_eq!(buendel_der_epoche(&st).len(), 3);
    }

    /// ⚑ **Das Leeren gehört zum Abschluss.** Ohne es wüchse der
    /// Zustand unbegrenzt, und D7 wäre gebrochen.
    #[test]
    fn das_leeren_gibt_die_zahl_zurueck() {
        let mut st = LedgerState::genesis(1);
        st.epoch = EpochId(4);
        angemeldet(&mut st, 1);
        for p in [1u8, 5] {
            buendel_einreichen(&mut st, &miner_adresse(1), buendel(p, 4, 100)).expect("Einreichung");
        }
        assert_eq!(buendel_leeren(&mut st), 2);
        assert!(st.buendel.is_empty());
        assert_eq!(buendel_leeren(&mut st), 0, "das zweite Leeren fand noch etwas");
    }

    /// Bündel gehen in die Zustandsverpflichtung ein.
    #[test]
    fn ein_buendel_veraendert_das_commitment() {
        let mut ohne = LedgerState::genesis(1);
        ohne.epoch = EpochId(4);
        angemeldet(&mut ohne, 1);
        let mut mit = ohne.clone();
        buendel_einreichen(&mut mit, &miner_adresse(1), buendel(9, 4, 500)).expect("Einreichung");
        assert_ne!(ohne.commitment(), mit.commitment());
    }

    // --- Punkt 40, letztes Glied: die Arbeitsverteilung ---

    fn verteilung(stand: u8, gewichte: Vec<u64>) -> Arbeitsverteilung {
        Arbeitsverteilung::neu(myl_types::Hash::sha256(&[stand; 4]), gewichte)
            .expect("Verteilung")
    }

    /// Eine Verteilung steht danach im Zustand.
    #[test]
    fn eine_verteilung_steht_im_zustand() {
        let mut st = LedgerState::genesis(1);
        arbeitsverteilung_setzen(&mut st, verteilung(1, vec![3, 1, 1, 5])).expect("setzen");
        let v = st.arbeitsverteilung.as_ref().expect("gesetzt");
        assert_eq!(v.gewichte(), &[3, 1, 1, 5]);
    }

    /// ⚑ **Derselbe Pipeline-Stand bekommt keine zweite Gewichtung.**
    /// Sonst folgten die Gewichte nicht aus dem Stand, sondern wären
    /// frei wählbar.
    #[test]
    fn derselbe_stand_bekommt_keine_zweite_gewichtung() {
        let mut st = LedgerState::genesis(1);
        arbeitsverteilung_setzen(&mut st, verteilung(1, vec![1, 1])).expect("erste");
        assert_eq!(
            arbeitsverteilung_setzen(&mut st, verteilung(1, vec![9, 1])),
            Err(TransitionError::VerteilungExistiert)
        );
        assert_eq!(
            st.arbeitsverteilung.as_ref().expect("gesetzt").gewichte(),
            &[1, 1],
            "die zweite hat ueberschrieben"
        );
    }

    /// Ein anderer Stand darf anders gewichten, und der Wechsel ist
    /// sichtbar.
    #[test]
    fn ein_anderer_stand_darf_anders_gewichten() {
        let mut st = LedgerState::genesis(1);
        arbeitsverteilung_setzen(&mut st, verteilung(1, vec![1, 1])).expect("erste");
        arbeitsverteilung_setzen(&mut st, verteilung(2, vec![9, 1])).expect("zweite");
        assert_eq!(
            st.arbeitsverteilung.as_ref().expect("gesetzt").gewichte(),
            &[9, 1]
        );
    }

    /// Die Verteilung geht in die Zustandsverpflichtung ein.
    #[test]
    fn die_verteilung_veraendert_das_commitment() {
        let ohne = LedgerState::genesis(1);
        let mut mit = LedgerState::genesis(1);
        arbeitsverteilung_setzen(&mut mit, verteilung(1, vec![1])).expect("setzen");
        assert_ne!(ohne.commitment(), mit.commitment());
    }

    /// ⚑ **Ein Schlüssel, der nicht zur Kennung gehört, wird
    /// abgewiesen.** Sonst trüge das Register einen fremden Schlüssel
    /// unter dieser Kennung, und die Aggregatprüfung eines Pods liefe
    /// gegen den falschen.
    #[test]
    fn ein_fremder_schluessel_wird_abgewiesen() {
        let mut st = LedgerState::genesis(1);
        assert_eq!(
            miner_anmelden(
                &mut st,
                &miner_adresse(1),
                &kennung(1),
                HardwareClass::SmallGpu,
                GeoRegion::Europe,
                probeschluessel(2),
            ),
            Err(TransitionError::SchluesselPasstNicht)
        );
        assert!(st.miner.is_empty());
    }

    /// Der eingetragene Schlüssel ist der, mit dem unterschrieben wurde.
    #[test]
    fn der_eingetragene_schluessel_ist_der_unterschreibende() {
        let mut st = LedgerState::genesis(1);
        angemeldet(&mut st, 3);
        assert_eq!(st.miner[&kennung(3)].schluessel, probeschluessel(3));
    }
}
