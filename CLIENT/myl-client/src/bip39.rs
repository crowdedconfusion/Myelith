//! **Wiederherstellungswoerter nach BIP-39** (Festlegung des Projektinhabers,
//! 2026-10-26: der etablierte Standard, zwoelf englische Woerter).
//!
//! # ⚑ Was hier steht, und was nicht
//!
//! - **Woerter**: 128 Bit Zufall und 4 Bit Pruefsumme (die ersten Bits von
//!   SHA-256 ueber den Zufall), in zwoelf Gruppen zu 11 Bit, je ein Wort der
//!   englischen Liste aus BIP-39 (`bip39_english.txt`, unveraendert, Lizenz
//!   MIT, SHA-256 `2f5eed53…dbda`; die Probe haelt den Abdruck fest).
//! - **Seed**: PBKDF2-HMAC-SHA512 ueber die Woerter mit dem Salz
//!   `mnemonic` und 2048 Runden, 64 Bytes, wie BIP-39 es festlegt.
//! - **Schluessel**: Aus dem Seed leitet `BlsSecretKey::aus_ausgangsmaterial` (IETF
//!   KeyGen) den Schluessel nach EIP-2333 und EIP-2334 ab; das steht in
//!   `crate::wallet`.
//!
//! ⚑ **HMAC und PBKDF2 sind hier selbst geschrieben**, auf `sha2`, das der
//! Client schon baut: zwanzig Zeilen gegen eine neue Fremdkiste. Geprueft
//! gegen RFC 4231 und die Pruefvektoren von BIP-39.
//!
//! ⚠️ **Keine Passphrase** (das „25. Wort"): Der Client nimmt die leere. Die
//! Funktion kennt sie, damit die Pruefvektoren laufen.

use sha2::{Digest, Sha256, Sha512};

/// Die Liste, unveraendert aus BIP-39.
const LISTE: &str = include_str!("bip39_english.txt");
/// Wie viele Woerter ein Wiederherstellungsschluessel hat.
pub const WOERTER: usize = 12;
/// Bytes Zufall dahinter.
pub const ENTROPIE_BYTES: usize = 16;

/// Die 2048 Woerter.
pub fn liste() -> Vec<&'static str> {
    LISTE.lines().map(str::trim).filter(|w| !w.is_empty()).collect()
}

/// **Zwoelf Woerter aus 16 Bytes Zufall.**
pub fn woerter(entropie: &[u8; ENTROPIE_BYTES]) -> Vec<&'static str> {
    let liste = liste();
    let pruef = Sha256::digest(entropie)[0] >> 4;
    // 132 Bit: Zufall, dann die vier Pruefbits.
    let mut bits: Vec<bool> = entropie.iter().flat_map(|b| (0..8).rev().map(move |i| b >> i & 1 == 1)).collect();
    bits.extend((0..4).rev().map(|i| pruef >> i & 1 == 1));
    bits.chunks(11)
        .map(|c| liste[c.iter().fold(0usize, |n, &b| n << 1 | usize::from(b))])
        .collect()
}

/// **Der Zufall aus zwoelf Woertern**, mit Pruefung von Wortliste und
/// Pruefsumme. Gross- und Kleinschreibung und Leerraum zaehlen nicht.
pub fn entropie(text: &str) -> Result<[u8; ENTROPIE_BYTES], String> {
    let liste = liste();
    let woerter: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    if woerter.len() != WOERTER {
        return Err(format!("Es sind {} Wörter, erwartet sind {WOERTER}.", woerter.len()));
    }
    let mut bits: Vec<bool> = Vec::with_capacity(132);
    for (n, w) in woerter.iter().enumerate() {
        let i = liste
            .iter()
            .position(|x| x == w)
            .ok_or_else(|| format!("Wort {} („{w}“) steht nicht in der Liste von BIP-39.", n + 1))?;
        bits.extend((0..11).rev().map(|s| i >> s & 1 == 1));
    }
    let mut e = [0u8; ENTROPIE_BYTES];
    for (i, b) in e.iter_mut().enumerate() {
        *b = bits[i * 8..i * 8 + 8].iter().fold(0u8, |n, &x| n << 1 | u8::from(x));
    }
    let pruef = bits[128..].iter().fold(0u8, |n, &x| n << 1 | u8::from(x));
    if pruef != Sha256::digest(e)[0] >> 4 {
        return Err("Die Prüfsumme stimmt nicht: Ein Wort ist falsch oder vertauscht.".into());
    }
    Ok(e)
}

