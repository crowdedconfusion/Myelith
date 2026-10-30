//! **Bezahlen aus dem Wallet** (2026-10-26): Credits kaufen, einen
//! Sitzungskontrakt für einen Agenten eröffnen, ihn widerrufen.
//!
//! # ⚑ Die Rollen (nach Ethereum, NEAR und ERC-7715)
//!
//! - Das **Wallet** ist Inhaber: Es unterschreibt Burn, Eröffnen und
//!   Widerruf mit seinem eigenen Schlüssel.
//! - Der **Agent** hat einen eigenen Schlüssel, getrennt vom Wallet. Mit ihm
//!   stellt er sich eine **Vollmacht** aus (ein Bearer-Token), die nur für
//!   diese Sitzung und bis zu ihrem Ende gilt. Die Tür nimmt den Token, die
//!   Kette prüft Budget und Einzellimit des Kontrakts. Den Schlüssel des
//!   Wallets bekommt der Agent nie.
//! - Der **Betreiber** der Tür steht als einziger Empfänger im Kontrakt;
//!   der Beweis dafür reist in der Vollmacht mit.
//!
//! ⚠️ **Die Summe der Budgets ist die Grenze**, nicht jedes einzeln: Zwei
//! Sitzungen für denselben Agenten geben ihm zusammen beide Budgets.
//!
//! # ⚑ Der Weg ins Netz
//!
//! Über die Relaisleitung eines Knotens auf dieser Maschine
//! ([`Knotenleitung`]): Er nennt Nonce und Netzwert, prüft jede
//! Transaktion und verbreitet sie. Im Betrieb ist das der Knoten des
//! Minings ([`eigene_leitung`]).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use myl_consensus::block::{Anweisung, Transaktion};
use myl_types::relais::{Kontostand, Relaisantwort, Relaisfrage};
use myl_types::ids::SitzungId;
use myl_types::sitzung::{Grenzen, Sitzungskontrakt};
use myl_types::zeitprobe::AUSWEIS_LEN;

use crate::wallet::{self, Address, Wallet};

/// Die Relaisleitung eines Knotens: Adresse und Ausweis.
#[derive(Debug, Clone, Copy)]
pub struct Knotenleitung {
    pub adresse: SocketAddr,
    pub ausweis: [u8; AUSWEIS_LEN],
}

impl Knotenleitung {
    fn fragen(&self, frage: &Relaisfrage) -> Result<Relaisantwort, String> {
        myl_types::relais::fragen(self.adresse, &self.ausweis, frage, Duration::from_secs(30))
    }

    /// Was die Kette über ein Konto weiß.
    pub fn kontostand(&self, adresse: &Address) -> Result<Kontostand, String> {
        match self.fragen(&Relaisfrage::Konto { adresse: *adresse })? {
            Relaisantwort::Konto(k) => Ok(k),
            Relaisantwort::Fehler(f) => Err(f),
            andere => Err(format!("unerwartete Antwort: {andere:?}")),
        }
    }
}

/// **Die Leitung des eigenen Mining-Knotens**, falls er läuft.
pub fn eigene_leitung() -> Result<Knotenleitung, String> {
    let z = crate::mining::zustand().ok_or("Mining läuft nicht: Bezahlen braucht den eigenen Knoten.")?;
    let adresse = z.relais.as_deref().ok_or("Der Knoten läuft ohne Relaisleitung; Mining neu starten.")?;
    let adresse = adresse.parse().map_err(|_| format!("keine Adresse: {adresse}"))?;
    let ausweis = myl_types::relais::ausweis_lesen(&crate::mining::datenordner().join(crate::mining::RELAISAUSWEIS))
        .map_err(|e| e.to_string())?;
    Ok(Knotenleitung { adresse, ausweis })
}

/// **Unterschreibt eine Anweisung mit dem Wallet und gibt sie ins Netz.**
pub fn senden(leitung: &Knotenleitung, w: &Wallet, anweisung: Anweisung) -> Result<String, String> {
    let stand = leitung.kontostand(&w.adresse)?;
    senden_mit(leitung, w, &stand, anweisung)
}

