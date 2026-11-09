//! **Adressen als Text**: bech32m mit einem Präfix je Netz, und der Pfad,
//! aus dem ein Konto entsteht (2026-11-08).
//!
//! ⚑ **Gespeichert und signiert wird weiter die Bytefolge.** Das hier ist nur,
//! was ein Mensch sieht und abtippt: Eine Prüfsumme fängt Tippfehler, und das
//! Präfix sorgt dafür, dass eine Adresse eines Testnetzes nie als eine des
//! Hauptnetzes durchgeht.
//!
//! Das Format steht in BIP-173 und BIP-350; die Prüfvektoren unten stammen
//! von dort.

use crate::ids::Address;

/// Präfix im Hauptnetz.
pub const PRAEFIX_HAUPTNETZ: &str = "myl";
/// Präfix in jedem Testnetz und auf der Probekette.
pub const PRAEFIX_TESTNETZ: &str = "tmyl";

/// ⚑ **Die Coin-Nummer im Ableitungspfad** `m/12381/<coin>/<konto>/0`: die
/// ASCII-Werte von `M`, `Y`, `L` nebeneinander (77, 89, 76).
pub const COIN_NUMMER: u32 = 778_976;

const ZEICHEN: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const KONSTANTE_BECH32M: u32 = 0x2bc8_30a3;
const LAENGE_PRUEFSUMME: usize = 6;
const LAENGE_HOECHSTENS: usize = 90;

/// Warum ein Text keine Adresse ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Adressfehler {
    /// Länge, Zeichen, gemischte Schreibweise, fehlendes Trennzeichen.
    Form,
    /// Die Prüfsumme stimmt nicht: ein Tippfehler.
    Pruefsumme,
    /// Eine gültige Adresse eines anderen Netzes.
    AnderesNetz { gefunden: String, erwartet: String },
    /// Die Nutzlast sind keine 32 Bytes.
    Laenge,
}

impl std::fmt::Display for Adressfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Form => write!(f, "das ist keine Adresse"),
            Self::Pruefsumme => write!(f, "die Prüfsumme stimmt nicht, wahrscheinlich ein Tippfehler"),
            Self::AnderesNetz { gefunden, erwartet } => {
                write!(f, "eine Adresse des Netzes `{gefunden}`, erwartet wird `{erwartet}`")
            }
            Self::Laenge => write!(f, "die Adresse hat die falsche Länge"),
        }
    }
}

