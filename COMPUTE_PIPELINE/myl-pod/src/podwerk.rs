//! **Das Podwerk**: ein Ortsdienst, der über den Pod der Kette rechnet statt
//! über ein Modell im eigenen Prozess (2026-10-26).
//!
//! # ⚑ Der Weg
//!
//! Die Tür eines Knotens reicht einen Auftrag über die Ortsleitung an diesen
//! Prozess; er zerlegt den Prompt in Token und fährt den Koordinator über
//! [`crate::shardweg::UeberDenKnoten`]: über die Relaisleitung des Knotens zu
//! den Knoten der Pod-Mitglieder und deren Shards. Welcher Pod, sagt das
//! Podbild der Kette; je Epoche entsteht der Koordinator neu.
//!
//! # ⚑ Das Bündel
//!
//! Ein eigener Faden sieht alle paar Sekunden nach: Liegen gegen Ende der
//! Epoche ([`EINREICHEN_AB_SECHSTEL`]) gerechnete Segmente vor und ist noch kein
//! Bündel eingereicht, lässt er es von den Positionen unterschreiben und
//! gibt es über die Leitung ab; der Knoten reicht es an die erste Position.
//! Die Kette nimmt ein Bündel je Pod und Epoche, also genau eines am Ende.
//!
//! Der Prozess hält keine Gewichte, nur den Wortschatz.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use integer_llm_runtime::tokenizer::Tokenizer;
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, PodId, SegmentId};
use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};
use myl_types::relais::{Podbild, Relaisantwort, Relaisfrage};

use crate::coordinator::Coordinator;
use crate::entsiegelung::Klartextwerk;
use crate::shardweg::{Ausweis, Shardanfrage, Shardantwort, Shardweg, UeberDenKnoten};

/// Ab welchem Teil einer Epoche das Bündel eingereicht wird, in Sechsteln:
/// im letzten Sechstel, damit die Arbeit der Epoche fast ganz darin steht und
/// noch Zeit für den Weg in einen Block bleibt. Die Länge der Epoche nennt
/// das Podbild.
pub const EINREICHEN_AB_SECHSTEL: u64 = 5;

/// **Wie lange auf einen neuen Pod gewartet wird**, bevor über ihn
/// gerechnet wird.
///
/// 📌 **Fund 649, zweiter Teil (2026-10-26, im langen Lauf über Docker):**
/// Gleich nach dem Wechsel liefen bei den neuen Mitgliedern teils noch die
/// Shards der alten Epoche; einer meldete „kein Shard“, ein anderer bekam
/// ein Token ohne Einbettung. Gerechnet wird erst, wenn jedes Mitglied den
/// Zuschnitt meldet, den die Kette ihm gibt. Weit unter der Frist der Tür.
pub const BEREIT_FRIST: Duration = Duration::from_secs(60);

/// Melden alle Mitglieder des Pods den Ebenenbereich aus der Kette?
fn pod_bereit(relais: SocketAddr, ausweis: &Ausweis, pod: &myl_types::relais::Podeintrag) -> bool {
    let weg = UeberDenKnoten::neu(relais, *ausweis, pod.mitglieder.iter().map(|m| m.netzadresse).collect());
    pod.mitglieder.iter().enumerate().all(|(j, m)| {
        matches!(
            weg.frage(j, &Shardanfrage::Zuschnitt),
            Ok(Shardantwort::Zuschnitt(z)) if z.layer_start == u64::from(m.von) && z.layer_end == u64::from(m.bis)
        )
    })
}

/// Der Koordinator einer Epoche und ob ihr Bündel schon eingereicht ist.
struct Podstand {
    epoche: u64,
    kennung: PodId,
    koordinator: Coordinator,
    eingereicht: bool,
}

/// Der Ausweis der Relaisleitung, aus seiner Datei, einmal gelesen.
struct Ausweisquelle {
    datei: PathBuf,
    gelesen: Mutex<Option<Ausweis>>,
}

impl Ausweisquelle {
    fn holen(&self) -> Result<Ausweis, String> {
        let mut g = self.gelesen.lock().map_err(|_| "Ausweis vergiftet")?;
        if let Some(a) = *g {
            return Ok(a);
        }
        let a = myl_types::relais::ausweis_lesen(&self.datei).map_err(|e| format!("Ausweis der Relaisleitung: {e}"))?;
        *g = Some(a);
        Ok(a)
    }
}

