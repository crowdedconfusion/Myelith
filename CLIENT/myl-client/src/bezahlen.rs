//! **Bezahlen aus dem Wallet** (2026-10-26): Credits kaufen, einen
//! Sitzungskontrakt für einen Agenten eröffnen, ihn widerrufen.
//!
//! # ⚑ Die Rollen
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
use myl_types::relais::{Kontostand, Relaisantwort, Relaisfrage, Sitzungsstand};
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

    /// Wie eine Sitzung in der Kette steht, samt Credit-Preis (Fund 707).
    pub fn sitzungsstand(&self, id: SitzungId) -> Result<Sitzungsstand, String> {
        match self.fragen(&Relaisfrage::Sitzung { id })? {
            Relaisantwort::Sitzung(s) => Ok(s),
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
/// Achtel; das Doppelte trägt über rund sechs volle Blöcke in Folge.
/// Abgebucht wird nur die Gebühr des
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

/// **Der Auftrag zu einer Vorschau**, für ein kaltes Wallet (CLIENT 7.12):
/// dieselbe Nonce, derselbe Netzwert, dieselbe Gebührengrenze, die ein
/// warmes Wallet unterschriebe.
pub fn ueberweisung_als_auftrag(vorschau: &Ueberweisungsvorschau, absender: myl_types::bls::BlsPublicKey) -> Result<crate::kalt::Auftrag, String> {
    if Address::aus_schluessel(&absender) != vorschau.von {
        return Err("Der Schlüssel gehört nicht zum Absender der Vorschau.".into());
    }
    Ok(crate::kalt::Auftrag {
        netzwert: vorschau.stand.netzwert,
        absender,
        nonce: vorschau.stand.nonce,
        max_gebuehr: vorschau.max_gebuehr,
        anweisung: Anweisung::Ueberweisung { nach: vorschau.an, betrag: vorschau.betrag },
    })
}

/// **Unterschreibt einen Auftrag** mit dem Schlüssel eines (warmen) Wallets
/// dieses Geräts (CLIENT 7.12). Der Schlüssel verlässt die Kiste nur als
/// Unterschrift, wie bei [`senden`].
pub fn kalt_unterschreiben(name: &str, auftrag: &crate::kalt::Auftrag) -> Result<Transaktion, String> {
    let geheim = wallet::geheimschluessel(name)?;
    auftrag.unterschreiben(&geheim)
}

/// **Reicht eine anderswo unterschriebene Transaktion ein** (CLIENT 7.12).
pub fn einreichen(leitung: &Knotenleitung, tx: &Transaktion) -> Result<String, String> {
    let bytes = borsh::to_vec(tx).map_err(|e| e.to_string())?;
    match leitung.fragen(&Relaisfrage::Transaktion { tx: bytes })? {
        Relaisantwort::Erledigt(z) => Ok(z),
        Relaisantwort::Fehler(f) => Err(f),
        andere => Err(format!("unerwartete Antwort: {andere:?}")),
    }
}

/// **Credits kaufen**: MYL verbrennen; die Kette schreibt Inferenz-Credits gut.
pub fn credits_kaufen(leitung: &Knotenleitung, w: &Wallet, betrag: u64) -> Result<String, String> {
    senden(leitung, w, Anweisung::Burn { betrag })
}

/// ⚑ **Das Budget einer Sitzung, wenn keines genannt ist**, in Credits; das
/// Einzellimit ist ein Zehntel davon. Seit Fund 711 kostet eine Anfrage
/// Prompt und Ausgabe, im Auftragslauf rund das Fünfundzwanzigfache der
/// Ausgabe allein: also das alte Budget von 10 000 mal 25. Eine Runde eines
/// Agenten mit 16 000 Prompt-Token und 4 096 neuen liegt unter dem
/// Einzellimit.
pub const SITZUNGSBUDGET: u64 = 250_000;

/// ⚑ **So viele Runden deckt ein Nachkauf** (Fund 707), wenn nichts
/// anderes verlangt ist. Credits verfallen kurz nach dem Kauf: Ein grosser
/// Vorrat verfällt ungenutzt, ein Kauf je Runde wartet jedes Mal auf einen
/// Block. Vier Runden sind bei einem Agenten rund ein Auftrag.
pub const VORRAT_RUNDEN: u64 = 4;

/// Die Lage vor einer Runde über die Tür, für [`nachkauf_entscheiden`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nachkauflage {
    /// Credits des Inhabers, die für diese Runde zählen: die geltenden,
    /// oder nur die haltbaren, wenn der Epochenwechsel in die Runde fällt
    /// ([`zaehlbare_credits`]).
    pub credits: u64,
    /// Über die Tür ausgegeben, aber noch nicht von der Kette abgebucht.
    pub offen: u64,
    /// Was diese Runde voraussichtlich höchstens kostet: Prompt und Deckel
    /// (Fund 711).
    pub bedarf: u64,
    /// Was eine Runde bisher höchstens kostete; null vor der ersten. Der
    /// Vorrat über diese Runde hinaus bemisst sich daran.
    pub je_runde: u64,
    /// So viele Runden soll ein Kauf decken.
    pub vorrat_runden: u64,
    /// Budget des Kontrakts abzüglich dessen, was die Kette abgebucht hat.
    pub budget_rest: u64,
    /// Kleinstbeträge je Credit.
    pub preis: u64,
    /// Guthaben des Wallets in Kleinstbeträgen.
    pub guthaben: u64,
    /// Was die Kauftransaktion höchstens an Gebühr kostet.
    pub gebuehr: u64,
}

/// Was vor einer Runde zu tun ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nachkauf {
    /// Das Guthaben deckt die Runde.
    Genug,
    /// So viele Credits kaufen, für so viele Kleinstbeträge.
    Kaufen { credits: u64, myl: u64 },
    /// ⚑ Der Rest des Budgets deckt die Runde nicht: **nicht kaufen**, die
    /// Tür lehnte ohnehin ab, und der Kauf verfiele ungenutzt.
    BudgetErschoepft { rest: u64 },
    /// Das Wallet kann die Runde nicht decken.
    GeldFehlt { noetig: u64, guthaben: u64 },
}

