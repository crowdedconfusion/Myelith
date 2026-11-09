//! **Der Keystore nach EIP-2335**: ein BLS-Schluessel, verschluesselt unter
//! einem Passwort, als JSON-Datei.
//!
//! Das Verfahren steht fest (Festlegung des Projektinhabers, 2026-10-27):
//! scrypt fuer das Passwort, AES-128-CTR fuer den Schluessel, SHA-256 als
//! Pruefsumme. Geschrieben wird immer scrypt; gelesen auch PBKDF2, damit eine
//! Datei aus einem anderen Werkzeug nach derselben Norm geht.
//!
//! ⚑ **Verschluesselt wird der Schluessel und nicht der Seed.** Die Norm nennt
//! genau das, und ein Seed oeffnete jedes weitere Konto derselben Woerter. Die
//! Sicherung ist die Woerterliste.
//!
//! ⛔️ **Die Parameter einer gelesenen Datei sind fremde Werte.** Ohne Deckel
//! liesse eine manipulierte Datei scrypt Terabyte anfordern; darum
//! [`speicher_ok`] vor der Rechnung.
//!
//! ⚠️ **Die Norm schuetzt die Datei, nicht den laufenden Rechner.** Wer das
//! Passwort mitliest oder den Speicher des Prozesses, hat den Schluessel.

use aes::cipher::{KeyIvInit, StreamCipher};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

/// `n` beim Schreiben, wie im Beispiel der Norm.
pub const SCRYPT_LOG_N: u8 = 18;
const SCRYPT_R: u32 = 8;
const SCRYPT_P: u32 = 1;

/// Hoechstens so viel Speicher fuer scrypt beim Lesen (1 GiB).
const SCRYPT_SPEICHER_HOECHSTENS: u128 = 1 << 30;
/// Hoechstens so viele Runden fuer PBKDF2 beim Lesen.
const PBKDF2_RUNDEN_HOECHSTENS: u32 = 1 << 24;
/// Mindestlaenge eines neuen Passworts, in Zeichen.
pub const PASSWORT_MINDESTENS: usize = 8;

const DKLEN: u32 = 32;

/// Die drei Module der Norm haben dieselbe Form; `params` je nach Funktion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modul<P> {
    pub function: String,
    pub params: P,
    pub message: String,
}

/// Die Parameter der KDF: scrypt (`n`, `r`, `p`) oder PBKDF2 (`c`, `prf`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KdfParameter {
    Scrypt { dklen: u32, n: u64, p: u32, r: u32, salt: String },
    Pbkdf2 { dklen: u32, c: u32, prf: String, salt: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Leer {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IvParameter {
    pub iv: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Krypto {
    pub kdf: Modul<KdfParameter>,
    pub checksum: Modul<Leer>,
    pub cipher: Modul<IvParameter>,
}

/// Eine Keystore-Datei.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keystore {
    pub crypto: Krypto,
    #[serde(default)]
    pub description: String,
    pub pubkey: String,
    pub path: String,
    pub uuid: String,
    pub version: u32,
}

/// **Das Passwort als Bytes**: NFKD, ohne C0, C1 und Delete (Norm, Abschnitt
/// „Password requirements“).
pub fn passwort_bytes(passwort: &str) -> Zeroizing<Vec<u8>> {
    let sauber: Zeroizing<String> =
        Zeroizing::new(passwort.nfkd().filter(|c| !matches!(*c as u32, 0..=0x1f | 0x7f..=0x9f)).collect());
    Zeroizing::new(sauber.as_bytes().to_vec())
}

/// Ob scrypt mit diesen Werten die Speichergrenze einhaelt.
fn speicher_ok(n: u64, r: u32, p: u32) -> bool {
    n >= 2 && n.is_power_of_two() && r >= 1 && p >= 1 && 128 * u128::from(n) * u128::from(r) * u128::from(p.max(1)) <= SCRYPT_SPEICHER_HOECHSTENS
}

fn hex_lesen(s: &str, was: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{was}: kein Hex"));
    }
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| format!("{was}: kein Hex"))).collect()
}