/// Rechnet über den Pod der Kette.
pub struct Podwerk {
    relais: SocketAddr,
    /// Der Ausweis der Relaisleitung: **gelesen, wenn er gebraucht wird**.
    /// Der Knoten schreibt ihn bei seinem Start, und der kommt nach diesem
    /// Prozess, denn er braucht dessen Ortsausweis.
    ausweis: Arc<Ausweisquelle>,
    wortschatz: Tokenizer,
    pipeline: Hash,
    deckel: u32,
    stand: Arc<Mutex<Option<Podstand>>>,
}

impl Podwerk {
    /// Lädt den Wortschatz und startet den Faden, der das Bündel einreicht.
    pub fn neu(artefakte: &Path, relais: SocketAddr, ausweisdatei: PathBuf, pipeline: Hash, deckel: u32) -> Result<Self, String> {
        let ausweis = Arc::new(Ausweisquelle { datei: ausweisdatei, gelesen: Mutex::new(None) });
        let wortschatz = Tokenizer::from_file(
            artefakte.join("tokenizer.json").to_str().ok_or("Artefaktpfad ist kein gültiges UTF-8")?,
        )
        .map_err(|e| format!("Wortschatz laden: {e}"))?;
        let stand: Arc<Mutex<Option<Podstand>>> = Arc::new(Mutex::new(None));
        let s = Arc::clone(&stand);
        let a = Arc::clone(&ausweis);
        std::thread::Builder::new()
            .name("buendel".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(3));
                // Ohne Ausweis (der Knoten startet noch) gibt es nichts zu tun.
                let Ok(ausweis) = a.holen() else { continue };
                if let Err(f) = einreichen_wenn_faellig(relais, &ausweis, &s) {
                    eprintln!("[myl-pod] Bündel: {f}");
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { relais, ausweis, wortschatz, pipeline, deckel, stand })
    }
}

fn podbild(relais: SocketAddr, ausweis: &Ausweis) -> Result<Podbild, String> {
    match myl_types::relais::fragen(relais, ausweis, &Relaisfrage::Podbild, Duration::from_secs(10))? {
        Relaisantwort::Podbild(b) => Ok(b),
        Relaisantwort::Fehler(f) => Err(f),
        andere => Err(format!("statt Podbild: {andere:?}")),
    }
}

/// Reicht das Bündel der laufenden Epoche ein, wenn es fällig ist.
fn einreichen_wenn_faellig(relais: SocketAddr, ausweis: &Ausweis, stand: &Mutex<Option<Podstand>>) -> Result<(), String> {
    let bild = podbild(relais, ausweis)?;
    let mut g = stand.lock().map_err(|_| "Stand vergiftet")?;
    let Some(s) = g.as_mut() else { return Ok(()) };
    let laenge = bild.bloecke_je_epoche.max(1);
    if s.eingereicht || s.epoche != bild.epoche || bild.hoehe % laenge < laenge * EINREICHEN_AB_SECHSTEL / 6 {
        return Ok(());
    }
    if s.koordinator.completed_segments().is_empty() {
        return Ok(());
    }
    let buendel = s.koordinator.build_signed_poi_bundle()?;
    let bytes = borsh::to_vec(&buendel).map_err(|e| e.to_string())?;
    // Eingereicht ist eingereicht, auch wenn es scheitert: Ein zweiter
    // Versuch derselben Epoche fände dieselben Gründe.
    s.eingereicht = true;
    match myl_types::relais::fragen(relais, ausweis, &Relaisfrage::BuendelEinreichen { buendel: bytes }, Duration::from_secs(30))? {
        Relaisantwort::Erledigt(z) => {
            eprintln!("[myl-pod] Bündel der Epoche {}: {z}", s.epoche);
            Ok(())
        }
        Relaisantwort::Fehler(f) => Err(format!("Epoche {}: {f}", s.epoche)),
        andere => Err(format!("unerwartet: {andere:?}")),
    }
}

