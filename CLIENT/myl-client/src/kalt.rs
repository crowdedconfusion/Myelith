//! **Das kalte Auszahlungskonto** (CLIENT 7.12, 2026-11-19).
//!
//! # Wozu
//!
//! Ein Betreiber will Erträge nicht auf einem Rechner ansammeln, auf dem
//! auch der Schlüssel liegt, der sie ausgeben kann: Der Rechner, der mint,
//! ist am Netz und läuft Tag und Nacht. Darum gibt es ein Wallet **nur als
//! öffentlichen Schlüssel** ([`crate::wallet::beobachten`]). Es kann als
//! Auszahlungskonto gewählt werden (der Knoten braucht nur die Adresse),
//! seinen Stand zeigen und Überweisungen **vorbereiten**; unterschrieben wird
//! auf dem Gerät mit dem Schlüssel, das dafür nicht am Netz sein muss.
//!
//! # Der Weg, nach dem Muster von PSBT (BIP-174)
//!
//! 1. Am Rechner mit dem kalten Wallet: `myl wallet ueberweisen` liest Nonce,
//!    Netzwert und Gebühr beim Knoten und schreibt einen **Auftrag**
//!    ([`Auftrag::als_text`]).
//! 2. Am Gerät mit dem Schlüssel: `myl wallet unterschreiben` liest den
//!    Auftrag, **zeigt, was die Bytes sagen** (nicht, was ein Kommentar
//!    behauptet), fragt und schreibt die **Unterschrift**
//!    ([`unterschrift_als_text`]).
//! 3. Wieder am Netz: `myl wallet einreichen` reicht sie beim Knoten ein.
//!
//! Übertragen wird eine Textdatei (USB-Stick, oder abgetippt); beide Formate
//! tragen eine Kennung mit Fassung vorn.
//!
//! # ⚑ Was geprüft wird
//!
//! - Beim Unterschreiben: Der Schlüssel des Geräts gehört zum Absender des
//!   Auftrags, sonst wird nichts unterschrieben.
//! - Beim Einreichen: Die Unterschrift trägt gegen den Netzwert des Auftrags
//!   ([`Transaktion::pruefe`]); eine verstümmelte Datei fällt so vor dem
//!   Knoten auf.
//! - Die Beschreibung, die ein Mensch bestätigt, wird aus den **kodierten
//!   Bytes** gebildet, die unterschrieben werden. Die Kommentarzeile im
//!   Auftrag ist nur für den Menschen am anderen Ende und wird nie gelesen.

use myl_consensus::block::{Anweisung, Transaktion};
use myl_types::bls::{BlsPublicKey, BlsSecretKey, BLS_PK_LEN};
use myl_types::hash::Hash;
use myl_types::ids::Address;

/// Kennung und Fassung eines Auftrags.
pub const AUFTRAG_KOPF: &str = "myelith-auftrag-1";
/// Kennung und Fassung einer Unterschrift.
pub const UNTERSCHRIFT_KOPF: &str = "myelith-unterschrift-1";

/// Was unterschrieben werden soll, ohne Unterschrift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Auftrag {
    /// Der Netzwert, gegen den unterschrieben wird (der Startwert der Kette).
    pub netzwert: Hash,
    /// Der öffentliche Schlüssel des Absenders.
    pub absender: BlsPublicKey,
    pub nonce: u64,
    pub max_gebuehr: u64,
    pub anweisung: Anweisung,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn aus_hex(t: &str) -> Option<Vec<u8>> {
    if !t.len().is_multiple_of(2) {
        return None;
    }
    (0..t.len()).step_by(2).map(|i| u8::from_str_radix(t.get(i..i + 2)?, 16).ok()).collect()
}

/// Die Zeilen `schluessel wert` nach dem Kopf; Kommentare und Leerzeilen fallen weg.
fn felder<'a>(text: &'a str, kopf: &str) -> Result<Vec<(&'a str, &'a str)>, String> {
    let mut zeilen = text.lines().map(str::trim).filter(|z| !z.is_empty() && !z.starts_with('#'));
    match zeilen.next() {
        Some(k) if k == kopf => {}
        Some(k) => return Err(format!("keine Datei der Art {kopf} (Kopf „{k}“)")),
        None => return Err("leere Datei".into()),
    }
    zeilen
        .map(|z| z.split_once(' ').map(|(a, b)| (a, b.trim())).ok_or_else(|| format!("Zeile ohne Wert: {z}")))
        .collect()
}

fn feld<'a>(f: &[(&'a str, &'a str)], name: &str) -> Result<&'a str, String> {
    let mut treffer = f.iter().filter(|(k, _)| *k == name);
    let w = treffer.next().map(|(_, w)| *w).ok_or_else(|| format!("Feld {name} fehlt"))?;
    if treffer.next().is_some() {
        return Err(format!("Feld {name} steht zweimal"));
    }
    Ok(w)
}