/// ⚑ **So viel mehr als die Gebühr von jetzt sagt das Wallet zu** (G1):
/// das Doppelte. Der Grundpreis steigt je vollem Block um höchstens ein
/// Achtel; das Doppelte trägt über rund sechs volle Blöcke in Folge, wie
/// bei Wallets nach EIP-1559 üblich. Abgebucht wird nur die Gebühr des
/// Blocks, nicht die Zusage.
pub const GEBUEHR_SPIELRAUM: u64 = 2;

/// Die Gebühr einer Anweisung beim Grundpreis von `stand`: Grundpreis mal
/// Bytes der kodierten Transaktion. Die Länge hängt nicht an Schlüssel,
/// Nonce oder Unterschrift, die alle feste Breite haben.
pub fn gebuehr(stand: &Kontostand, anweisung: &Anweisung) -> u64 {
    let muster = Transaktion {
        absender: myl_types::bls::BlsPublicKey([0; myl_types::bls::BLS_PK_LEN]),
        nonce: 0,
        max_gebuehr: 0,
        anweisung: anweisung.clone(),
        signatur: myl_types::bls::BlsSignature([0; myl_types::bls::BLS_SIG_LEN]),
    };
    stand.grundpreis_je_byte.saturating_mul(muster.gebuehreneinheiten())
}

/// Wie [`senden`], mit einem schon gelesenen Stand: unterschrieben wird
/// gegen genau dessen Nonce und Netzwert, mit der Zusage
/// [`GEBUEHR_SPIELRAUM`] mal der Gebühr von jetzt.
fn senden_mit(leitung: &Knotenleitung, w: &Wallet, stand: &Kontostand, anweisung: Anweisung) -> Result<String, String> {
    let geheim = wallet::geheimschluessel(&w.name)?;
    let max_gebuehr = gebuehr(stand, &anweisung).saturating_mul(GEBUEHR_SPIELRAUM);
    let tx = Transaktion::signiere_mit_grenze(&stand.netzwert, &geheim, stand.nonce, max_gebuehr, anweisung)
        .map_err(|e| format!("unterschreiben: {e:?}"))?;
    let bytes = borsh::to_vec(&tx).map_err(|e| e.to_string())?;
    match leitung.fragen(&Relaisfrage::Transaktion { tx: bytes })? {
        Relaisantwort::Erledigt(z) => Ok(z),
        Relaisantwort::Fehler(f) => Err(f),
        andere => Err(format!("unerwartete Antwort: {andere:?}")),
    }
}

/// **Was eine Überweisung tun wird**, bevor unterschrieben wird
/// (2026-10-27).
///
/// ⚑ **Unterschrieben wird, was gezeigt wurde.** [`ueberweisen`] nimmt diese
/// Vorschau und bricht ab, wenn sich der Stand des Kontos inzwischen
/// geändert hat; sonst stünde in der Vorschau eine andere Nonce als in der
/// Unterschrift.
#[derive(Debug, Clone)]
pub struct Ueberweisungsvorschau {
    pub von: Address,
    pub an: Address,
    pub betrag: u64,
    pub guthaben: u64,
    /// Die Gebühr beim Grundpreis von jetzt (G1) ...
    pub gebuehr: u64,
    /// ... und höchstens so viel, wenn er bis zum Block steigt.
    pub max_gebuehr: u64,
    /// Der Aufschlag, wenn der Empfänger ein neues Konto ist, sonst null.
    pub aufschlag: u64,
    stand: Kontostand,
}

impl Ueberweisungsvorschau {
    /// Die Nonce, unter der unterschrieben wird.
    pub fn nonce(&self) -> u64 {
        self.stand.nonce
    }
    /// Der Netzwert, gegen den unterschrieben wird.
    pub fn netzwert(&self) -> myl_types::hash::Hash {
        self.stand.netzwert
    }
}

