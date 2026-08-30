//! Binaer-Codec fuer Pipeline-Nachrichten
//! 
//! Format:
//! [Magic: 8 bytes]
//! [Header: 72 bytes]
//! [Payload: N bytes]
//! [Padding: 0-7 bytes]
//! 
//! Idempotenz: request_id + token_position + stage_id ist eindeutig.
//! Duplikate werden auf Empfaengerseite erkannt und ignoriert.

pub const MAGIC: &[u8; 8] = b"IINTPIPE";
pub const HEADER_SIZE: usize = 8 + 8 * 9 + 4; // magic + 9 u64 + crc

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageMeta {
    pub version: u64,
    pub theta_v_hash: u64,       // Trunkierter Hash fuer schnellen Vergleich
    pub request_id: u64,
    pub sequence_id: u64,        // Fuer Multi-Token-Streaming
    pub stage_id: u64,
    pub token_position: u64,     // Basis-Position der Nachricht im Kontext
    pub payload_len: u64,
    pub flags: u64,              // Bit 0: starts_generation, Bit 1: is_abort,
                                 // Bit 2: token_input (Payload = gepackte
                                 // Token-IDs statt Aktivierungen)
    pub reserved: u64,
    pub crc: u32,
}

/// Bit 0: Nach diesem Token beginnt die Generation (Feedback-Schleife).
pub const FLAG_STARTS_GENERATION: u64 = 0x1;
/// Bit 1: Request abbrechen.
pub const FLAG_ABORT: u64 = 0x2;
/// Bit 2: Payload sind gepackte Token-IDs (je Token zwei i16:
/// Low-/High-Hälften des u32-Token-IDs, little-endian) statt
/// Aktivierungs-Tensoren. Nur für Stages mit Embedding (Stage 0).
pub const FLAG_TOKEN_INPUT: u64 = 0x4;

/// Packt Token-IDs in das i16-Payload-Format (je ID zwei i16).
pub fn pack_tokens(tokens: &[u32]) -> Vec<i16> {
    let mut out = Vec::with_capacity(tokens.len() * 2);
    for t in tokens {
        out.push((t & 0xFFFF) as i16);
        out.push((t >> 16) as i16);
    }
    out
}

/// Entpackt Token-IDs aus dem i16-Payload-Format.
pub fn unpack_tokens(payload: &[i16]) -> Result<Vec<u32>, String> {
    if payload.len() % 2 != 0 {
        return Err("Token-Payload muss eine gerade Anzahl i16 haben".to_string());
    }
    let mut out = Vec::with_capacity(payload.len() / 2);
    // `as_chunks::<2>()` waere clippys Vorschlag, ist aber erst seit Rust
    // 1.88 stabil; die Schwester-Crates erklaeren MSRV 1.85.
    #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
    for pair in payload.chunks_exact(2) {
        let lo = pair[0] as u16 as u32;
        let hi = pair[1] as u16 as u32;
        out.push(lo | (hi << 16));
    }
    Ok(out)
}

impl MessageMeta {
    /// Eindeutiger Schluessel fuer Duplikaterkennung.
    pub fn dedup_key(&self) -> (u64, u64, u64) {
        (self.request_id, self.stage_id, self.token_position)
    }

    pub fn starts_generation(&self) -> bool {
        (self.flags & FLAG_STARTS_GENERATION) != 0
    }

    pub fn is_abort(&self) -> bool {
        (self.flags & FLAG_ABORT) != 0
    }

    pub fn is_token_input(&self) -> bool {
        (self.flags & FLAG_TOKEN_INPUT) != 0
    }
}

pub fn encode_message(meta: &MessageMeta, tensor: &[i16]) -> Vec<u8> {
    let payload: Vec<u8> = tensor.iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    
    let crc = crc32fast::hash(&payload);
    let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len() + 8);
    
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&meta.version.to_le_bytes());
    buf.extend_from_slice(&meta.theta_v_hash.to_le_bytes());
    buf.extend_from_slice(&meta.request_id.to_le_bytes());
    buf.extend_from_slice(&meta.sequence_id.to_le_bytes());
    buf.extend_from_slice(&meta.stage_id.to_le_bytes());
    buf.extend_from_slice(&meta.token_position.to_le_bytes());
    buf.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    buf.extend_from_slice(&meta.flags.to_le_bytes());
    buf.extend_from_slice(&meta.reserved.to_le_bytes());
    buf.extend_from_slice(&crc.to_le_bytes());
    buf.extend_from_slice(&payload);
    
    // 8-Byte Alignment
    let pad = (8 - (buf.len() % 8)) % 8;
    buf.extend(std::iter::repeat_n(0u8, pad));
    buf
}