fn hex_text(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn ableiten(p: &KdfParameter, function: &str, passwort: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    let mut schluessel = Zeroizing::new([0u8; 32]);
    match (function, p) {
        ("scrypt", KdfParameter::Scrypt { dklen, n, p: par, r, salt }) => {
            if *dklen != DKLEN || !speicher_ok(*n, *r, *par) {
                return Err("scrypt: Parameter außerhalb der Grenzen".into());
            }
            let salz = hex_lesen(salt, "Salz")?;
            let params = scrypt::Params::new(n.trailing_zeros() as u8, *r, *par, 32).map_err(|f| format!("scrypt: {f}"))?;
            scrypt::scrypt(passwort, &salz, &params, schluessel.as_mut()).map_err(|f| format!("scrypt: {f}"))?;
        }
        ("pbkdf2", KdfParameter::Pbkdf2 { dklen, c, prf, salt }) => {
            if *dklen != DKLEN || prf != "hmac-sha256" || *c == 0 || *c > PBKDF2_RUNDEN_HOECHSTENS {
                return Err("pbkdf2: Parameter außerhalb der Grenzen".into());
            }
            let salz = hex_lesen(salt, "Salz")?;
            pbkdf2::pbkdf2_hmac::<Sha256>(passwort, &salz, *c, schluessel.as_mut());
        }
        _ => return Err(format!("unbekannte KDF: {function}")),
    }
    Ok(schluessel)
}

fn pruefsumme(schluessel: &[u8; 32], geheimtext: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(&schluessel[16..32]);
    h.update(geheimtext);
    h.finalize().into()
}

fn aes_anwenden(schluessel: &[u8; 32], iv: &[u8; 16], daten: &mut [u8]) {
    type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;
    let k: &[u8; 16] = schluessel[..16].try_into().expect("16 von 32 Bytes");
    Aes128Ctr::new(k.into(), iv.into()).apply_keystream(daten);
}

fn zufall(b: &mut [u8]) -> Result<(), String> {
    getrandom::fill(b).map_err(|f| format!("kein Zufall: {f}"))
}

/// Eine zufaellige UUID, Version 4.
fn uuid() -> Result<String, String> {
    let mut b = [0u8; 16];
    zufall(&mut b)?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex_text(&b);
    Ok(format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..]))
}

/// **Verschluesselt einen Schluessel** mit scrypt nach den Werten oben.
pub fn verschluesseln(geheim: &[u8; 32], pubkey: &[u8], pfad: &str, beschreibung: &str, passwort: &str) -> Result<Keystore, String> {
    if passwort.chars().count() < PASSWORT_MINDESTENS {
        return Err(format!("Das Passwort braucht mindestens {PASSWORT_MINDESTENS} Zeichen."));
    }
    verschluesseln_mit(geheim, pubkey, pfad, beschreibung, passwort, SCRYPT_LOG_N)
}

fn verschluesseln_mit(geheim: &[u8; 32], pubkey: &[u8], pfad: &str, beschreibung: &str, passwort: &str, log_n: u8) -> Result<Keystore, String> {
    let (mut salz, mut iv) = ([0u8; 32], [0u8; 16]);
    zufall(&mut salz)?;
    zufall(&mut iv)?;
    let kdf = KdfParameter::Scrypt { dklen: DKLEN, n: 1 << log_n, p: SCRYPT_P, r: SCRYPT_R, salt: hex_text(&salz) };
    let passwort = passwort_bytes(passwort);
    let schluessel = ableiten(&kdf, "scrypt", &passwort)?;
    let mut text = *geheim;
    aes_anwenden(&schluessel, &iv, &mut text);
    let summe = pruefsumme(&schluessel, &text);
    Ok(Keystore {
        crypto: Krypto {
            kdf: Modul { function: "scrypt".into(), params: kdf, message: String::new() },
            checksum: Modul { function: "sha256".into(), params: Leer {}, message: hex_text(&summe) },
            cipher: Modul { function: "aes-128-ctr".into(), params: IvParameter { iv: hex_text(&iv) }, message: hex_text(&text) },
        },
        description: beschreibung.to_string(),
        pubkey: hex_text(pubkey),
        path: pfad.to_string(),
        uuid: uuid()?,
        version: 4,
    })
}

/// Das Ergebnis, wenn das Passwort nicht passt.
pub const FALSCHES_PASSWORT: &str = "Das Passwort passt nicht.";