/// **Liest den Stand und prüft die Überweisung**, ohne zu unterschreiben:
/// Betrag über null, Empfänger nicht der Absender, Guthaben deckt Betrag,
/// Gebühr und, an ein neues Konto, den Aufschlag.
pub fn ueberweisung_vorschauen(leitung: &Knotenleitung, w: &Wallet, an: Address, betrag: u64) -> Result<Ueberweisungsvorschau, String> {
    if betrag == 0 {
        return Err("Ein Betrag von null ist keine Überweisung.".into());
    }
    if an == w.adresse {
        return Err("Empfänger und Absender sind dasselbe Konto.".into());
    }
    let stand = leitung.kontostand(&w.adresse)?;
    let empfaenger = leitung.kontostand(&an)?;
    let aufschlag = if empfaenger.bekannt { 0 } else { empfaenger.aufschlag_konto };
    let gebuehr = gebuehr(&stand, &Anweisung::Ueberweisung { nach: an, betrag });
    let max_gebuehr = gebuehr.saturating_mul(GEBUEHR_SPIELRAUM);
    let noetig = betrag.saturating_add(gebuehr).saturating_add(aufschlag);
    if stand.guthaben < noetig {
        return Err(format!(
            "Das Guthaben ({} Kleinstbeträge) deckt {betrag} samt Gebühr {gebuehr} und Aufschlag {aufschlag} nicht.",
            stand.guthaben
        ));
    }
    Ok(Ueberweisungsvorschau { von: w.adresse, an, betrag, guthaben: stand.guthaben, gebuehr, max_gebuehr, aufschlag, stand })
}

/// **Überweist, wie die Vorschau es zeigte.** Hat sich der Stand des
/// Kontos seither geändert (Nonce oder Netz), wird nichts unterschrieben.
pub fn ueberweisen(leitung: &Knotenleitung, w: &Wallet, vorschau: &Ueberweisungsvorschau) -> Result<String, String> {
    if vorschau.von != w.adresse {
        return Err("Die Vorschau gehört zu einem anderen Wallet.".into());
    }
    let jetzt = leitung.kontostand(&w.adresse)?;
    if jetzt.nonce != vorschau.stand.nonce || jetzt.netzwert != vorschau.stand.netzwert {
        return Err("Der Stand des Kontos hat sich seit der Vorschau geändert; bitte neu ansehen.".into());
    }
    senden_mit(leitung, w, &vorschau.stand, Anweisung::Ueberweisung { nach: vorschau.an, betrag: vorschau.betrag })
}

/// **Credits kaufen**: MYL verbrennen; die Kette schreibt Inferenz-Credits gut.
pub fn credits_kaufen(leitung: &Knotenleitung, w: &Wallet, betrag: u64) -> Result<String, String> {
    senden(leitung, w, Anweisung::Burn { betrag })
}

/// Eine eröffnete Sitzung, wie der Client sie für den Agenten ablegt.
#[derive(Debug, Clone)]
pub struct Sitzung {
    pub id: SitzungId,
    /// Die Vollmacht als Bearer-Token, für `Authorization: Bearer`.
    pub vollmacht: String,
    pub gueltig_bis: u64,
    pub budget: u64,
    pub betreiber: Address,
}

/// Wo die Schlüssel und Vollmachten der Agenten liegen.
pub fn agentenordner() -> PathBuf {
    wallet::ordner().join("agenten")
}