/// 📌 **Lehnt die Tür wegen des Guthabens ab, zählt ihr Wort** (Fund 716,
/// Auftragslauf vom 2026-11-20). Die Tür zieht vom Guthaben des Inhabers
/// auch ab, was sie anderen Anfragen zugesagt hat, die noch rechnen; dieser
/// Klient sieht das nicht. Im Lauf hielt eine verwaiste Anfrage (ihr Agent
/// war an der Frist beendet, ihr Pod rechnete sie nach dem Wechsel neu)
/// rund 24 000 von 26 980 Credits; jede neue Sitzung bekam 2 697 zugesagt,
/// der Klient hielt sich für gedeckt, kaufte nicht, und sechs Aufträge
/// endeten in Sekunden mit 402. Nach einer Ablehnung (und den zwei Slots,
/// die ein eben gekaufter Bestand braucht, um bei der Tür anzukommen) gilt
/// deshalb nichts als verfügbar: Gekauft wird die Runde samt Vorrat, nie
/// über das Budget.
pub fn nach_ablehnung(l: Nachkauflage) -> Nachkauflage {
    Nachkauflage { credits: l.offen.min(l.credits), ..l }
}

/// ⚑ **Die Entscheidung des Nachkaufs, ohne Netz und ohne Uhr** (Fund 707,
/// Festlegung des Projektinhabers vom 2026-11-16: Credits bleiben kurz
/// gültig, der Klient kauft innerhalb des Sitzungsbudgets nach).
///
/// - Deckt das Guthaben abzüglich des Offenen die Runde, geschieht nichts.
/// - Sonst wird auf den Vorrat aufgefüllt: diese Runde und `vorrat_runden - 1`
///   weitere wie die teuerste bisher; vor der ersten Runde nur diese. **Nie
///   über den Rest des Budgets**: Das Budget des Kontrakts bleibt der
///   Sprengradius (Kap. 8.2), auch für das, was der Nachkauf verbrennt.
///   📌 Seit Fund 711 ist der Bedarf einer Runde eine Schätzung über dem
///   Preis, vor der ersten Runde die Schranke aus Bytes und Deckel (rund das
///   Vierfache). Im kurzen Auftragslauf vom 2026-11-17 kaufte der Vorrat aus
///   vier Schranken 60 177 Credits für einen Auftrag von 17 758; 53 782
///   blieben übrig und verfallen. Der Vorrat wartet deshalb auf eine
///   gemessene Runde.
/// - Reicht das Geld nicht für den Vorrat, aber für die Runde, wird gekauft,
///   was geht.
pub fn nachkauf_entscheiden(l: &Nachkauflage) -> Nachkauf {
    let verfuegbar = l.credits.saturating_sub(l.offen);
    if verfuegbar >= l.bedarf {
        return Nachkauf::Genug;
    }
    let rest = l.budget_rest.saturating_sub(l.offen);
    if rest < l.bedarf {
        return Nachkauf::BudgetErschoepft { rest };
    }
    let preis = l.preis.max(1);
    let ziel = l.bedarf.saturating_add(l.je_runde.saturating_mul(l.vorrat_runden.max(1) - 1)).min(rest);
    let mut credits = ziel - verfuegbar;
    let bezahlbar = l.guthaben.saturating_sub(l.gebuehr) / preis;
    if bezahlbar < credits {
        let mindestens = l.bedarf - verfuegbar;
        if bezahlbar < mindestens {
            return Nachkauf::GeldFehlt {
                noetig: mindestens.saturating_mul(preis).saturating_add(l.gebuehr),
                guthaben: l.guthaben,
            };
        }
        credits = bezahlbar;
    }
    Nachkauf::Kaufen { credits, myl: credits.saturating_mul(preis) }
}

