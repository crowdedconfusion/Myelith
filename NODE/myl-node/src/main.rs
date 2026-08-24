//! `myl-node` — Kommandozeile des Myelith-Knotens.
//!
//! ```text
//! myl-node --name alpha --port 4150
//! myl-node --name beta  --port 4151 --bootstrap /ip4/…/tcp/4150/p2p/12D3Koo…
//! myl-node --name relais --rolle relais --oeffentlich /ip4/203.0.113.5/tcp/4150
//! ```
//!
//! Der Knoten läuft, bis Strg-C kommt, oder für `--laufzeit` Sekunden.
//! Beim Beenden schreibt er eine letzte Zustandsaufnahme, damit das
//! Protokoll mit einem Bild vom Ende schließt und nicht mittendrin
//! abbricht.

use std::path::PathBuf;
use std::time::Duration;

use myl_node::konfig::{standard_horchadressen, KnotenKonfig, Rolle};
use myl_node::Knoten;

const HILFE: &str = "\
myl-node — ein Myelith-Netzknoten

  --name <text>          Name im Protokoll (Vorgabe: knoten)
  --port <zahl>          Port für TCP und QUIC (Vorgabe: 4150)
  --horche <multiaddr>   Horchadresse, mehrfach möglich (ersetzt --port)
  --bootstrap <addr>     Einstiegsknoten mit /p2p/-Anteil, mehrfach möglich
  --rolle <wort>         teilnehmer (Vorgabe) oder relais
  --oeffentlich <addr>   eigene erreichbare Adresse, für --rolle relais Pflicht
  --relais <addr>        Relais, über das dieser Knoten erreichbar sein will
  --schluessel <datei>   Schlüsseldatei (Vorgabe: knoten.key)
  --protokolle <verz>    Verzeichnis für Betriebsprotokolle (Vorgabe: logs)
  --aufnahme <sek>       Abstand der Zustandsaufnahmen (Vorgabe: 30)
  --laufzeit <sek>       nach so vielen Sekunden beenden (Vorgabe: bis Strg-C)
  --still                nicht auf den Bildschirm protokollieren
  --hilfe                diese Übersicht

Die Schlüsseldatei bestimmt die Identität. Bleibt sie erhalten, behält
der Knoten seine Peer-Id über Neustarts, und nur dann lassen sich die
Protokolle mehrerer Läufe zusammenführen.
";

struct Argumente {
    konfig: KnotenKonfig,
    laufzeit: Option<u64>,
    auf_bildschirm: bool,
}

fn lies_argumente() -> Result<Option<Argumente>, String> {
    let roh: Vec<String> = std::env::args().skip(1).collect();
    if roh.iter().any(|a| a == "--hilfe" || a == "-h" || a == "--help") {
        print!("{HILFE}");
        return Ok(None);
    }

    let mut konfig = KnotenKonfig::default();
    let mut port: u16 = 4150;
    let mut horche: Vec<String> = Vec::new();
    let mut laufzeit = None;
    let mut auf_bildschirm = true;

    let mut i = 0;
    while i < roh.len() {
        let wert = |i: usize| -> Result<String, String> {
            roh.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{} erwartet einen Wert", roh[i]))
        };
        match roh[i].as_str() {
            "--name" => { konfig.name = wert(i)?; i += 2; }
            "--port" => {
                port = wert(i)?.parse().map_err(|_| "--port erwartet eine Zahl".to_string())?;
                i += 2;
            }
            "--horche" => { horche.push(wert(i)?); i += 2; }
            "--bootstrap" => { konfig.bootstrap.push(wert(i)?); i += 2; }
            "--rolle" => {
                let t = wert(i)?;
                konfig.rolle = Rolle::aus_text(&t)
                    .ok_or_else(|| format!("unbekannte Rolle: {t} (teilnehmer oder relais)"))?;
                i += 2;
            }
            "--oeffentlich" => { konfig.nat.oeffentliche_adressen.push(wert(i)?); i += 2; }
            "--relais" => { konfig.nat.relais.push(wert(i)?); i += 2; }
            "--schluessel" => { konfig.schluesseldatei = PathBuf::from(wert(i)?); i += 2; }
            "--protokolle" => { konfig.protokollverzeichnis = PathBuf::from(wert(i)?); i += 2; }
            "--aufnahme" => {
                konfig.aufnahme_sekunden = wert(i)?
                    .parse()
                    .map_err(|_| "--aufnahme erwartet eine Zahl".to_string())?;
                i += 2;
            }
            "--laufzeit" => {
                laufzeit = Some(
                    wert(i)?.parse().map_err(|_| "--laufzeit erwartet eine Zahl".to_string())?,
                );
                i += 2;
            }
            "--still" => { auf_bildschirm = false; i += 1; }
            unbekannt => return Err(format!("unbekannte Angabe: {unbekannt} (--hilfe)")),
        }
    }

    konfig.horchadressen = if horche.is_empty() { standard_horchadressen(port) } else { horche };
    // Die Schlüsseldatei bekommt den Knotennamen, sonst teilen sich
    // zwei Knoten im selben Verzeichnis eine Identität und damit eine
    // Peer-Id. Das ist beim lokalen Mehrknotenlauf der Normalfall.
    if konfig.schluesseldatei.as_os_str() == "knoten.key" {
        konfig.schluesseldatei = PathBuf::from(format!("{}.key", konfig.name));
    }
    Ok(Some(Argumente { konfig, laufzeit, auf_bildschirm }))
}

#[tokio::main]
async fn main() {
    let args = match lies_argumente() {
        Ok(Some(a)) => a,
        Ok(None) => return,
        Err(e) => {
            eprintln!("myl-node: {e}");
            std::process::exit(2);
        }
    };

    let mut knoten = match Knoten::starten(args.konfig, args.auf_bildschirm).await {
        Ok(k) => k,
        Err(e) => {
            eprintln!("myl-node: Start fehlgeschlagen: {e}");
            std::process::exit(1);
        }
    };

    eprintln!("myl-node: Peer-Id {}", knoten.peer_id());
    eprintln!("myl-node: Protokoll {}", knoten.protokollpfad().display());

    // Die eigenen Adressen nennen, sobald sie feststehen. Sie sind das,
    // was die anderen Maschinen als --bootstrap brauchen.
    if knoten.warte_auf_adresse(Duration::from_secs(5)).await.is_some() {
        for a in knoten.adressen() {
            eprintln!("myl-node: erreichbar unter {a}");
        }
    } else {
        eprintln!("myl-node: noch keine Horchadresse gemeldet");
    }

    match args.laufzeit {
        Some(sek) => knoten.laufe_fuer(Duration::from_secs(sek)).await,
        None => knoten.laufen().await,
    }
    knoten.aufnahme().await;
    eprintln!(
        "myl-node: beendet, {} Protokollzeilen in {}",
        knoten.protokollzeilen(),
        knoten.protokollpfad().display()
    );
}