/// Der Schlüssel eines Agenten; beim ersten Mal angelegt (`0600`).
fn agentenschluessel(agent: &str) -> Result<myl_types::bls::BlsSecretKey, String> {
    wallet::name_pruefen(agent)?;
    let pfad = agentenordner().join(format!("{agent}.key"));
    let ikm = match std::fs::read_to_string(&pfad) {
        Ok(t) => hex_lesen(t.trim()).ok_or_else(|| format!("{}: kein Schlüssel", pfad.display()))?,
        Err(_) => {
            let mut ikm = [0u8; 32];
            getrandom::fill(&mut ikm).map_err(|e| e.to_string())?;
            std::fs::create_dir_all(agentenordner()).map_err(|e| e.to_string())?;
            myl_types::geheimdatei::schreiben(&pfad, format!("{}\n", hex(&ikm)).as_bytes()).map_err(|e| e.to_string())?;
            ikm.to_vec()
        }
    };
    myl_types::bls::BlsSecretKey::key_gen(&ikm).map_err(|_| "Agentenschlüssel nicht ableitbar".to_string())
}

/// **Eröffnet einen Sitzungskontrakt für einen Agenten** und stellt ihm die
/// Vollmacht aus: Budget und Einzellimit in Credits, gültig ab jetzt für
/// `epochen` Epochen, einziger Empfänger der Betreiber des Knotens.
pub fn sitzung_eroeffnen(
    leitung: &Knotenleitung,
    w: &Wallet,
    agent: &str,
    budget: u64,
    einzellimit: u64,
    epochen: u64,
) -> Result<Sitzung, String> {
    let agent_sk = agentenschluessel(agent)?;
    let agent_pk = agent_sk.public_key().map_err(|_| "Agentenschlüssel".to_string())?;
    let stand = leitung.kontostand(&w.adresse)?;
    let empfaenger = vec![stand.betreiber];
    let kontrakt = Sitzungskontrakt {
        inhaber: w.adresse,
        agent: Address::aus_schluessel(&agent_pk),
        credits: Grenzen { budget, einzellimit, schwelle: u64::MAX, zeugenleiter: Vec::new() },
        myl: Grenzen::gesperrt(),
        empfaenger_wurzel: myl_types::sitzung::empfaengerwurzel(&empfaenger),
        gueltig_ab: myl_types::ids::EpochId(stand.epoche),
        gueltig_bis: myl_types::ids::EpochId(stand.epoche + epochen),
        max_schritte: 10_000,
    };
    let id = kontrakt.adresse();
    senden(leitung, w, Anweisung::SitzungEroeffnen { kontrakt: kontrakt.clone() })?;
    let mut saat = [0u8; 32];
    getrandom::fill(&mut saat).map_err(|e| e.to_string())?;
    let beweis = myl_types::sitzung::empfaengerbeweis(&empfaenger, &stand.betreiber).ok_or("Empfängerbeweis")?;
    let vollmacht = myl_types::vollmacht::Vollmacht::ausstellen(
        &agent_sk,
        vec![
            myl_types::vollmacht::Vorbehalt::NurSitzung(id),
            myl_types::vollmacht::Vorbehalt::GueltigBis(kontrakt.gueltig_bis),
        ],
        saat,
    )
    .map_err(|_| "Vollmacht ausstellen".to_string())?
    .mit_empfaengerbeweis(stand.betreiber, beweis)
    .als_bearer();
    // Die Vollmacht für den Agenten ablegen (`0600`): der Netzweg liest sie.
    let datei = agentenordner().join(format!("{agent}.vollmacht"));
    myl_types::geheimdatei::schreiben(&datei, format!("{vollmacht}\n").as_bytes()).map_err(|e| e.to_string())?;
    Ok(Sitzung { id, vollmacht, gueltig_bis: kontrakt.gueltig_bis.0, budget, betreiber: stand.betreiber })
}

/// **Widerruft eine Sitzung**; nur der Inhaber kann es.
pub fn sitzung_widerrufen(leitung: &Knotenleitung, w: &Wallet, id: SitzungId) -> Result<String, String> {
    senden(leitung, w, Anweisung::SitzungWiderrufen { sitzung: id })
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hex_lesen(t: &str) -> Option<Vec<u8>> {
    if !t.len().is_multiple_of(2) {
        return None;
    }
    (0..t.len() / 2).map(|i| u8::from_str_radix(t.get(i * 2..i * 2 + 2)?, 16).ok()).collect()
}