/// ⚑ **Welche Credits für eine Runde zählen** (Fund 707): Eine Runde wird
/// abgebucht, wenn sie fertig ist. Fällt der Epochenwechsel hinein, zählt
/// nur, was ihn übersteht; sonst rechnete die Runde gegen Credits, die bei
/// der Abbuchung verfallen sind, und der Miner ginge leer aus. „Hinein“
/// heisst: Der Wechsel kommt früher als das Doppelte der längsten Runde.
pub fn zaehlbare_credits(s: &Sitzungsstand, laengste_runde: Duration) -> u64 {
    let bis_wechsel_ms = u128::from(s.bloecke_bis_wechsel) * u128::from(s.slot_ms);
    if bis_wechsel_ms <= laengste_runde.as_millis() * 2 {
        s.credits_haltbar
    } else {
        s.credits
    }
}

/// So lange gilt eine Runde als lang, bevor eine gemessen ist. 📌 Über die
/// Pods des 35B brauchten Runden im Auftragslauf 82 bis 356 s.
pub const RUNDE_GESCHAETZT: Duration = Duration::from_secs(300);

/// Was ein Nachkauf vor einer Runde tat, für eine Zeile im Protokoll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nachkaufbericht {
    /// Nichts zu tun.
    Genug,
    /// Gekauft, und die Credits stehen in der Kette.
    Gekauft { credits: u64, myl: u64, preis: u64, epoche: u64 },
}

/// **Der Nachkauf einer Sitzung** (Fund 707): Wallet, Leitung und Sitzung,
/// dazu der Stand der Abbuchungen beim ersten Blick.
///
/// ⚑ **Der Schlüssel des Wallets bleibt beim Klienten.** Der Agent bekommt
/// ihn auch hier nicht: Er unterschreibt nur über diese Stelle, nur einen
/// Burn, und nie mehr, als der Rest des Sitzungsbudgets deckt.
pub struct Nachkaeufer {
    leitung: Knotenleitung,
    wallet: Wallet,
    sitzung: SitzungId,
    vorrat_runden: u64,
    /// `verbraucht` der Kette beim ersten Blick und was dieser Klient
    /// seither selbst über die Tür ausgab, als die Kette das zuletzt
    /// einholte: daraus folgt das Offene.
    anfang: std::sync::Mutex<Option<u64>>,
}

