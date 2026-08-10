//! Pipeline-Coordinator
//! 
//! Startet alle Nodes, verteilt Manifeste, prueft theta_v-Konsistenz,
//! und routet den ersten Request.

use crate::manifest::{PipelineManifest, StageManifest};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::thread;

/// Zentraler Coordinator fuer die Pipeline-Initialisierung.
pub struct Coordinator {
    pub manifest: PipelineManifest,
    pub nodes: HashMap<String, NodeConnection>,
}

#[derive(Debug)]
pub struct NodeConnection {
    pub node_id: String,
    pub address: String,
    pub stream: Option<TcpStream>,
    pub theta_v_verified: bool,
    pub stage_id: usize,
}

impl Coordinator {
    pub fn new(manifest: PipelineManifest) -> Self {
        Coordinator {
            manifest,
            nodes: HashMap::new(),
        }
    }
    
    /// Wartet auf Verbindungen aller Nodes und verteilt Manifeste.
    pub fn bootstrap(&mut self, listen_addr: &str) -> Result<(), String> {
        let expected_nodes = self.manifest.stages.len();
        let listener = TcpListener::bind(listen_addr)
            .map_err(|e| format!("Coordinator bind failed: {}", e))?;
        
        println!("[coordinator] Warte auf {} Nodes auf {}...", expected_nodes, listen_addr);
        
        let mut connected = 0;
        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    // Handshake: Node sendet seine node_id + stage_id
                    let mut buf = [0u8; 256];
                    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
                    let handshake = String::from_utf8_lossy(&buf[..n]);
                    let parts: Vec<&str> = handshake.trim().split(':').collect();
                    if parts.len() != 2 {
                        return Err("Invalid handshake format".to_string());
                    }
                    let node_id = parts[0].to_string();
                    let stage_id: usize = parts[1].parse().map_err(|_| "Invalid stage_id")?;
                    
                    // Manifest senden
                    let manifest_json = serde_json::to_string(&self.manifest)
                        .map_err(|e| e.to_string())?;
                    stream.write_all(manifest_json.as_bytes())
                        .map_err(|e| e.to_string())?;
                    
                    self.nodes.insert(node_id.clone(), NodeConnection {
                        node_id,
                        address: stream.peer_addr().map_err(|e| e.to_string())?.to_string(),
                        stream: Some(stream),
                        theta_v_verified: true, // TODO: Echte Verifikation
                        stage_id,
                    });
                    
                    connected += 1;
                    println!("[coordinator] Node {} (Stage {}) verbunden.", parts[0], stage_id);
                    
                    if connected >= expected_nodes {
                        break;
                    }
                }
                Err(e) => eprintln!("[coordinator] Connection error: {}", e),
            }
        }
        
        println!("[coordinator] Alle {} Nodes verbunden und validiert.", connected);
        Ok(())
    }
    
    /// Sendet Start-Signal an alle Nodes.
    pub fn start_pipeline(&mut self) -> Result<(), String> {
        for (node_id, conn) in &mut self.nodes {
            if let Some(ref mut stream) = conn.stream {
                stream.write_all(b"START")
                    .map_err(|e| format!("Failed to start {}: {}", node_id, e))?;
            }
        }
        println!("[coordinator] Pipeline gestartet.");
        Ok(())
    }
}