fn netzwert_aus(t: &str) -> Result<Hash, String> {
    let b: [u8; 32] = aus_hex(t).and_then(|v| v.try_into().ok()).ok_or("netz: 64 Hexzeichen erwartet")?;
    Ok(Hash::from_bytes(b))
}

impl Auftrag {
    /// Die Adresse des Absenders.
    pub fn absender_adresse(&self) -> Address {
        Address::aus_schluessel(&self.absender)
    }

    /// Als Textdatei. Die letzte Zeile ist ein Kommentar für den Menschen.
    pub fn als_text(&self) -> String {
        let anweisung = borsh::to_vec(&self.anweisung).expect("Anweisung ist serialisierbar");
        format!(
            "{AUFTRAG_KOPF}\nnetz {}\nabsender {}\nnonce {}\nmax_gebuehr {}\nanweisung {}\n# {}\n",
            hex(self.netzwert.as_bytes()),
            hex(&self.absender.0),
            self.nonce,
            self.max_gebuehr,
            hex(&anweisung),
            self.beschreibung().replace('\n', " ")
        )
    }

    /// Aus der Textdatei.
    pub fn aus_text(text: &str) -> Result<Self, String> {
        let f = felder(text, AUFTRAG_KOPF)?;
        let absender: [u8; BLS_PK_LEN] =
            aus_hex(feld(&f, "absender")?).and_then(|v| v.try_into().ok()).ok_or("absender: 96 Hexzeichen erwartet")?;
        let absender = BlsPublicKey(absender);
        absender.validate().map_err(|_| "absender: kein gültiger Schlüssel")?;
        let roh = aus_hex(feld(&f, "anweisung")?).ok_or("anweisung: Hex erwartet")?;
        let anweisung: Anweisung = borsh::from_slice(&roh).map_err(|e| format!("anweisung unlesbar: {e}"))?;
        Ok(Self {
            netzwert: netzwert_aus(feld(&f, "netz")?)?,
            absender,
            nonce: feld(&f, "nonce")?.parse().map_err(|_| "nonce: Zahl erwartet")?,
            max_gebuehr: feld(&f, "max_gebuehr")?.parse().map_err(|_| "max_gebuehr: Zahl erwartet")?,
            anweisung,
        })
    }

    /// **Was die Bytes sagen**, für die Bestätigung vor dem Unterschreiben.
    pub fn beschreibung(&self) -> String {
        let t = |a: &Address| myl_types::adresstext::als_text(a, myl_types::adresstext::PRAEFIX_TESTNETZ);
        let was = match &self.anweisung {
            Anweisung::Ueberweisung { nach, betrag } => {
                format!("Überweisung von {} MYL an {}", crate::mining::myl_text(*betrag), t(nach))
            }
            Anweisung::Burn { betrag } => format!("Credits kaufen: {} MYL verbrennen", crate::mining::myl_text(*betrag)),
            andere => format!("Anweisung {andere:?}"),
        };
        format!(
            "{was}\nvon {} (Nonce {}), Gebühr höchstens {} MYL, Netz {}",
            t(&self.absender_adresse()),
            self.nonce,
            crate::mining::myl_text(self.max_gebuehr),
            &hex(self.netzwert.as_bytes())[..16]
        )
    }

    /// **Unterschreibt**, wenn der Schlüssel zum Absender gehört.
    pub fn unterschreiben(&self, geheim: &BlsSecretKey) -> Result<Transaktion, String> {
        let eigener = geheim.public_key().map_err(|_| "Schlüssel ungültig".to_string())?;
        if eigener != self.absender {
            return Err(format!(
                "Dieser Schlüssel gehört zu {}, der Auftrag zu {}; es wird nichts unterschrieben.",
                myl_types::adresstext::als_text(&Address::aus_schluessel(&eigener), myl_types::adresstext::PRAEFIX_TESTNETZ),
                myl_types::adresstext::als_text(&self.absender_adresse(), myl_types::adresstext::PRAEFIX_TESTNETZ)
            ));
        }
        Transaktion::signiere_mit_grenze(&self.netzwert, geheim, self.nonce, self.max_gebuehr, self.anweisung.clone())
            .map_err(|e| format!("unterschreiben: {e:?}"))
    }
}

/// Die Unterschrift als Textdatei: Netzwert und die Transaktion (borsh).
pub fn unterschrift_als_text(netzwert: &Hash, tx: &Transaktion) -> String {
    format!(
        "{UNTERSCHRIFT_KOPF}\nnetz {}\ntx {}\n",
        hex(netzwert.as_bytes()),
        hex(&borsh::to_vec(tx).expect("Transaktion ist serialisierbar"))
    )
}