impl Nachkaeufer {
    /// Für die Sitzung, die in der Vollmacht des Agenten steht.
    pub fn neu(leitung: Knotenleitung, wallet: Wallet, vollmacht: &str, vorrat_runden: u64) -> Result<Self, String> {
        let sitzung = myl_types::vollmacht::Vollmacht::aus_bearer(vollmacht)
            .and_then(|v| v.sitzung())
            .ok_or("Die Vollmacht nennt keine Sitzung; ein Nachkauf braucht eine.")?;
        let stand = leitung.sitzungsstand(sitzung)?;
        if stand.inhaber != wallet.adresse {
            return Err(format!("Das Wallet „{}“ ist nicht der Inhaber dieser Sitzung.", wallet.name));
        }
        Ok(Self { leitung, wallet, sitzung, vorrat_runden: vorrat_runden.max(1), anfang: std::sync::Mutex::new(None) })
    }

    /// Die Lage aus dem Stand der Kette: `heraus` Credits gingen über die
    /// Tür, seit dieser Klient läuft.
    fn lage(&self, s: &Sitzungsstand, konto: &Kontostand, bedarf: u64, je_runde: u64, heraus: u64, laengste_runde: Duration) -> Nachkauflage {
        let anfang = {
            let mut a = self.anfang.lock().unwrap_or_else(|e| e.into_inner());
            *a.get_or_insert(s.verbraucht.saturating_sub(heraus))
        };
        let abgebucht = s.verbraucht.saturating_sub(anfang);
        Nachkauflage {
            credits: zaehlbare_credits(s, laengste_runde),
            offen: heraus.saturating_sub(abgebucht),
            bedarf,
            je_runde,
            vorrat_runden: self.vorrat_runden,
            budget_rest: s.budget.saturating_sub(s.verbraucht),
            preis: s.credit_preis,
            guthaben: konto.guthaben,
            // Der Betrag ändert die Länge der Transaktion nicht.
            gebuehr: gebuehr(konto, &Anweisung::Burn { betrag: 0 }).saturating_mul(GEBUEHR_SPIELRAUM),
        }
    }

    /// ⚑ **Vor einer Runde**: prüfen, bei Bedarf kaufen und warten, bis die
    /// Credits in der Kette stehen. `abgelehnt` sagt, dass die Tür die
    /// Runde eben wegen des Guthabens ablehnte, obwohl dieser Klient es für
    /// gedeckt hielt: Dann sieht ihr Knoten den Kauf noch nicht, und es
    /// wird zwei Slots gewartet, bevor neu geprüft wird.
    pub fn vor_der_runde(
        &self,
        bedarf: u64,
        je_runde: u64,
        heraus: u64,
        laengste_runde: Duration,
        abgelehnt: bool,
    ) -> Result<Nachkaufbericht, String> {
        let mut s = self.leitung.sitzungsstand(self.sitzung)?;
        let slot = Duration::from_millis(s.slot_ms.clamp(100, 10_000));
        if abgelehnt {
            std::thread::sleep(slot * 2);
            s = self.leitung.sitzungsstand(self.sitzung)?;
        }
        if s.widerrufen {
            return Err("Die Sitzung ist widerrufen.".into());
        }
        if s.epoche > s.gueltig_bis {
            return Err(format!("Die Sitzung galt bis Epoche {}, jetzt läuft {}.", s.gueltig_bis, s.epoche));
        }
        let konto = self.leitung.kontostand(&self.wallet.adresse)?;
        let mut lage = self.lage(&s, &konto, bedarf, je_runde, heraus, laengste_runde);
        if abgelehnt {
            lage = nach_ablehnung(lage);
        }
        let (credits, myl) = match nachkauf_entscheiden(&lage) {
            Nachkauf::Genug => return Ok(Nachkaufbericht::Genug),
            Nachkauf::Kaufen { credits, myl } => (credits, myl),
            Nachkauf::BudgetErschoepft { rest } => {
                return Err(format!("Das Budget der Sitzung ist erschöpft: Rest {rest} Credits, die Runde braucht bis zu {bedarf}."));
            }
            Nachkauf::GeldFehlt { noetig, guthaben } => {
                return Err(format!("Das Wallet deckt den Nachkauf nicht: nötig {noetig} Kleinstbeträge, vorhanden {guthaben}."));
            }
        };
        senden_mit(&self.leitung, &self.wallet, &konto, Anweisung::Burn { betrag: myl })?;
        // ⚑ Warten, bis der Kauf in der Kette steht: an der Nonce des
        //   Wallets abgelesen, nicht an einer Zeit. Die Frist ist nur der
        //   Ausweg, wenn die Transaktion verloren ging.
        let frist = std::time::Instant::now() + (slot * 60).max(Duration::from_secs(60));
        loop {
            std::thread::sleep(slot);
            let jetzt = self.leitung.kontostand(&self.wallet.adresse)?;
            if jetzt.nonce > konto.nonce {
                let s = self.leitung.sitzungsstand(self.sitzung)?;
                let danach = self.lage(&s, &jetzt, bedarf, je_runde, heraus, laengste_runde);
                if danach.credits.saturating_sub(danach.offen) < bedarf {
                    return Err(format!(
                        "Der Kauf steht in der Kette, deckt die Runde aber nicht ({} Credits, nötig {bedarf}); der Preis ist gestiegen oder die Epoche hat gewechselt.",
                        danach.credits.saturating_sub(danach.offen)
                    ));
                }
                return Ok(Nachkaufbericht::Gekauft { credits, myl, preis: s.credit_preis, epoche: s.epoche });
            }
            if std::time::Instant::now() >= frist {
                return Err("Der Kauf steht nach der Frist nicht in der Kette.".into());
            }
        }
    }
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

#[cfg(test)]
mod nachkauf_proben {
    use super::*;

