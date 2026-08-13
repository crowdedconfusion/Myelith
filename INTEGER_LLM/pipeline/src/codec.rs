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
    buf.extend(std::iter::repeat(0u8).take(pad));
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
    
    let payload_end = off + payload_len as usize;
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
    
    let tensor: Vec<i16> = payload.chunks_exact(2)
        .map(|c| i16::from_le_bytes(c.try_into().unwrap()))
        .collect();
    
    let meta = MessageMeta {
        version, theta_v_hash, request_id, sequence_id, stage_id,
        token_position, payload_len, flags, reserved, crc,
    };
    
    Ok((meta, tensor))
}