/// Liest eine Unterschrift und **prüft sie** gegen ihren Netzwert.
pub fn unterschrift_aus_text(text: &str) -> Result<(Hash, Transaktion), String> {
    let f = felder(text, UNTERSCHRIFT_KOPF)?;
    let netz = netzwert_aus(feld(&f, "netz")?)?;
    let roh = aus_hex(feld(&f, "tx")?).ok_or("tx: Hex erwartet")?;
    let tx: Transaktion = borsh::from_slice(&roh).map_err(|e| format!("tx unlesbar: {e}"))?;
    tx.clone().pruefe(&netz).map_err(|e| format!("Die Unterschrift trägt nicht: {e}"))?;
    Ok((netz, tx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schluessel(n: u8) -> BlsSecretKey {
        BlsSecretKey::aus_ausgangsmaterial(&[n; 32]).unwrap()
    }

    fn auftrag(von: &BlsSecretKey) -> Auftrag {
        Auftrag {
            netzwert: Hash::from_bytes([7; 32]),
            absender: von.public_key().unwrap(),
            nonce: 3,
            max_gebuehr: 900,
            anweisung: Anweisung::Ueberweisung {
                nach: Address::aus_schluessel(&schluessel(9).public_key().unwrap()),
                betrag: 5_000_000,
            },
        }
    }

    /// Der ganze Weg: Auftrag als Text, unterschrieben, als Text, gelesen
    /// und geprüft; heraus kommt dieselbe Transaktion wie direkt
    /// unterschrieben.
    #[test]
    fn der_weg_ueber_zwei_dateien_ergibt_dieselbe_transaktion() {
        let sk = schluessel(1);
        let a = auftrag(&sk);
        let gelesen = Auftrag::aus_text(&a.als_text()).unwrap();
        assert_eq!(gelesen, a);
        let tx = gelesen.unterschreiben(&sk).unwrap();
        let direkt = Transaktion::signiere_mit_grenze(&a.netzwert, &sk, a.nonce, a.max_gebuehr, a.anweisung.clone()).unwrap();
        assert_eq!(tx, direkt);
        let (netz, zurueck) = unterschrift_aus_text(&unterschrift_als_text(&a.netzwert, &tx)).unwrap();
        assert_eq!(netz, a.netzwert);
        assert_eq!(zurueck, tx);
    }

    /// Ein fremder Schlüssel unterschreibt nichts.
    #[test]
    fn ein_fremder_schluessel_unterschreibt_nicht() {
        let a = auftrag(&schluessel(1));
        assert!(a.unterschreiben(&schluessel(2)).unwrap_err().contains("nichts unterschrieben"));
    }

    /// ⚑ Die Beschreibung kommt aus den Bytes: Ein geänderter Kommentar
    /// ändert nichts, eine geänderte Anweisung ändert die Beschreibung.
    #[test]
    fn die_beschreibung_kommt_aus_den_bytes_nicht_aus_dem_kommentar() {
        let a = auftrag(&schluessel(1));
        let text = a.als_text();
        let gelogen = text.lines().map(|z| if z.starts_with('#') { "# Überweisung von 0,01 MYL an einen Freund" } else { z }).collect::<Vec<_>>().join("\n");
        let b = Auftrag::aus_text(&gelogen).unwrap();
        assert_eq!(b.beschreibung(), a.beschreibung());
        assert!(b.beschreibung().contains("5 MYL") || b.beschreibung().contains("5,"), "{}", b.beschreibung());
    }

    /// Eine verstümmelte oder umgeschriebene Unterschrift fällt beim Lesen auf.
    #[test]
    fn eine_veraenderte_unterschrift_faellt_auf() {
        let sk = schluessel(1);
        let a = auftrag(&sk);
        let tx = a.unterschreiben(&sk).unwrap();
        // anderer Netzwert: Die Unterschrift trägt nicht.
        let falsch = unterschrift_als_text(&Hash::from_bytes([8; 32]), &tx);
        assert!(unterschrift_aus_text(&falsch).unwrap_err().contains("trägt nicht"));
        // Betrag umgeschrieben: dito.
        let mut anders = tx.clone();
        anders.anweisung = Anweisung::Ueberweisung { nach: Address::aus_schluessel(&schluessel(9).public_key().unwrap()), betrag: 6_000_000 };
        assert!(unterschrift_aus_text(&unterschrift_als_text(&a.netzwert, &anders)).is_err());
    }

    /// Falsche Kopfzeile, doppeltes oder fehlendes Feld: kein Auftrag.
    #[test]
    fn ein_kaputter_auftrag_wird_benannt() {
        let a = auftrag(&schluessel(1)).als_text();
        assert!(Auftrag::aus_text(&a.replace(AUFTRAG_KOPF, "myelith-auftrag-2")).unwrap_err().contains("Kopf"));
        assert!(Auftrag::aus_text(&a.replace("nonce 3\n", "")).unwrap_err().contains("nonce"));
        assert!(Auftrag::aus_text(&format!("{a}nonce 4\n")).unwrap_err().contains("zweimal"));
        assert!(Auftrag::aus_text(&unterschrift_als_text(&Hash::from_bytes([0; 32]), &auftrag(&schluessel(1)).unterschreiben(&schluessel(1)).unwrap())).is_err());
    }
}