/// **Oeffnet einen Keystore.** Erst die Pruefsumme, dann die Entschluesselung,
/// wie die Norm es verlangt.
pub fn entschluesseln(k: &Keystore, passwort: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    if k.version != 4 {
        return Err(format!("Keystore-Fassung {} wird nicht gelesen", k.version));
    }
    if k.crypto.checksum.function != "sha256" || k.crypto.cipher.function != "aes-128-ctr" {
        return Err("Keystore mit unbekannten Funktionen".into());
    }
    let geheimtext = hex_lesen(&k.crypto.cipher.message, "Geheimtext")?;
    let iv: [u8; 16] = hex_lesen(&k.crypto.cipher.params.iv, "IV")?.try_into().map_err(|_| "IV: nicht 16 Bytes".to_string())?;
    let summe: [u8; 32] = hex_lesen(&k.crypto.checksum.message, "Pruefsumme")?.try_into().map_err(|_| "Pruefsumme: nicht 32 Bytes".to_string())?;
    if geheimtext.len() != 32 {
        return Err("Der Keystore traegt keinen Schluessel aus 32 Bytes.".into());
    }
    let passwort = passwort_bytes(passwort);
    let schluessel = ableiten(&k.crypto.kdf.params, &k.crypto.kdf.function, &passwort)?;
    // ⚑ Kein Geheimnis wird verglichen: Die Pruefsumme steht offen in der Datei.
    if pruefsumme(&schluessel, &geheimtext) != summe {
        return Err(FALSCHES_PASSWORT.into());
    }
    let mut klar = Zeroizing::new([0u8; 32]);
    klar.copy_from_slice(&geheimtext);
    aes_anwenden(&schluessel, &iv, klar.as_mut());
    Ok(klar)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORT: &str = "𝔱𝔢𝔰𝔱𝔭𝔞𝔰𝔰𝔴𝔬𝔯𝔡🔑";
    const GEHEIM: &str = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";

    /// Die Prüfvektoren der Norm, wörtlich.
    const SCRYPT: &str = r#"{"crypto":{"kdf":{"function":"scrypt","params":{"dklen":32,"n":262144,"p":1,"r":8,"salt":"d4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3"},"message":""},"checksum":{"function":"sha256","params":{},"message":"d2217fe5f3e9a1e34581ef8a78f7c9928e436d36dacc5e846690a5581e8ea484"},"cipher":{"function":"aes-128-ctr","params":{"iv":"264daa3f303d7259501c93d997d84fe6"},"message":"06ae90d55fe0a6e9c5c3bc5b170827b2e5cce3929ed3f116c2811e6366dfe20f"}},"description":"This is a test keystore that uses scrypt to secure the secret.","pubkey":"9612d7a727c9d0a22e185a1c768478dfe919cada9266988cb32359c11f2b7b27f4ae4040902382ae2910c15e2b420d07","path":"m/12381/60/3141592653/589793238","uuid":"1d85ae20-35c5-4611-98e8-aa14a633906f","version":4}"#;
    const PBKDF2: &str = r#"{"crypto":{"kdf":{"function":"pbkdf2","params":{"dklen":32,"c":262144,"prf":"hmac-sha256","salt":"d4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3"},"message":""},"checksum":{"function":"sha256","params":{},"message":"8a9f5d9912ed7e75ea794bc5a89bca5f193721d30868ade6f73043c6ea6febf1"},"cipher":{"function":"aes-128-ctr","params":{"iv":"264daa3f303d7259501c93d997d84fe6"},"message":"cee03fde2af33149775b7223e7845e4fb2c8ae1792e5f99fe9ecf474cc8c16ad"}},"description":"This is a test keystore that uses PBKDF2 to secure the secret.","pubkey":"9612d7a727c9d0a22e185a1c768478dfe919cada9266988cb32359c11f2b7b27f4ae4040902382ae2910c15e2b420d07","path":"m/12381/60/0/0","uuid":"64625def-3331-4eea-ab6f-782f3ed16a83","version":4}"#;

    #[test]
    fn das_passwort_wird_wie_in_der_norm_vorbereitet() {
        assert_eq!(hex_text(&passwort_bytes(PASSWORT)), "7465737470617373776f7264f09f9491");
        assert_eq!(&*passwort_bytes("a\u{7}b\u{9f}c\u{7f}"), b"abc", "C0, C1 und Delete fallen weg");
        assert_eq!(*passwort_bytes("\u{e9}"), *passwort_bytes("e\u{301}"), "NFKD: dieselbe Eingabe, derselbe Schluessel");
    }

    #[test]
    fn die_pruefvektoren_der_norm_gehen_auf() {
        for j in [SCRYPT, PBKDF2] {
            let k: Keystore = serde_json::from_str(j).expect("lesbar");
            let klar = entschluesseln(&k, PASSWORT).expect("oeffnet");
            assert_eq!(hex_text(klar.as_ref()), GEHEIM);
            assert_eq!(entschluesseln(&k, "falsch").unwrap_err(), FALSCHES_PASSWORT);
        }
    }

    /// Klein gerechnet, damit der Test nicht Sekunden je Fall braucht.
    fn probe(p: &str) -> Keystore {
        verschluesseln_mit(&[7; 32], &[1; 48], "m/12381/778976/0/0", "Probe", p, 4).expect("verschluesselt")
    }

    #[test]
    fn hin_und_zurueck_und_jede_stoerung_faellt_auf() {
        let k = probe("ein gutes Passwort");
        assert_eq!(*entschluesseln(&k, "ein gutes Passwort").unwrap(), [7; 32]);
        assert_eq!(entschluesseln(&k, "ein anderes Passwort").unwrap_err(), FALSCHES_PASSWORT);
        let mut b = k.clone();
        let mut m = b.crypto.cipher.message.into_bytes();
        m[0] = if m[0] == b'0' { b'1' } else { b'0' };
        b.crypto.cipher.message = String::from_utf8(m).unwrap();
        assert_eq!(entschluesseln(&b, "ein gutes Passwort").unwrap_err(), FALSCHES_PASSWORT, "ein gekipptes Bit");
        let a = probe("ein gutes Passwort");
        assert_ne!(k.crypto.kdf.params, a.crypto.kdf.params, "das Salz ist neu");
        assert_ne!(k.crypto.cipher.params, a.crypto.cipher.params, "der IV ist neu");
        assert_ne!(k.crypto.cipher.message, a.crypto.cipher.message);
        assert_ne!(k.uuid, a.uuid);
        assert_eq!(a.uuid.as_bytes()[14], b'4');
    }

    #[test]
    fn die_datei_steht_in_der_form_der_norm() {
        let j: serde_json::Value = serde_json::to_value(probe("ein gutes Passwort")).unwrap();
        assert_eq!(j["version"], 4);
        assert_eq!(j["crypto"]["kdf"]["function"], "scrypt");
        assert_eq!(j["crypto"]["kdf"]["params"]["n"], 16);
        assert_eq!(j["crypto"]["kdf"]["params"]["r"], 8);
        assert_eq!(j["crypto"]["kdf"]["params"]["dklen"], 32);
        assert_eq!(j["crypto"]["cipher"]["function"], "aes-128-ctr");
        assert_eq!(j["crypto"]["checksum"]["function"], "sha256");
        assert_eq!(j["crypto"]["kdf"]["message"], "");
        assert_eq!(j["path"], "m/12381/778976/0/0");
    }

    #[test]
    fn manipulierte_parameter_werden_nicht_gerechnet() {
        assert!(speicher_ok(1 << 18, 8, 1));
        assert!(!speicher_ok(1 << 30, 8, 1), "128 GiB");
        assert!(!speicher_ok(1 << 18, 8, 1 << 20));
        assert!(!speicher_ok(1000, 8, 1), "keine Zweierpotenz");
        assert!(!speicher_ok(0, 8, 1));
        let mut k = probe("ein gutes Passwort");
        if let KdfParameter::Scrypt { n, .. } = &mut k.crypto.kdf.params {
            *n = 1 << 40;
        }
        assert!(entschluesseln(&k, "ein gutes Passwort").unwrap_err().contains("Grenzen"));
        let mut k = probe("ein gutes Passwort");
        if let KdfParameter::Scrypt { dklen, .. } = &mut k.crypto.kdf.params {
            *dklen = 16;
        }
        assert!(entschluesseln(&k, "ein gutes Passwort").unwrap_err().contains("Grenzen"));
        let mut k = probe("ein gutes Passwort");
        k.crypto.kdf.params = KdfParameter::Pbkdf2 { dklen: 32, c: u32::MAX, prf: "hmac-sha256".into(), salt: "00".into() };
        k.crypto.kdf.function = "pbkdf2".into();
        assert!(entschluesseln(&k, "ein gutes Passwort").unwrap_err().contains("Grenzen"));
        let mut k = probe("ein gutes Passwort");
        k.version = 3;
        assert!(entschluesseln(&k, "ein gutes Passwort").is_err());
        let mut k = probe("ein gutes Passwort");
        k.crypto.cipher.message = "00".into();
        assert!(entschluesseln(&k, "ein gutes Passwort").is_err(), "kein Schluessel aus 32 Bytes");
    }

    #[test]
    fn ein_kurzes_passwort_wird_abgewiesen() {
        assert!(verschluesseln(&[7; 32], &[1; 48], "m", "", "kurz").unwrap_err().contains("mindestens"));
    }
}
