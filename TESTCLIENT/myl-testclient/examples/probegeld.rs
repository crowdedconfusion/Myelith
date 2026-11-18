//! **Probegeld für einen Probelauf** (2026-11-15): überweist aus den
//! Probekonten an ein Konto, über die Relaisleitung eines Knotens.
//!
//! ```text
//! probegeld <relais> <ausweis> <an, 64 Hexzeichen> <betrag in Kleinstbeträgen>
//! probegeld <relais> <ausweis> <konto, 64 Hexzeichen> stand
//! ```
//!
//! `stand` zeigt den Kontostand, wie die Relaisleitung ihn nennt, samt
//! Credits (die Beobachtung des Knotens nennt nur MYL).
//!
//! Eine Zeile auf stdout, beginnend mit `[probegeld]`, sobald der Betrag auf
//! dem Konto steht; Rückgabe 0, sonst 1 mit dem Grund.
//!
//! # ⚑ Wofür
//!
//! Im Testnetz mit dem 35B (`--netz test-int4`) gibt es keine Probeprägung:
//! Ein Wallet, das keinen Miner führt, kommt dort an kein Test-MYL, und ohne
//! Test-MYL kauft es keine Credits. Der Auftragslauf braucht aber einen
//! Nutzer, der bezahlt. Dieses Werkzeug spielt den Hahn eines Testnetzes.
//!
//! ⚑ **Nur für Probeläufe und deshalb kein ausgeliefertes Programm**, wie der
//! Lastgeber daneben: Das Geld kommt aus den Probekonten, deren Schlüssel
//! jeder kennt.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use myl_consensus::block::{Anweisung, Transaktion};
use myl_types::ids::Address;
use myl_types::relais::{Kontostand, Relaisantwort, Relaisfrage};

/// Wie lange eine einzelne Frage an die Leitung warten darf.
const FRIST: Duration = Duration::from_secs(10);

/// So viel bleibt auf jedem Probekonto liegen: die Zapfungen des Probehahns
/// der Knoten (Anmeldung, Einsatz, Polster), wie beim Lastgeber.
const RESERVE_JE_KONTO: u64 = 4 * myl_tokenomics::UNITS_PER_MYL;

/// Die Zusage für die Gebühr einer Überweisung: grosszügig, abgebucht wird
/// nur die Gebühr des Blocks.
fn gebuehrenzusage(grundpreis: u64) -> u64 {
    grundpreis.max(1) * (400 + myl_consensus::block::PRUEFEINHEIT) * 2
}

struct Leitung {
    adresse: std::net::SocketAddr,
    ausweis: [u8; myl_types::zeitprobe::AUSWEIS_LEN],
}

impl Leitung {
    fn stand(&self, a: &Address) -> Result<Kontostand, String> {
        match myl_types::relais::fragen(self.adresse, &self.ausweis, &Relaisfrage::Konto { adresse: *a }, FRIST)? {
            Relaisantwort::Konto(k) => Ok(k),
            andere => Err(format!("unerwartete Antwort: {andere:?}")),
        }
    }

    fn senden(&self, tx: &Transaktion) -> Result<(), String> {
        let bytes = borsh::to_vec(tx).map_err(|e| e.to_string())?;
        match myl_types::relais::fragen(self.adresse, &self.ausweis, &Relaisfrage::Transaktion { tx: bytes }, FRIST)? {
            Relaisantwort::Erledigt(_) => Ok(()),
            andere => Err(format!("{andere:?}")),
        }
    }
}

fn adresse_aus_hex(text: &str) -> Result<Address, String> {
    if text.len() != 64 {
        return Err(format!("keine Adresse aus 64 Hexzeichen: {text}"));
    }
    let mut b = [0u8; 32];
    for (i, stelle) in b.iter_mut().enumerate() {
        *stelle = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| format!("kein Hex: {text}"))?;
    }
    Ok(Address::new(b))
}

fn lauf(a: &[String]) -> Result<(), String> {
    let [relais, ausweis, an, betrag] = a else {
        return Err("probegeld <relais> <ausweis> <an> <betrag in Kleinstbeträgen>".into());
    };
    let l = Leitung {
        adresse: relais.parse().map_err(|_| format!("keine Adresse: {relais}"))?,
        ausweis: myl_types::relais::ausweis_lesen(std::path::Path::new(ausweis)).map_err(|e| e.to_string())?,
    };
    let an = adresse_aus_hex(an)?;
    if betrag == "stand" {
        let k = l.stand(&an)?;
        let betreiber: String = k.betreiber.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        println!(
            "[probegeld] stand hoehe={} epoche={} guthaben={} credits={} nonce={} betreiber={betreiber}",
            k.hoehe, k.epoche, k.guthaben, k.credits, k.nonce
        );
        return Ok(());
    }
    let betrag: u64 = betrag.parse().map_err(|_| "Betrag als Zahl".to_string())?;
    let vorher = l.stand(&an)?.guthaben;

    // Die reichsten Probekonten zuerst; jedes gibt nur, was über seiner
    // Reserve liegt.
    let mut quellen = Vec::new();
    for q in 0..myl_node::kette::PROBEKONTEN {
        let sk = myl_node::kette::probeschluessel(q);
        let a = myl_node::kette::probekonto(q);
        quellen.push((sk, a, l.stand(&a)?));
    }
    quellen.sort_by_key(|(_, _, s)| std::cmp::Reverse(s.guthaben));
    let mut rest = betrag;
    for (sk, _, stand) in &quellen {
        if rest == 0 {
            break;
        }
        let zusage = gebuehrenzusage(stand.grundpreis_je_byte);
        let frei = stand.guthaben.saturating_sub(RESERVE_JE_KONTO + zusage);
        if frei == 0 {
            continue;
        }
        let teil = frei.min(rest);
        let tx = Transaktion::signiere_mit_grenze(&stand.netzwert, sk, stand.nonce, zusage, Anweisung::Ueberweisung { nach: an, betrag: teil })
            .map_err(|e| format!("unterschreiben: {e:?}"))?;
        l.senden(&tx)?;
        rest -= teil;
    }
    if rest > 0 {
        return Err(format!("die Probekonten geben über ihrer Reserve nur {} von {betrag} her", betrag - rest));
    }
    let beginn = Instant::now();
    loop {
        let jetzt = l.stand(&an)?.guthaben;
        if jetzt >= vorher + betrag {
            println!("[probegeld] {betrag} an {} überwiesen, Guthaben {vorher} -> {jetzt}", &a[2][..16]);
            return Ok(());
        }
        if beginn.elapsed() > Duration::from_secs(180) {
            return Err(format!("nach 180 s steht das Geld nicht auf dem Konto ({vorher} -> {jetzt})"));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match lauf(&a) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[probegeld] Fehler: {e}");
            ExitCode::FAILURE
        }
    }
}