impl Podwerk {
    /// Rechnet einen Auftrag einmal über den ersten Pod dieses Podbilds:
    /// Token, letztes Segment, Epoche. Der Koordinator entsteht je Epoche
    /// und Pod neu.
    fn einmal(
        &self,
        bild: &Podbild,
        ausweis: &Ausweis,
        sitzung: u64,
        eingabe: &[u32],
        deckel: u64,
    ) -> Result<(Vec<u32>, SegmentId, u64), String> {
        let pod = bild.pods.first().ok_or("kein Pod in dieser Epoche")?;
        let mut g = self.stand.lock().map_err(|_| "Stand vergiftet")?;
        if g.as_ref().map(|s| (s.epoche, s.kennung)) != Some((bild.epoche, pod.kennung)) {
            // Erst wenn der Pod steht (Fund 649, zweiter Teil).
            let bis = std::time::Instant::now() + BEREIT_FRIST;
            while !pod_bereit(self.relais, ausweis, pod) && std::time::Instant::now() < bis {
                std::thread::sleep(Duration::from_secs(2));
            }
            let mitglieder = pod.mitglieder.iter().map(|m| m.netzadresse).collect();
            let weg = Arc::new(UeberDenKnoten::neu(self.relais, *ausweis, mitglieder));
            let k = Coordinator::ueber_weg(pod.kennung, EpochId(bild.epoche), weg, crate::coordinator::DEFAULT_WINDOW_MS)?;
            *g = Some(Podstand { epoche: bild.epoche, kennung: pod.kennung, koordinator: k, eingereicht: false });
        }
        let s = g.as_mut().ok_or("kein Koordinator")?;
        let beginn = std::time::Instant::now();
        let token = s.koordinator.run_prompt(sitzung, eingabe, deckel);
        eprintln!(
            "[myl-pod] Epoche {}: {} Prompt-Token, {} neue in {} ms",
            s.epoche,
            eingabe.len(),
            token.len(),
            beginn.elapsed().as_millis()
        );
        let segment = s.koordinator.completed_segments().last().map(|c| c.id).unwrap_or(SegmentId::new([0u8; 32]));
        s.koordinator.sitzung_abschliessen(sitzung);
        Ok((token, segment, s.epoche))
    }
}

impl Klartextwerk for Podwerk {
    fn rechne(&self, auftrag: &Inferenzauftrag, prompt: &[u8]) -> Inferenzantwort {
        let sitzung = auftrag.sitzung;
        let abgelehnt = |grund: &str| {
            eprintln!("[myl-pod] abgelehnt: {grund}");
            Inferenzantwort::Abgelehnt { sitzung }
        };
        let Ok(text) = core::str::from_utf8(prompt) else {
            return abgelehnt("kein UTF-8");
        };
        let eingabe: Vec<u32> = self.wortschatz.encode(text).iter().map(|t| *t as u32).collect();
        if eingabe.is_empty() {
            return abgelehnt("leerer Prompt");
        }
        let ausweis = match self.ausweis.holen() {
            Ok(a) => a,
            Err(f) => return abgelehnt(&f),
        };
        let deckel = u64::from(auftrag.max_token.min(self.deckel));
        let bild = match podbild(self.relais, &ausweis) {
            Ok(b) => b,
            Err(f) => return abgelehnt(&f),
        };
        let (mut token, mut segment, epoche) = match self.einmal(&bild, &ausweis, sitzung, &eingabe, deckel) {
            Ok(t) => t,
            Err(f) => return abgelehnt(&f),
        };
        // 📌 **Fund 649: ein Auftrag über die Epochengrenze.** Am Wechsel
        //    beenden die Knoten ihre Shards, auch mitten in einem Auftrag,
        //    und auch die Wiederholung des Koordinators trifft dann nur noch
        //    die alten Plätze. Kam kein Token und steht die Kette inzwischen
        //    in einer neuen Epoche, rechnet der Pod dieser Epoche den Auftrag
        //    einmal neu.
        if token.is_empty() {
            if let Ok(neu) = podbild(self.relais, &ausweis) {
                if neu.epoche != epoche {
                    eprintln!("[myl-pod] Epoche {epoche} endete im Auftrag; der Pod der Epoche {} rechnet ihn neu", neu.epoche);
                    match self.einmal(&neu, &ausweis, sitzung, &eingabe, deckel) {
                        Ok((t, seg, _)) => {
                            token = t;
                            segment = seg;
                        }
                        Err(f) => return abgelehnt(&f),
                    }
                }
            }
        }
        if token.is_empty() {
            return abgelehnt("der Pod lieferte kein Token");
        }
        let ausgabe: Vec<usize> = token.iter().map(|t| *t as usize).collect();
        Inferenzantwort::Ergebnis { sitzung, text: self.wortschatz.decode(&ausgabe), token, segment, prompt_token: eingabe.len() as u64 }
    }

    fn pipeline(&self) -> Hash {
        self.pipeline
    }

    fn shards(&self) -> u32 {
        self.stand
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.koordinator.num_shards() as u32))
            .unwrap_or(4)
    }
}
