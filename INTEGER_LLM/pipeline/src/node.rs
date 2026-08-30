//! Pipeline-Node – Netzwerk-Transport & Event Loop
//!
//! Unterstuetzt TCP mit binaerem Custom-Format.
//! Jede Node kann gleichzeitig Server (fuer vorherige Stage) und Client
//! (fuer naechste Stage bzw. Feedback an Stage 0) sein.
//!
//! Retry-Logik (Phase 12.62): Fehlschläge beim Downstream-Senden werden
//! mit Backoff wiederholt; die Duplikaterkennung der Empfängerseite
//! ((request_id, stage_id, token_position)) macht Retransmits idempotent.

use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::codec::encode_message;
use crate::stage::StageRuntime;

/// Maximale Anzahl an Sende-Versuchen (Downstream/Feedback).
pub const SEND_ATTEMPTS: u32 = 4;
/// Basis-Backoff zwischen den Versuchen (ms), verdoppelt sich je Versuch.
pub const SEND_BACKOFF_MS: u64 = 100;

/// Liest eine vollständige Nachricht aus einem Stream.
///
/// Protokoll: Die Rahmenlänge ergibt sich aus dem Header
/// (HEADER_SIZE + payload_len, aufgerundet auf 8-Byte-Alignment). Es
/// wird gelesen, bis der Rahmen vollständig ist (mehrere read()-Aufrufe
/// sind erlaubt).
fn read_full_message(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; crate::codec::HEADER_SIZE];
    let mut filled = 0usize;
    while filled < crate::codec::HEADER_SIZE {
        let n = stream
            .read(&mut buf[filled..])
            .map_err(|e| format!("Header-Lesen fehlgeschlagen: {}", e))?;
        if n == 0 {
            return Err("Verbindung vor vollständigem Header geschlossen".to_string());
        }
        filled += n;
    }
    // payload_len steht an Offset 8 + 6*8 = 56.
    let payload_len = u64::from_le_bytes(buf[56..64].try_into().unwrap()) as usize;
    let raw_len = crate::codec::HEADER_SIZE + payload_len;
    let total = raw_len + ((8 - (raw_len % 8)) % 8);
    buf.resize(total, 0);
    while filled < total {
        let n = stream
            .read(&mut buf[filled..])
            .map_err(|e| format!("Payload-Lesen fehlgeschlagen: {}", e))?;
        if n == 0 {
            return Err("Verbindung vor vollständigem Payload geschlossen".to_string());
        }
        filled += n;
    }
    Ok(buf)
}

pub struct Node {
    pub node_id: String,
    pub bind_address: String,
    // ⚑ Hier stand bis zum 2026-08-29 ein `upstream: Option<String>`.
    // Es wurde gesetzt, von der CLI durchgereicht, in der
    // Benutzungszeile beworben und **an keiner Stelle gelesen**;
    // `downstream` daneben wird an drei gelesen. Die Pipeline fliesst
    // vorwaerts, der Rueckweg der autoregressiven Schleife laeuft ueber
    // `feedback_address` von der letzten Stufe zu Stufe 0. Fuer eine
    // Adresse stromaufwaerts gibt es also keinen Verwender und keinen
    // Bedarf. Ein Knopf, der sich drehen laesst und nichts tut, ist
    // schlimmer als keiner: Wer ihn setzt, glaubt, etwas eingestellt zu
    // haben.
    pub downstream: Option<String>,
    /// Feedback-Adresse der finalen Stage → Stage 0 (autoregressive
    /// Schleife). Nur für die finale Stage gesetzt.
    pub feedback_address: Option<String>,
    pub runtime: Option<Arc<StageRuntime>>,
}

impl Node {
    pub fn new(node_id: &str, bind_address: &str) -> Self {
        Node {
            node_id: node_id.to_string(),
            bind_address: bind_address.to_string(),
            downstream: None,
            feedback_address: None,
            runtime: None,
        }
    }

    pub fn attach_runtime(&mut self, runtime: Arc<StageRuntime>) {
        self.runtime = Some(runtime);
    }

    /// Haupt-Event-Loop: Empfaengt, verarbeitet, leitet weiter.
    pub fn run_event_loop(&self) -> Result<(), String> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or("No runtime attached")?
            .clone();