    fn lage() -> Nachkauflage {
        Nachkauflage { credits: 100, offen: 0, bedarf: 512, je_runde: 512, vorrat_runden: 4, budget_rest: 10_000, preis: 67, guthaben: 1_000_000, gebuehr: 300 }
    }

    /// Fund 716: Hält dieser Klient die Runde für gedeckt, die Tür aber nicht,
    /// wird nach der Ablehnung gekauft. Gegenprobe: ohne [`nach_ablehnung`]
    /// bliebe es bei `Genug`, und die Runde scheiterte erneut.
    #[test]
    fn nach_einer_ablehnung_wird_gekauft_auch_wenn_es_reicht() {
        let l = Nachkauflage { credits: 26_980, bedarf: 3_700, je_runde: 0, budget_rest: 400_000, ..lage() };
        assert_eq!(nachkauf_entscheiden(&l), Nachkauf::Genug);
        assert_eq!(nachkauf_entscheiden(&nach_ablehnung(l)), Nachkauf::Kaufen { credits: 3_700, myl: 3_700 * 67 });
        // Das Offene bleibt offen, das Budget die Grenze.
        let knapp = Nachkauflage { credits: 900, offen: 300, budget_rest: 2_000, ..lage() };
        assert_eq!(nach_ablehnung(knapp).credits, 300);
        assert_eq!(nachkauf_entscheiden(&nach_ablehnung(knapp)), Nachkauf::Kaufen { credits: 1_700, myl: 1_700 * 67 });
    }

    /// ⚑ **Aufgefüllt wird auf den Vorrat, nie über den Rest des Budgets.**
    /// Gegenprobe zu jeder Grenze: Ohne das Abziehen des Offenen gälte die
    /// zweite Lage als gedeckt; ohne die Budgetgrenze kaufte die dritte 1948
    /// statt 500.
    #[test]
    fn der_nachkauf_fuellt_auf_den_vorrat_und_nie_ueber_das_budget() {
        assert_eq!(nachkauf_entscheiden(&Nachkauflage { credits: 512, ..lage() }), Nachkauf::Genug);
        assert_eq!(
            nachkauf_entscheiden(&Nachkauflage { credits: 600, offen: 100, ..lage() }),
            Nachkauf::Kaufen { credits: 2048 - 500, myl: (2048 - 500) * 67 }
        );
        assert_eq!(nachkauf_entscheiden(&lage()), Nachkauf::Kaufen { credits: 1948, myl: 1948 * 67 });
        assert_eq!(
            nachkauf_entscheiden(&Nachkauflage { budget_rest: 600, ..lage() }),
            Nachkauf::Kaufen { credits: 500, myl: 500 * 67 }
        );
        // Das Offene zählt auch gegen das Budget.
        assert_eq!(
            nachkauf_entscheiden(&Nachkauflage { budget_rest: 600, offen: 100, ..lage() }),
            Nachkauf::BudgetErschoepft { rest: 500 }
        );
        assert_eq!(nachkauf_entscheiden(&Nachkauflage { budget_rest: 511, ..lage() }), Nachkauf::BudgetErschoepft { rest: 511 });
    }

