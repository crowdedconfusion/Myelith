//! Pipeline-Node – Netzwerk-Transport & Event Loop
//! 
//! Unterstuetzt TCP mit binaerem Custom-Format.
//! Jede Node kann gleichzeitig Server (fuer vorherige Stage) und Client (fuer naechste Stage) sein.

use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;
use std::sync::{Arc, Mutex};
use crate::stage::StageRuntime;
use crate::codec::{encode_message, decode_message};

pub struct Node {
    pub node_id: String,
    pub bind_address: String,
    pub upstream: Option<String>,
    pub downstream: Option<String>,
    pub runtime: Option<Arc<StageRuntime>>,
}

impl Node {
    pub fn new(node_id: &str, bind_address: &str) -> Self {
        Node {
            node_id: node_id.to_string(),
            bind_address: bind_address.to_string(),
            upstream: None,
            downstream: None,
            runtime: None,
        }
    }
    
    pub fn attach_runtime(&mut self, runtime: Arc<StageRuntime>) {
        self.runtime = Some(runtime);
    }
    
    /// Haupt-Event-Loop: Empfaengt, verarbeitet, sendet weiter.
    pub fn run_event_loop(&self) -> Result<(), String> {
        let runtime = self.runtime.as_ref()
            .ok_or("No runtime attached")?.clone();
        
        let listener = TcpListener::bind(&self.bind_address)
            .map_err(|e| format!("Bind failed on {}: {}", self.bind_address, e))?;
        
        println!("[node:{}] Event-Loop gestartet auf {}", self.node_id, self.bind_address);
        
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut buf = vec![0u8; 65536];
                    match stream.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            match runtime.process_message(&buf[..n]) {
                                Ok(Some((next_meta, tensor))) => {
                                    // Weiterleiten an naechste Stage
                                    if let Err(e) = self.send_downstream(&encode_message(&next_meta, &tensor)) {
                                        eprintln!("[node:{}] Forward error: {}", self.node_id, e);
                                    }
                                }
                                Ok(None) => {
                                    // Duplikat oder Abort – kein Forward
                                }
                                Err(e) => {
                                    eprintln!("[node:{}] Processing error: {}", self.node_id, e);
                                    // Bei theta_v-Mismatch oder schwerem Fehler: Abort
                                    if e.contains("theta_v hash mismatch") {
                                        eprintln!("[node:{}] KRITISCHER FEHLER: theta_v mismatch. Node stoppt.", self.node_id);
                                        break;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => eprintln!("[node:{}] Connection error: {}", self.node_id, e),
            }
        }
        
        Ok(())
    }
    
    /// Sendet ein Tensor an die naechste Stage.
    pub fn send_downstream(&self, blob: &[u8]) -> Result<(), String> {
        let addr = self.downstream.as_ref()
            .ok_or("No downstream configured – this is the final stage")?;
        
        let mut stream = TcpStream::connect(addr)
            .map_err(|e| format!("Connect to downstream {} failed: {}", addr, e))?;
        stream.write_all(blob)
            .map_err(|e| format!("Send to downstream failed: {}", e))?;
        
        Ok(())
    }
    
    /// Sendet ein Request an die erste Stage (Client-API).
    pub fn send_request(&self, addr: &str, blob: &[u8]) -> Result<Vec<u8>, String> {
        let mut stream = TcpStream::connect(addr)
            .map_err(|e| format!("Connect to first stage failed: {}", e))?;
        stream.write_all(blob)
            .map_err(|e| format!("Send request failed: {}", e))?;
        
        // Antwort lesen (fuer finale Stage)
        let mut buf = vec![0u8; 65536];
        let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
        buf.truncate(n);
        Ok(buf)
    }
}