        let listener = TcpListener::bind(&self.bind_address)
            .map_err(|e| format!("Bind failed on {}: {}", self.bind_address, e))?;

        println!(
            "[node:{}] Event-Loop gestartet auf {}",
            self.node_id, self.bind_address
        );

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let buf = match read_full_message(&mut stream) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("[node:{}] Lesen fehlgeschlagen: {}", self.node_id, e);
                            continue;
                        }
                    };
                    match runtime.process_message(&buf) {
                        Ok(output) => {
                            if let Some((meta, tensor)) = output.forward {
                                let blob = encode_message(&meta, &tensor);
                                if let Some(addr) = &self.downstream {
                                    if let Err(e) = self.send_with_retry(addr, &blob) {
                                        eprintln!("[node:{}] Forward error: {}", self.node_id, e);
                                    }
                                }
                            }
                            if let Some((meta, tensor)) = output.feedback {
                                let blob = encode_message(&meta, &tensor);
                                if let Some(addr) = &self.feedback_address {
                                    if let Err(e) = self.send_with_retry(addr, &blob) {
                                        eprintln!("[node:{}] Feedback error: {}", self.node_id, e);
                                    }
                                } else {
                                    eprintln!(
                                        "[node:{}] Feedback erzeugt, aber keine Feedback-Adresse konfiguriert",
                                        self.node_id
                                    );
                                }
                            }
                            for (request_id, position, token) in output.tokens {
                                println!(
                                    "[token] request={} position={} token={}",
                                    request_id, position, token
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!("[node:{}] Processing error: {}", self.node_id, e);
                            if e.contains("theta_v hash mismatch") {
                                eprintln!(
                                    "[node:{}] KRITISCHER FEHLER: theta_v mismatch. Node stoppt.",
                                    self.node_id
                                );
                                break;
                            }
                        }
                    }
                }
                Err(e) => eprintln!("[node:{}] Connection error: {}", self.node_id, e),
            }
        }

        Ok(())
    }

    /// Sendet einen Rahmen mit Retry und Backoff (Retransmits sind durch
    /// die Duplikaterkennung der Empfänger idempotent).
    pub fn send_with_retry(&self, addr: &str, blob: &[u8]) -> Result<(), String> {
        let mut backoff = SEND_BACKOFF_MS;
        for attempt in 1..=SEND_ATTEMPTS {
            match TcpStream::connect(addr) {
                Ok(mut stream) => match stream.write_all(blob) {
                    Ok(_) => return Ok(()),
                    Err(e) => {
                        eprintln!(
                            "[node:{}] Senden fehlgeschlagen (Versuch {}/{}): {}",
                            self.node_id,
                            attempt,
                            SEND_ATTEMPTS,
                            e
                        );
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[node:{}] Verbinden zu {} fehlgeschlagen (Versuch {}/{}): {}",
                        self.node_id,
                        addr,
                        attempt,
                        SEND_ATTEMPTS,
                        e
                    );
                }
            }
            if attempt < SEND_ATTEMPTS {
                thread::sleep(Duration::from_millis(backoff));
                backoff *= 2;
            }
        }
        Err(format!("Senden an {} nach {} Versuchen aufgegeben", addr, SEND_ATTEMPTS))
    }

    /// Sendet einen Rahmen an die naechste Stage (Kompatibilitäts-API).
    pub fn send_downstream(&self, blob: &[u8]) -> Result<(), String> {
        let addr = self
            .downstream
            .as_ref()
            .ok_or("No downstream configured – this is the final stage")?;
        self.send_with_retry(addr, blob)
    }

    /// Sendet einen Request an die erste Stage (Client-API).
    pub fn send_request(&self, addr: &str, blob: &[u8]) -> Result<Vec<u8>, String> {
        let mut stream = TcpStream::connect(addr)
            .map_err(|e| format!("Connect to first stage failed: {}", e))?;
        stream
            .write_all(blob)
            .map_err(|e| format!("Send request failed: {}", e))?;

        let mut buf = vec![0u8; 65536];
        let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
        buf.truncate(n);
        Ok(buf)
    }
}