pub fn decode_message(buf: &[u8]) -> Result<(MessageMeta, Vec<i16>), String> {
    if buf.len() < HEADER_SIZE {
        return Err(format!("Buffer zu klein: {} < {}", buf.len(), HEADER_SIZE));
    }
    if &buf[0..8] != MAGIC {
        return Err("Bad magic".to_string());
    }
    
    let mut off = 8;
    let read_u64 = |b: &[u8], o: &mut usize| {
        let val = u64::from_le_bytes(b[*o..*o+8].try_into().unwrap());
        *o += 8;
        val
    };
    
    let version = read_u64(buf, &mut off);
    let theta_v_hash = read_u64(buf, &mut off);
    let request_id = read_u64(buf, &mut off);
    let sequence_id = read_u64(buf, &mut off);
    let stage_id = read_u64(buf, &mut off);
    let token_position = read_u64(buf, &mut off);
    let payload_len = read_u64(buf, &mut off);
    let flags = read_u64(buf, &mut off);
    let reserved = read_u64(buf, &mut off);
    let crc = u32::from_le_bytes(buf[off..off+4].try_into().unwrap());
    off += 4;
    
    // Fund A13: `off + payload_len as usize` lief bei manipulierter
    // Laengenangabe ueber — Panic im Debug-Build, im Release-Build ein
    // Umlauf auf einen kleinen Wert, der die folgende Schranke passiert
    // haette. `payload_len` kommt ungeprueft von der Gegenstelle; ein
    // einzelnes Feld reichte, um einen Pipeline-Node abzuschiessen.
    let payload_end = match usize::try_from(payload_len).ok().and_then(|n| off.checked_add(n)) {
        Some(end) => end,
        None => {
            return Err(format!(
                "Payload-Laenge {} unplausibel (Ueberlauf)",
                payload_len
            ))
        }
    };
    if payload_end > buf.len() {
        return Err(format!("Payload ueberlaeuft Buffer: {} > {}", payload_end, buf.len()));
    }
    
    let payload = &buf[off..payload_end];
    let computed_crc = crc32fast::hash(payload) as u32;
    if crc != computed_crc {
        return Err(format!("CRC mismatch: {:08x} != {:08x}", crc, computed_crc));
    }
    
    if payload.len() % 2 != 0 {
        return Err("Ungerade Payload-Laenge".to_string());
    }
    
    #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
    let tensor: Vec<i16> = payload
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    
    let meta = MessageMeta {
        version, theta_v_hash, request_id, sequence_id, stage_id,
        token_position, payload_len, flags, reserved, crc,
    };
    
    Ok((meta, tensor))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(payload_len: u64) -> MessageMeta {
        MessageMeta {
            version: 1,
            theta_v_hash: 0xDEAD_BEEF,
            request_id: 7,
            sequence_id: 3,
            stage_id: 2,
            token_position: 11,
            payload_len,
            flags: 0,
            reserved: 0,
            crc: 0,
        }
    }

    #[test]
    fn rundtrip_erhaelt_metadaten_und_tensor() {
        let tensor: Vec<i16> = vec![-32768, -1, 0, 1, 32767];
        let buf = encode_message(&meta(tensor.len() as u64 * 2), &tensor);
        let (m, t) = decode_message(&buf).expect("dekodierbar");

        assert_eq!(t, tensor);
        assert_eq!(m.request_id, 7);
        assert_eq!(m.sequence_id, 3);
        assert_eq!(m.stage_id, 2);
        assert_eq!(m.token_position, 11);
        assert_eq!(m.theta_v_hash, 0xDEAD_BEEF);
        assert_eq!(m.payload_len, tensor.len() as u64 * 2);
    }

    #[test]
    fn leerer_tensor_ist_gueltig() {
        let buf = encode_message(&meta(0), &[]);
        let (m, t) = decode_message(&buf).expect("dekodierbar");
        assert!(t.is_empty());
        assert_eq!(m.payload_len, 0);
    }

    #[test]
    fn ausgabe_ist_acht_byte_ausgerichtet() {
        // Ungerade Tensorlaengen erzeugen Padding — die Ausrichtung ist
        // Vertrag des Formats (Direktzugriff ohne Kopie).
        for n in 0..8usize {
            let tensor: Vec<i16> = (0..n as i16).collect();
            let buf = encode_message(&meta(n as u64 * 2), &tensor);
            assert_eq!(buf.len() % 8, 0, "Laenge {} nicht ausgerichtet (n={})", buf.len(), n);
        }
    }

    #[test]
    fn kodierung_ist_deterministisch() {
        let tensor: Vec<i16> = vec![5, -5, 100];
        let a = encode_message(&meta(6), &tensor);
        let b = encode_message(&meta(6), &tensor);
        assert_eq!(a, b, "gleiche Eingabe muss bitgleiche Bytes liefern");
    }

    #[test]
    fn falsche_magic_wird_abgelehnt() {
        let mut buf = encode_message(&meta(2), &[1i16]);
        buf[0] = b'X';
        assert!(decode_message(&buf).is_err());
    }

    #[test]
    fn zu_kurzer_puffer_wird_abgelehnt() {
        let buf = encode_message(&meta(2), &[1i16]);
        for len in 0..HEADER_SIZE {
            assert!(
                decode_message(&buf[..len]).is_err(),
                "Puffer der Laenge {} muesste abgelehnt werden",
                len
            );
        }
    }

    /// Jedes gekippte Nutzlast-Bit muss die CRC-Pruefung ausloesen —
    /// das ist der Manipulationsschutz auf der Leitung.
    #[test]
    fn verfaelschte_nutzlast_wird_von_der_crc_gefangen() {
        let tensor: Vec<i16> = (0..32).collect();
        let original = encode_message(&meta(64), &tensor);

        for byte_idx in HEADER_SIZE..HEADER_SIZE + 64 {
            for bit in 0..8 {
                let mut buf = original.clone();
                buf[byte_idx] ^= 1 << bit;
                assert!(
                    decode_message(&buf).is_err(),
                    "Bitflip an Byte {} Bit {} blieb unentdeckt",
                    byte_idx,
                    bit
                );
            }
        }
    }

    #[test]
    fn ueberlaufende_payload_laenge_wird_abgelehnt() {
        let mut buf = encode_message(&meta(2), &[1i16]);
        // payload_len steht als 7. u64 nach der Magic.
        let off = 8 + 6 * 8;
        buf[off..off + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(decode_message(&buf).is_err());
    }

    #[test]
    fn flags_werden_korrekt_ausgewertet() {
        let mut m = meta(0);
        assert!(!m.starts_generation() && !m.is_abort() && !m.is_token_input());

        m.flags = FLAG_STARTS_GENERATION;
        assert!(m.starts_generation() && !m.is_abort() && !m.is_token_input());

        m.flags = FLAG_ABORT | FLAG_TOKEN_INPUT;
        assert!(!m.starts_generation() && m.is_abort() && m.is_token_input());
    }

    #[test]
    fn flags_ueberstehen_den_rundtrip() {
        let mut m = meta(0);
        m.flags = FLAG_STARTS_GENERATION | FLAG_TOKEN_INPUT;
        let buf = encode_message(&m, &[]);
        let (back, _) = decode_message(&buf).expect("dekodierbar");
        assert!(back.starts_generation());
        assert!(back.is_token_input());
        assert!(!back.is_abort());
    }

    #[test]
    fn dedup_key_unterscheidet_die_richtigen_felder() {
        let a = meta(0);
        let mut b = meta(0);
        b.token_position = 12;
        assert_ne!(a.dedup_key(), b.dedup_key());

        let mut c = meta(0);
        c.flags = FLAG_ABORT; // Flags gehoeren nicht zum Schluessel
        assert_eq!(a.dedup_key(), c.dedup_key());
    }

    #[test]
    fn token_rundtrip_ueber_den_ganzen_u32_bereich() {
        let tokens: Vec<u32> = vec![0, 1, 0x7FFF, 0x8000, 0xFFFF, 0x1_0000, 0x7FFF_FFFF, u32::MAX];
        let packed = pack_tokens(&tokens);
        assert_eq!(packed.len(), tokens.len() * 2);
        assert_eq!(unpack_tokens(&packed).expect("entpackbar"), tokens);
    }

    #[test]
    fn ungerade_token_payload_wird_abgelehnt() {
        assert!(unpack_tokens(&[1i16, 2, 3]).is_err());
        assert!(unpack_tokens(&[]).expect("leer ist gueltig").is_empty());
    }
}
