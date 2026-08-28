//! Wire-Protokoll zwischen den Shards eines Pods.
//!
//! Jede Nachricht trägt die Aktivierungen (oder gepackte Token-IDs für
//! Shard 0) zusammen mit der Segment-Spur und der Signatur des sendenden
//! Shards. Die Spur ist die akkumulierte Folge der Ausgabe-Hashes
//! `h(a_0), h(a_1), …`; der empfangende Shard prüft den Hash der
//! empfangenen Aktivierungen gegen den letzten Spur-Eintrag
//! (Manipulationserkennung, Anhang A.3 Schritt 2).
//!
//! Kodierung: Borsh (kanonisch, Konsens-Vertrag wie in `myl-types`).

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::BlsSignature;
use myl_types::ids::SegmentId;

/// Magic-Bytes zur Rahmen-Erkennung.
pub const MAGIC: [u8; 8] = *b"MYLPOD01";

/// Bit 0: Payload sind gepackte Token-IDs (Eingang für Shard 0), keine
/// Aktivierungen. Wird von Shard 0 nach dem Embedding entfernt.
pub const FLAG_TOKEN_INPUT: u64 = 0x1;
/// Bit 1: Der End-Shard sampelt an dieser Position ein Token (letzte
/// Prompt-Position und Feedback-Positionen). Bleibt bis zum End-Shard
/// erhalten.
pub const FLAG_SAMPLE: u64 = 0x2;
/// Bit 2: Feedback-Nachricht des End-Shards an Shard 0 (trägt zusätzlich
/// FLAG_TOKEN_INPUT | FLAG_SAMPLE).
pub const FLAG_FEEDBACK: u64 = 0x4;
/// Bit 3: Request abbrechen.
pub const FLAG_ABORT: u64 = 0x8;

/// Nachricht zwischen zwei Shards eines Pods.
///
/// `trace` ist die Spur **bis einschließlich der Ausgabe des sendenden
/// Shards**: Der Sender hängt seinen Ausgabe-Hash an, bevor er sendet.
/// Der Empfänger prüft `hash(payload) == trace.last()`.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct PodMessage {
    pub magic: [u8; 8],
    pub segment_id: SegmentId,
    /// Session-Id für die KV-Cache-Session-Affinität (Kap. 4.2): Der
    /// KV-Cache bleibt auf den Shards des zugewiesenen Pods und wird je
    /// Session geführt.
    pub session_id: u64,
    /// Index des sendenden Shards in der Pod-Pipeline.
    pub sender_shard: u64,
    /// Token-Position (Basis-Position; für Token-Eingänge die Position
    /// des Tokens).
    pub position: u64,
    pub flags: u64,
    /// Akkumulierte Spur der Ausgabe-Hashes (leer für die allererste
    /// Nachricht an Shard 0, da diese Token trägt).
    pub trace: Vec<[u8; 32]>,
    /// Signatur des sendenden Shards über den Übergang
    /// (`trace::transition_message`); für die initiale Token-Nachricht
    /// eine Null-Signatur.
    pub signature: BlsSignature,
    /// Aktivierungen (int16, Boundary-Skala) oder gepackte Token-IDs
    /// (je zwei i16 pro Token, little-endian) bei `FLAG_TOKEN_INPUT` /
    /// `FLAG_FEEDBACK`.
    pub payload: Vec<i16>,
}

impl PodMessage {
    /// Neue Token-Eingangs-Nachricht für Shard 0.
    pub fn token_input(
        segment_id: SegmentId,
        session_id: u64,
        position: u64,
        packed_tokens: Vec<i16>,
        flags: u64,
    ) -> Self {
        Self {
            magic: MAGIC,
            segment_id,
            session_id,
            sender_shard: 0,
            position,
            flags: flags | FLAG_TOKEN_INPUT,
            trace: Vec::new(),
            signature: BlsSignature([0u8; 96]),
            payload: packed_tokens,
        }
    }

    /// Prüft die Struktur-Rahmenbedingungen (Magic, Spur-Payload-
    /// Konsistenz). Die eigentliche Hash-Prüfung gegen die Spur macht
    /// `shard.rs` (dort ist der Hash der Payload zu berechnen).
    pub fn is_valid_frame(&self) -> bool {
        self.magic == MAGIC
    }

    /// True, wenn der Payload gepackte Token-IDs trägt (Eingang für
    /// Shard 0). FLAG_SAMPLE allein bedeutet kein Token, sondern nur,
    /// dass der End-Shard sampelt.
    pub fn carries_tokens(&self) -> bool {
        (self.flags & FLAG_TOKEN_INPUT) != 0
    }
}

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
    // clippy schlaegt seit 1.98 `as_chunks::<2>()` vor, stabil erst seit
    // Rust 1.88. Dieses Crate erklaert MSRV 1.85 (Cargo.toml).
    // `unknown_lints` muss mit erlaubt sein: Den Lint-Namen gibt es erst
    // ab clippy 1.98, ein `allow` darauf ist auf aelteren Werkzeugketten
    // selbst eine Warnung. So baut es mit beiden.
    #[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
    for pair in payload.chunks_exact(2) {
        let lo = pair[0] as u16 as u32;
        let hi = pair[1] as u16 as u32;
        out.push(lo | (hi << 16));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{from_slice, to_vec};

    #[test]
    fn token_packing_rundtrip() {
        let tokens = [0u32, 1, 42, 65535, 65536, 151935, u32::MAX];
        let packed = pack_tokens(&tokens);
        assert_eq!(packed.len(), tokens.len() * 2);
        let back = unpack_tokens(&packed).expect("Entpacken");
        assert_eq!(back, tokens);
    }

    #[test]
    fn unpack_ungleiche_laenge_wird_abgelehnt() {
        assert!(unpack_tokens(&[1i16, 2, 3]).is_err());
    }

    #[test]
    fn pod_message_borsh_rundtrip() {
        let msg = PodMessage {
            magic: MAGIC,
            segment_id: SegmentId::new([7u8; 32]),
            session_id: 11,
            sender_shard: 2,
            position: 5,
            flags: 0,
            trace: vec![[1u8; 32], [2u8; 32]],
            signature: BlsSignature([9u8; 96]),
            payload: vec![10, -20, 30],
        };
        let bytes = to_vec(&msg).expect("Serialisierung");
        let back: PodMessage = from_slice(&bytes).expect("Deserialisierung");
        assert_eq!(back, msg);
        assert!(back.is_valid_frame());
    }

    #[test]
    fn token_input_nachricht_hat_flag() {
        let msg =
            PodMessage::token_input(SegmentId::new([1u8; 32]), 5, 0, pack_tokens(&[42]), 0);
        assert!(msg.carries_tokens());
        assert!(msg.is_valid_frame());
        assert_eq!(msg.session_id, 5);
        assert_eq!(unpack_tokens(&msg.payload).expect("unpack"), vec![42]);
    }
}