fn polymod(werte: impl Iterator<Item = u8>) -> u32 {
    const GEN: [u32; 5] = [0x3b6a_57b2, 0x2650_8e6d, 0x1ea1_19fa, 0x3d42_33dd, 0x2a14_62b3];
    let mut chk: u32 = 1;
    for v in werte {
        let oben = chk >> 25;
        chk = ((chk & 0x01ff_ffff) << 5) ^ u32::from(v);
        for (i, g) in GEN.iter().enumerate() {
            if (oben >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    chk
}

fn praefix_werte(praefix: &[u8]) -> Vec<u8> {
    let mut v: Vec<u8> = praefix.iter().map(|b| b >> 5).collect();
    v.push(0);
    v.extend(praefix.iter().map(|b| b & 31));
    v
}

fn pruefsumme(praefix: &[u8], daten: &[u8]) -> [u8; LAENGE_PRUEFSUMME] {
    let mut v = praefix_werte(praefix);
    v.extend_from_slice(daten);
    v.extend_from_slice(&[0; LAENGE_PRUEFSUMME]);
    let m = polymod(v.into_iter()) ^ KONSTANTE_BECH32M;
    std::array::from_fn(|i| ((m >> (5 * (LAENGE_PRUEFSUMME - 1 - i))) & 31) as u8)
}

/// Bytes zu Fünfergruppen, mit Auffüllen.
fn zu_fuenf(bytes: &[u8]) -> Vec<u8> {
    let mut aus = Vec::with_capacity(bytes.len() * 8 / 5 + 1);
    let (mut speicher, mut bits) = (0u32, 0u32);
    for b in bytes {
        speicher = (speicher << 8) | u32::from(*b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            aus.push(((speicher >> bits) & 31) as u8);
        }
        speicher &= (1 << bits) - 1;
    }
    if bits > 0 {
        aus.push(((speicher << (5 - bits)) & 31) as u8);
    }
    aus
}

/// Fünfergruppen zu Bytes. ⛔️ Die Füllbits müssen null sein und weniger als
/// fünf; sonst gäbe es zwei Schreibweisen derselben Adresse.
fn zu_acht(fuenfer: &[u8]) -> Option<Vec<u8>> {
    let mut aus = Vec::with_capacity(fuenfer.len() * 5 / 8);
    let (mut speicher, mut bits) = (0u32, 0u32);
    for f in fuenfer {
        speicher = (speicher << 5) | u32::from(*f);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            aus.push(((speicher >> bits) & 255) as u8);
        }
        speicher &= (1 << bits) - 1;
    }
    (bits < 5 && speicher == 0).then_some(aus)
}

/// Präfix und Datenteil eines bech32m-Texts, in Kleinschreibung.
fn zerlegen(text: &str) -> Result<(String, Vec<u8>), Adressfehler> {
    if text.len() > LAENGE_HOECHSTENS || !text.is_ascii() || !text.bytes().all(|b| (33..=126).contains(&b)) {
        return Err(Adressfehler::Form);
    }
    if text.bytes().any(|b| b.is_ascii_lowercase()) && text.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(Adressfehler::Form);
    }
    let klein = text.to_ascii_lowercase();
    let trenner = klein.rfind('1').ok_or(Adressfehler::Form)?;
    let (praefix, rest) = (&klein[..trenner], &klein[trenner + 1..]);
    if praefix.is_empty() || praefix.len() > 83 || rest.len() < LAENGE_PRUEFSUMME {
        return Err(Adressfehler::Form);
    }
    let daten: Vec<u8> = rest
        .bytes()
        .map(|b| ZEICHEN.iter().position(|z| *z == b).map(|p| p as u8))
        .collect::<Option<_>>()
        .ok_or(Adressfehler::Form)?;
    let mut v = praefix_werte(praefix.as_bytes());
    v.extend_from_slice(&daten);
    if polymod(v.into_iter()) != KONSTANTE_BECH32M {
        return Err(Adressfehler::Pruefsumme);
    }
    let nutz = daten.len() - LAENGE_PRUEFSUMME;
    Ok((praefix.to_string(), daten[..nutz].to_vec()))
}

/// Eine Adresse als Text für das Netz mit diesem Präfix.
pub fn als_text(adresse: &Address, praefix: &str) -> String {
    let daten = zu_fuenf(adresse.as_bytes());
    let summe = pruefsumme(praefix.as_bytes(), &daten);
    let mut aus = String::with_capacity(praefix.len() + 1 + daten.len() + LAENGE_PRUEFSUMME);
    aus.push_str(praefix);
    aus.push('1');
    aus.extend(daten.iter().chain(summe.iter()).map(|d| char::from(ZEICHEN[usize::from(*d)])));
    aus
}

/// ⚑ **Liest eine Adresse für das Netz mit diesem Präfix.** Eine Adresse
/// eines anderen Netzes ist ein eigener Fehler und wird nie angenommen.
/// 64 Hexzeichen gehen weiter durch (Skripte, ältere Notizen); sie haben
/// keine Prüfsumme.
pub fn aus_text(text: &str, praefix: &str) -> Result<Address, Adressfehler> {
    let text = text.trim();
    if text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Address::from_hex(text).map_err(|_| Adressfehler::Form);
    }
    let (gefunden, daten) = zerlegen(text)?;
    if gefunden != praefix {
        return Err(Adressfehler::AnderesNetz { gefunden, erwartet: praefix.to_string() });
    }
    let bytes = zu_acht(&daten).ok_or(Adressfehler::Form)?;
    let roh: [u8; 32] = bytes.try_into().map_err(|_| Adressfehler::Laenge)?;
    Ok(Address::new(roh))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_gueltigen_vektoren_aus_bip_350_tragen() {
        for t in [
            "A1LQFN3A",
            "a1lqfn3a",
            "abcdef1l7aum6echk45nj3s0wdvt2fg8x9yrzpqzd3ryx",
            "split1checkupstagehandshakeupstreamerranterredcaperredlc445v",
            "?1v759aa",
        ] {
            assert!(zerlegen(t).is_ok(), "{t}");
        }
    }

    #[test]
    fn die_ungueltigen_vektoren_aus_bip_350_fallen() {
        for t in ["qyrz8wqd2c9m", "1qyrz8wqd2c9m", "y1b0jsk6g", "lt1igcx5c0", "in1muywd", "mm1crxm3i", "au1s5cgom", "16plkw9", "1p2gdwpf"] {
            assert!(zerlegen(t).is_err(), "{t}");
        }
    }

    #[test]
    fn eine_adresse_kommt_unveraendert_zurueck() {
        let a = Address::new(std::array::from_fn(|i| (i as u8).wrapping_mul(37)));
        for p in [PRAEFIX_HAUPTNETZ, PRAEFIX_TESTNETZ] {
            let t = als_text(&a, p);
            assert!(t.starts_with(&format!("{p}1")), "{t}");
            assert_eq!(aus_text(&t, p), Ok(a));
            assert_eq!(aus_text(&t.to_ascii_uppercase(), p), Ok(a), "Grossschreibung ist dieselbe Adresse");
        }
    }

    /// ⛔️ Jedes einzelne falsche Zeichen fällt auf.
    #[test]
    fn jeder_einzelne_tippfehler_faellt_auf() {
        let a = Address::new([0x5a; 32]);
        let t = als_text(&a, PRAEFIX_TESTNETZ);
        for i in PRAEFIX_TESTNETZ.len() + 1..t.len() {
            for z in ZEICHEN {
                if t.as_bytes()[i] == *z {
                    continue;
                }
                let mut b = t.clone().into_bytes();
                b[i] = *z;
                let falsch = String::from_utf8(b).expect("ascii");
                assert_eq!(aus_text(&falsch, PRAEFIX_TESTNETZ), Err(Adressfehler::Pruefsumme), "{falsch}");
            }
        }
    }

    /// ⛔️ Eine Adresse eines anderen Netzes geht nie durch.
    #[test]
    fn eine_adresse_eines_anderen_netzes_wird_abgewiesen() {
        let a = Address::new([7; 32]);
        let t = als_text(&a, PRAEFIX_TESTNETZ);
        assert!(matches!(aus_text(&t, PRAEFIX_HAUPTNETZ), Err(Adressfehler::AnderesNetz { .. })));
    }

    #[test]
    fn hex_geht_weiter_durch() {
        let a = Address::new([9; 32]);
        assert_eq!(aus_text(&a.to_hex(), PRAEFIX_TESTNETZ), Ok(a));
    }

    /// Füllbits, die nicht null sind, wären eine zweite Schreibweise.
    #[test]
    fn fuellbits_muessen_null_sein() {
        assert!(zu_acht(&[0b11111, 0b00001]).is_none());
    }
}