    /// ⚑ **Der Vorrat bemisst sich an der teuersten Runde, nicht an der
    /// Schätzung** (Fund 711): diese Runde mit ihrem Bedarf, drei weitere wie
    /// die teuerste bisher. Vor der ersten Runde nur diese; Gegenprobe zur
    /// Fassung davor, die viermal den Bedarf kaufte (1948).
    #[test]
    fn der_vorrat_folgt_der_teuersten_runde() {
        assert_eq!(
            nachkauf_entscheiden(&Nachkauflage { je_runde: 200, ..lage() }),
            Nachkauf::Kaufen { credits: 512 + 3 * 200 - 100, myl: (512 + 3 * 200 - 100) * 67 }
        );
        assert_eq!(nachkauf_entscheiden(&Nachkauflage { je_runde: 0, ..lage() }), Nachkauf::Kaufen { credits: 412, myl: 412 * 67 });
        assert_eq!(nachkauf_entscheiden(&Nachkauflage { je_runde: 200, vorrat_runden: 1, ..lage() }), Nachkauf::Kaufen { credits: 412, myl: 412 * 67 });
    }

    /// ⚑ Reicht das Geld nicht für den Vorrat, aber für die Runde, wird
    /// gekauft, was geht; reicht es nicht für die Runde, wird nichts
    /// verbrannt. Gegenprobe: Wer die Gebühr nicht abzieht, kauft einen
    /// Credit zu viel und die Transaktion scheitert.
    #[test]
    fn ein_knappes_wallet_kauft_was_geht_oder_nichts() {
        let knapp = Nachkauflage { guthaben: 500 * 67 + 300, ..lage() };
        assert_eq!(nachkauf_entscheiden(&knapp), Nachkauf::Kaufen { credits: 500, myl: 500 * 67 });
        let zu_knapp = Nachkauflage { guthaben: 411 * 67 + 300, ..lage() };
        assert_eq!(nachkauf_entscheiden(&zu_knapp), Nachkauf::GeldFehlt { noetig: 412 * 67 + 300, guthaben: 411 * 67 + 300 });
        // Ein Preis von null teilt nicht durch null.
        assert!(matches!(nachkauf_entscheiden(&Nachkauflage { preis: 0, ..lage() }), Nachkauf::Kaufen { credits: 1948, .. }));
    }

    fn stand(bloecke_bis_wechsel: u64) -> Sitzungsstand {
        Sitzungsstand {
            hoehe: 0,
            epoche: 3,
            bloecke_bis_wechsel,
            slot_ms: 2_000,
            credit_preis: 67,
            inhaber: Address::new([1; 32]),
            credits: 900,
            credits_haltbar: 500,
            guthaben: 0,
            budget: 4_000,
            einzellimit: 1_000,
            verbraucht: 0,
            hoechste_abrechnung: 0,
            gueltig_bis: 9,
            widerrufen: false,
        }
    }

    /// ⚑ **Nah am Wechsel zählt nur, was ihn übersteht.** Eine Runde von
    /// 300 s: 300 Blöcke zu 2 s sind genau das Doppelte, also nah; 301 nicht.
    /// Gegenprobe: Wer immer die geltenden zählt, bekommt beide Male 900.
    #[test]
    fn nah_am_wechsel_zaehlen_nur_haltbare_credits() {
        assert_eq!(zaehlbare_credits(&stand(300), Duration::from_secs(300)), 500);
        assert_eq!(zaehlbare_credits(&stand(301), Duration::from_secs(300)), 900);
        assert_eq!(zaehlbare_credits(&stand(1), Duration::ZERO), 900, "ohne Rundenzeit ist nichts nah");
    }
}