/// **Der Seed** nach BIP-39: PBKDF2-HMAC-SHA512, Salz `mnemonic` und
/// Passphrase, 2048 Runden.
pub fn seed(woerter: &str, passphrase: &str) -> [u8; 64] {
    let satz = woerter.split_whitespace().map(str::to_lowercase).collect::<Vec<_>>().join(" ");
    pbkdf2_sha512(satz.as_bytes(), format!("mnemonic{passphrase}").as_bytes(), 2048)
}

/// HMAC-SHA512 nach RFC 2104.
fn hmac_sha512(schluessel: &[u8], daten: &[u8]) -> [u8; 64] {
    const BLOCK: usize = 128;
    let mut k = [0u8; BLOCK];
    if schluessel.len() > BLOCK {
        k[..64].copy_from_slice(&Sha512::digest(schluessel));
    } else {
        k[..schluessel.len()].copy_from_slice(schluessel);
    }
    let mut innen = Sha512::new();
    innen.update(k.map(|b| b ^ 0x36));
    innen.update(daten);
    let mut aussen = Sha512::new();
    aussen.update(k.map(|b| b ^ 0x5c));
    aussen.update(innen.finalize());
    aussen.finalize().into()
}

/// PBKDF2 nach RFC 8018 mit HMAC-SHA512, ein Block (64 Bytes).
fn pbkdf2_sha512(passwort: &[u8], salz: &[u8], runden: u32) -> [u8; 64] {
    let mut s = salz.to_vec();
    s.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac_sha512(passwort, &s);
    let mut t = u;
    for _ in 1..runden {
        u = hmac_sha512(passwort, &u);
        for (a, b) in t.iter_mut().zip(u.iter()) {
            *a ^= b;
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// ⛔️ **Die Liste ist die von BIP-39, Byte für Byte.** Wer sie ändert,
    /// macht jeden notierten Schlüssel unbrauchbar.
    #[test]
    fn die_liste_ist_die_von_bip39() {
        assert_eq!(hex(&Sha256::digest(LISTE.as_bytes())), "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda");
        let l = liste();
        assert_eq!(l.len(), 2048);
        assert_eq!((l[0], l[2047]), ("abandon", "zoo"));
    }

    /// RFC 4231, Testfall 1.
    #[test]
    fn hmac_wie_rfc_4231() {
        assert_eq!(
            hex(&hmac_sha512(&[0x0b; 20], b"Hi There")),
            "87aa7cdea5ef619d4ff0b4241a1d6cb02379f4e2ce4ec2787ad0b30545e17cdedaa833b7d6b8a702038b274eaea3f4e4be9d914eeb61f1702e696c203a126854"
        );
    }

    /// ⚑ **Die Prüfvektoren von BIP-39** (Passphrase `TREZOR`): Woerter und
    /// Seed, hin und zurueck.
    #[test]
    fn die_pruefvektoren_von_bip39() {
        for (e, w, s) in [
            ([0x00u8; 16], "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
             "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04"),
            ([0x7f; 16], "legal winner thank year wave sausage worth useful legal winner thank yellow",
             "2e8905819b8723fe2c1d161860e5ee1830318dbf49a83bd451cfb8440c28bd6fa457fe1296106559a3c80937a1c1069be3a3a5bd381ee6260e8d9739fce1f607"),
            ([0x80; 16], "letter advice cage absurd amount doctor acoustic avoid letter advice cage above",
             "d71de856f81a8acc65e6fc851a38d4d7ec216fd0796d0a6827a3ad6ed5511a30fa280f12eb2e47ed2ac03b5c462a0358d18d69fe4f985ec81778c1b370b652a8"),
            ([0xff; 16], "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong",
             "ac27495480225222079d7be181583751e86f571027b0497b5b5d11218e0a8a13332572917f0f8e5a589620c6f15b11c61dee327651a14c34e18231052e48c069"),
        ] {
            assert_eq!(woerter(&e).join(" "), w);
            assert_eq!(entropie(w).expect("zurueck"), e);
            assert_eq!(hex(&seed(w, "TREZOR")), s);
        }
    }

    /// Ein vertauschtes Wort faellt an der Pruefsumme auf, ein fremdes an
    /// der Liste, eine falsche Zahl an der Zahl.
    #[test]
    fn falsche_woerter_werden_erkannt() {
        let gut = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        assert!(entropie("legal winner thank year wave sausage worth useful legal winner yellow thank").is_err());
        assert!(entropie(&gut.replace("wave", "welle")).unwrap_err().contains("Wort 5"));
        assert!(entropie("legal winner").is_err());
        assert_eq!(entropie(&gut.to_uppercase()).expect("gross"), [0x7f; 16]);
    }

    /// ⚑ **Jede Verstuemmelung scheitert sauber oder ist kanonisch** (CLIENT
    /// 7.15, Parser der Woerter): 4 000 Abwandlungen der Normvektoren aus
    /// einem festen Generator (Tausch, Ersatz durch ein Listenwort, Loeschen,
    /// Verdoppeln, Grossschreibung, Leerraum, fremde Zeichen). Keine darf
    /// abstuerzen; was angenommen wird, ergibt genau die eingegebenen Woerter
    /// in Kleinschreibung, eine zweite Schreibweise fuer dieselbe Entropie
    /// gibt es also nicht. Gegenprobe: Einige Abwandlungen gehen durch (ein
    /// Ersatzwort trifft die Pruefsumme in rund einem von sechzehn Faellen),
    /// sonst saehe die Probe die Annahme nie.
    #[test]
    fn jede_verstuemmelung_scheitert_sauber_oder_ist_kanonisch() {
        let liste = liste();
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        let mut zufall = |n: usize| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % n as u64) as usize
        };
        let grundlagen: Vec<Vec<String>> =
            [[0x00u8; 16], [0x7f; 16], [0x80; 16], [0xff; 16]].iter().map(|e| woerter(e).iter().map(|w| w.to_string()).collect()).collect();
        let mut angenommen = 0;
        for _ in 0..4_000 {
            let mut w = grundlagen[zufall(grundlagen.len())].clone();
            for _ in 0..1 + zufall(3) {
                let i = zufall(w.len().max(1));
                match zufall(7) {
                    0 if w.len() > 1 => { let j = zufall(w.len()); w.swap(i, j); }
                    1 if !w.is_empty() => w[i] = liste[zufall(liste.len())].to_string(),
                    2 if !w.is_empty() => { w.remove(i); }
                    3 if !w.is_empty() => { let d = w[i].clone(); w.insert(i, d); }
                    4 if !w.is_empty() => w[i] = w[i].to_uppercase(),
                    5 if !w.is_empty() => {
                        // Nur an Zeichengrenzen: Ein eingefuegtes `ä` hat zwei Bytes.
                        let stellen: Vec<usize> = w[i].char_indices().map(|(b, _)| b).chain([w[i].len()]).collect();
                        let an = stellen[zufall(stellen.len())];
                        let zeichen = ['ä', '-', '1', ' ', '\t'][zufall(5)];
                        w[i].insert(an, zeichen);
                    }
                    _ => {}
                }
            }
            let text = w.join(if zufall(4) == 0 { "  \n " } else { " " });
            if let Ok(e) = entropie(&text) {
                angenommen += 1;
                let eingegeben: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
                assert_eq!(woerter(&e).join(" "), eingegeben.join(" "), "angenommen, aber nicht kanonisch: {text:?}");
            }
        }
        assert!(angenommen > 0, "keine Abwandlung ging durch; die Probe prueft die Annahme nie");
    }
}
