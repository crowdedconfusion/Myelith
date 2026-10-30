//! **Ein Knoten, ein Shard** (Stufe 4 des Dispatchers, P7, 2026-10-22).
//!
//! # ⚑ Was hier geschieht
//!
//! Der Knoten liest Zuteilung und Grenzen der laufenden Epoche aus seiner
//! Kette, findet seine Position und startet `myl-shard --von --bis` mit dem
//! Artefakt seines Netzes. Am Epochenwechsel lässt er ihn laufen, wenn sein
//! Bereich gleich blieb, und startet ihn sonst neu; ohne Position beendet er
//! ihn. `--pod` von Hand bleibt für Probeläufe, ausdrücklich so benannt.
//!
//! ⚑ **Der Shard läuft in einem eigenen Prozess**, wie seit dem 2026-09-03
//! entschieden: Ein Absturz beim Rechnen hält den Konsens nicht an.
//!
//! ⚑ **Nicht blockierend**: Das Laden des 35B dauert Sekunden. Die Adresse,
//! die der Shard als erste Zeile nennt, liest ein eigener Faden; bis sie da
//! ist, gilt der Shard als startend.
//!
//! # ⚑ Die Netzlast für den lokalen Plan (P8)
//!
//! Neben dem Protokoll liegt `netzlast.json`: welches Modell, welcher
//! Bereich, welche Epoche. Der lokale Plan des Klienten liest sie und gibt
//! dem Shard Vorrang vor dem Agenten (Netz vor Mensch). Ohne Shard wird die
//! Datei entfernt.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use myl_ledger::state::LedgerState;
use myl_types::ids::MinerId;

/// Was das Netz diesem Knoten in einer Epoche zuteilt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shardauftrag {
    /// Die Epoche.
    pub epoche: u64,
    /// Die Platznummer des Pods.
    pub pod: u32,
    /// Die Position im Pod.
    pub position: u32,
    /// Wie viele Positionen der Pod hat.
    pub positionen: u32,
    /// Der Bereich `[von, bis)`.
    pub von: u32,
    pub bis: u32,
}

impl Shardauftrag {
    /// Hält er die Einbettung?
    pub fn mit_einbettung(&self) -> bool {
        self.position == 0
    }

    /// Trägt er den Kopf?
    pub fn mit_kopf(&self) -> bool {
        self.position + 1 == self.positionen
    }

    /// Derselbe Shard, gleich welche Epoche oder welcher Pod: gleicher
    /// Bereich, gleiche Rolle. Dann darf der Prozess weiterlaufen.
    pub fn gleicher_shard(&self, anderer: &Shardauftrag) -> bool {
        (self.von, self.bis, self.position, self.positionen) == (anderer.von, anderer.bis, anderer.position, anderer.positionen)
    }
}

/// **Der Auftrag dieses Knotens in der laufenden Epoche**, aus der Kette:
/// sein Pod, seine Position und deren Grenzen. `None`, wenn er keine
/// Position hat (keine Zusage, Reserve, kein machbarer Pod).
///
/// ⚑ **Reserve rechnet nicht**: Ein Reservemitglied hält sich bereit und
/// startet seinen Shard erst bei einer Übernahme.
pub fn auftrag_der_epoche(zustand: &LedgerState, ich: &MinerId) -> Option<Shardauftrag> {
    let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
    let pod = zuteilung.pods.iter().find(|p| p.shards.iter().any(|s| s.miner.miner_id == *ich))?;
    let position = pod.shards.iter().position(|s| s.miner.miner_id == *ich)?;
    let grenzen = crate::kette::Kette::grenzen_des_pods(zustand, pod)?;
    let (von, bis) = *grenzen.get(position)?;
    Some(Shardauftrag {
        epoche: zustand.epoch.0,
        pod: pod.pod_index,
        position: position as u32,
        positionen: pod.shards.len() as u32,
        von,
        bis,
    })
}

/// ⚑ **Darf `ich` die Position `position` des Pods `pod` übernehmen?**
/// (Whitepaper 6.8, 2026-10-28): nur in der laufenden Epoche, nur als
/// Mitglied der Reserve dieses Pods, nur eine Position, die es gibt, und
/// nur, wenn die eigene Zusage so viele Ebenen hält. Liefert den Auftrag.
pub fn uebernahme_pruefen(
    zustand: &LedgerState,
    ich: &MinerId,
    epoche: u64,
    pod: u32,
    position: u32,
) -> Result<Shardauftrag, String> {
    if epoche != zustand.epoch.0 {
        return Err(format!("Epoche {epoche}, laufend ist {}", zustand.epoch.0));
    }
    let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(zustand);
    let p = zuteilung.pods.iter().find(|p| p.pod_index == pod).ok_or("diesen Pod gibt es nicht")?;
    if !p.reserve.iter().any(|r| r.miner_id == *ich) {
        return Err("dieser Knoten ist nicht Reserve dieses Pods".into());
    }
    let grenzen = crate::kette::Kette::grenzen_des_pods(zustand, p).ok_or("Pod ohne Schnitt")?;
    let (von, bis) = *grenzen.get(position as usize).ok_or("diese Position gibt es nicht")?;
    let zusage = myl_ledger::transitions::modellzusage_der_epoche(zustand, ich, zustand.epoch).ok_or("keine Zusage")?;
    if zusage.ebenen_hoechstens < bis - von {
        return Err(format!("{} Ebenen zugesagt, die Position hat {}", zusage.ebenen_hoechstens, bis - von));
    }
    Ok(Shardauftrag { epoche, pod, position, positionen: p.shards.len() as u32, von, bis })
}

/// ⚑ **Ob eine Spurablage zu diesem Knoten gehört** (Stufe 2): Er ist
/// Mitglied des Pods (Position oder Reserve), die Ablage trägt Einträge, und
/// ihre Zeugnisse ergeben die Wurzel, die sie nennt. ⚠️ Wer abgelegt hat,
/// prüft das nicht: Ein Mitglied desselben Pods kann eine erfundene Ablage
/// neben die echte legen. Beim Antworten zählt aber nur die Ablage mit der
/// Wurzel des Bündels aus der Kette.
pub fn ablage_pruefen(
    pod: &myl_scheduler::shard_assignment::Pod,
    ich: &MinerId,
    ablage: &myl_types::Spurablage,
) -> Result<(), String> {
    if !pod.shards.iter().map(|s| &s.miner).chain(pod.reserve.iter()).any(|m| m.miner_id == *ich) {
        return Err("dieser Knoten ist nicht Mitglied dieses Pods".into());
    }
    if ablage.eintraege.is_empty() {
        return Err("Ablage ohne Einträge".into());
    }
    // Die Tokens stehen je Segment daneben, oder gar nicht (Stufe 2).
    if !ablage.eingaben.is_empty() && ablage.eingaben.len() != ablage.eintraege.len() {
        return Err(format!("{} Eingaben zu {} Einträgen", ablage.eingaben.len(), ablage.eintraege.len()));
    }
    let zeugnisse: Vec<_> = ablage.eintraege.iter().map(|(z, _)| *z).collect();
    if myl_types::segments_root(&zeugnisse).map_err(|e| format!("{e:?}"))? != ablage.segments_root {
        return Err("die Zeugnisse ergeben die genannte Wurzel nicht".into());
    }
    Ok(())
}

/// Womit der Knoten einen Shard startet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shardstart {
    /// Das Programm `myl-shard`.
    pub programm: PathBuf,
    /// Das Artefakt des Netzmodells.
    pub artefakte: PathBuf,
    /// Die Datei des Konsensschlüssels: Der Shard unterschreibt mit dem
    /// Schlüssel, unter dem der Miner angemeldet ist.
    pub konsensschluessel: PathBuf,
    /// Mit Trainingshälfte?
    pub training: bool,
    /// Wohin `netzlast.json` geht.
    pub netzlast: PathBuf,
}

/// Der Dateiname des Ausweises der Shardtür, neben `netzlast.json`.
pub const AUSWEIS_DATEI: &str = "shardausweis";

/// Was beim Abgleich geschah.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wechsel {
    /// Nichts zu tun.
    Unveraendert,
    /// Ein Shard wurde gestartet.
    Gestartet(Shardauftrag),
    /// Der laufende wurde beendet, und es gibt keinen Auftrag.
    Beendet,
}

/// **Führt den Shard-Prozess dieses Knotens.**
pub struct Shardfuehrung {
    start: Shardstart,
    laufend: Option<Laufend>,
}

struct Laufend {
    auftrag: Shardauftrag,
    kind: Child,
    adresse: std::sync::mpsc::Receiver<std::net::SocketAddr>,
    bekannt: Option<std::net::SocketAddr>,
    /// Der Ausweis seiner Tür (Fund 611), je Start neu gezogen.
    ausweis: [u8; myl_types::zeitprobe::AUSWEIS_LEN],
}

impl Shardfuehrung {
    pub fn neu(start: Shardstart) -> Self {
        Self { start, laufend: None }
    }

    /// Das Artefakt des Netzmodells, für den Checker (Stufe 2).
    pub fn artefakte(&self) -> &std::path::Path {
        &self.start.artefakte
    }

    /// Der laufende Auftrag, wenn einer läuft.
    pub fn auftrag(&self) -> Option<&Shardauftrag> {
        self.laufend.as_ref().map(|l| &l.auftrag)
    }

    /// Die Prozessnummer des laufenden Shards.
    pub fn pid(&self) -> Option<u32> {
        self.laufend.as_ref().map(|l| l.kind.id())
    }

    /// Die Tür des laufenden Shards, sobald er sie genannt hat: Adresse
    /// und Ausweis, ohne den sie nichts annimmt (Fund 611).
    pub fn tuer(&mut self) -> Option<(std::net::SocketAddr, [u8; myl_types::zeitprobe::AUSWEIS_LEN])> {
        let l = self.laufend.as_mut()?;
        if l.bekannt.is_none() {
            l.bekannt = l.adresse.try_recv().ok();
        }
        l.bekannt.map(|a| (a, l.ausweis))
    }

    /// **Gleicht den laufenden Shard mit dem Soll ab.** Bleibt der Bereich
    /// gleich, läuft er weiter (nur die Epoche wird vermerkt); sonst wird er
    /// beendet und, wenn es ein Soll gibt, neu gestartet.
    pub fn abgleichen(&mut self, soll: Option<Shardauftrag>) -> Result<Wechsel, String> {
        if let (Some(l), Some(s)) = (self.laufend.as_mut(), soll.as_ref()) {
            if l.auftrag.gleicher_shard(s) && l.kind.try_wait().ok().flatten().is_none() {
                l.auftrag = s.clone();
                netzlast_schreiben(&self.start.netzlast, s, &self.start.artefakte);
                return Ok(Wechsel::Unveraendert);
            }
        }
        let hatte = self.laufend.is_some();
        self.beenden();
        let Some(soll) = soll else {
            let _ = std::fs::remove_file(&self.start.netzlast);
            let _ = std::fs::remove_file(self.start.netzlast.with_file_name(AUSWEIS_DATEI));
            return Ok(if hatte { Wechsel::Beendet } else { Wechsel::Unveraendert });
        };
        // ⚑ **Der Ausweis der Tür, je Start neu** (Fund 611): 32 Bytes aus
        //   der Zufallsquelle, mit `0600` neben die Netzlast gelegt. Wer die
        //   Datei lesen kann, darf fragen; das ist der Kreis, den das
        //   Dateisystem ohnehin zieht.
        let mut ausweis = [0u8; myl_types::zeitprobe::AUSWEIS_LEN];
        getrandom::fill(&mut ausweis).map_err(|e| format!("keine Zufallsquelle fuer den Ausweis: {e}"))?;
        let ausweisdatei = self.start.netzlast.with_file_name(AUSWEIS_DATEI);
        myl_types::ortsleitung::schluessel_ablegen(&ausweisdatei, &ausweis)
            .map_err(|e| format!("{}: {e}", ausweisdatei.display()))?;
        let mut befehl = Command::new(&self.start.programm);
        befehl
            .arg("--ausweis")
            .arg(&ausweisdatei)
            .arg("--artefakte")
            .arg(&self.start.artefakte)
            .arg("--shard")
            .arg(soll.position.to_string())
            .arg("--shards")
            .arg(soll.positionen.to_string())
            .arg("--von")
            .arg(soll.von.to_string())
            .arg("--bis")
            .arg(soll.bis.to_string())
            .arg("--konsensschluessel")
            .arg(&self.start.konsensschluessel)
            .arg("--adresse")
            .arg("127.0.0.1:0");
        if self.start.training {
            befehl.arg("--training");
        }
        let mut kind = befehl
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("{}: {e}", self.start.programm.display()))?;
        let aus = kind.stdout.take().ok_or("keine Standardausgabe")?;
        let (sender, adresse) = std::sync::mpsc::channel();
        let nummer = soll.position;
        std::thread::spawn(move || {
            let mut zeile = String::new();
            if std::io::BufReader::new(aus).read_line(&mut zeile).unwrap_or(0) > 0 {
                if let Some(a) = zeile.trim().strip_prefix(&format!("SHARD {nummer} ")).and_then(|t| t.parse().ok()) {
                    let _ = sender.send(a);
                }
            }
        });
        netzlast_schreiben(&self.start.netzlast, &soll, &self.start.artefakte);
        self.laufend = Some(Laufend { auftrag: soll.clone(), kind, adresse, bekannt: None, ausweis });
        Ok(Wechsel::Gestartet(soll))
    }

    /// Beendet den laufenden Shard, falls einer läuft.
    pub fn beenden(&mut self) {
        if let Some(mut l) = self.laufend.take() {
            let _ = l.kind.kill();
            let _ = l.kind.wait();
        }
    }
}

impl Drop for Shardfuehrung {
    fn drop(&mut self) {
        self.beenden();
        let _ = std::fs::remove_file(&self.start.netzlast);
        let _ = std::fs::remove_file(self.start.netzlast.with_file_name(AUSWEIS_DATEI));
    }
}

/// Schreibt `netzlast.json` (Format in `myl_store::teilablage::Netzlast`);
/// ein Fehler geht nur verloren, denn der Shard läuft auch ohne sie (der
/// Plan sieht ihn dann nicht).
fn netzlast_schreiben(pfad: &Path, auftrag: &Shardauftrag, artefakte: &Path) {
    let last = myl_store::teilablage::Netzlast {
        artefakte: artefakte.to_path_buf(),
        epoche: auftrag.epoche,
        pod: auftrag.pod,
        position: auftrag.position,
        positionen: auftrag.positionen,
        von: auftrag.von,
        bis: auftrag.bis,
    };
    let _ = last.schreiben(pfad);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auftrag(epoche: u64, von: u32, bis: u32) -> Shardauftrag {
        Shardauftrag { epoche, pod: 0, position: 1, positionen: 4, von, bis }
    }

    /// ⚑ **Der Auftrag kommt aus der Kette**: Sechs Probekonten mit Zusage
    /// bilden einen Pod; jede Position bekommt ihren Bereich, die Bereiche
    /// decken alle Ebenen lückenlos, und die Reserve bekommt keinen.
    #[test]
    fn der_auftrag_folgt_aus_zuteilung_und_schnitt() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        let mut k = crate::kette::Kette::probestand();
        for w in 0..6u8 {
            k.aufnehmen(crate::kette::probeanmeldung(w));
            k.aufnehmen(crate::kette::probezusage(w, myl_types::ids::EpochId(1), 1));
        }
        k.baue_block();
        for _ in 0..(3 * BLOECKE_JE_EPOCHE) {
            k.baue_block();
        }
        let ich = |w: u8| MinerId::new(*crate::kette::probekonto(w).as_bytes());
        let auftraege: Vec<Shardauftrag> = (0..6).filter_map(|w| auftrag_der_epoche(k.zustand(), &ich(w))).collect();
        assert_eq!(auftraege.len(), 4, "vier Positionen, zwei Reserve: {auftraege:?}");
        let mut bereiche: Vec<(u32, u32)> = auftraege.iter().map(|a| (a.von, a.bis)).collect();
        bereiche.sort();
        assert_eq!(bereiche.first().map(|b| b.0), Some(0));
        assert_eq!(bereiche.last().map(|b| b.1), Some(28));
        assert!(bereiche.windows(2).all(|w| w[0].1 == w[1].0), "lückenlos: {bereiche:?}");
        assert_eq!(auftraege.iter().filter(|a| a.mit_kopf()).count(), 1);
    }

    /// ⚑ **Gleicher Bereich, gleicher Shard**, über die Epochengrenze; ein
    /// anderer Bereich ist ein anderer.
    /// ⚑ **Eine Position übernimmt nur die Reserve ihres Pods** (Whitepaper
    /// 6.8): in der laufenden Epoche, für eine Position, die es gibt; der
    /// Auftrag hat deren Bereich.
    #[test]
    fn nur_die_reserve_uebernimmt_eine_position() {
        use myl_consensus::block::BLOECKE_JE_EPOCHE;
        let mut k = crate::kette::Kette::probestand();
        for w in 0..6u8 {
            k.aufnehmen(crate::kette::probeanmeldung(w));
            k.aufnehmen(crate::kette::probezusage(w, myl_types::ids::EpochId(1), 1));
        }
        k.baue_block();
        for _ in 0..(3 * BLOECKE_JE_EPOCHE) {
            k.baue_block();
        }
        let z = k.zustand();
        let e = z.epoch.0;
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(z);
        let pod = &zuteilung.pods[0];
        let reserve = pod.reserve[0].miner_id;
        let inhaber = pod.shards[0].miner.miner_id;
        let soll = auftrag_der_epoche(z, &pod.shards[2].miner.miner_id).expect("Position 2");
        let a = uebernahme_pruefen(z, &reserve, e, pod.pod_index, 2).expect("die Reserve übernimmt");
        assert_eq!((a.position, a.von, a.bis, a.positionen), (2, soll.von, soll.bis, soll.positionen));
        // ⛔️ Gegenproben.
        assert!(uebernahme_pruefen(z, &inhaber, e, pod.pod_index, 2).is_err(), "ein Inhaber ist keine Reserve");
        assert!(uebernahme_pruefen(z, &reserve, e, pod.pod_index, 4).is_err(), "keine solche Position");
        assert!(uebernahme_pruefen(z, &reserve, e + 1, pod.pod_index, 2).is_err(), "eine andere Epoche");
        assert!(uebernahme_pruefen(z, &reserve, e, pod.pod_index + 7, 2).is_err(), "kein solcher Pod");
    }

    /// ⚑ Nur ein Mitglied nimmt eine Ablage an, und nur eine, deren
    /// Zeugnisse ihre Wurzel ergeben.
    #[test]
    fn nur_ein_mitglied_legt_eine_stimmige_spur_ab() {
        let mut k = crate::kette::Kette::probestand();
        for w in 0..8u8 {
            k.aufnehmen(crate::kette::probeanmeldung(w));
            k.aufnehmen(crate::kette::probezusage(w, myl_types::ids::EpochId(1), 1));
        }
        k.baue_block();
        for _ in 0..(3 * myl_consensus::block::BLOECKE_JE_EPOCHE) {
            k.baue_block();
        }
        let z = k.zustand();
        let zuteilung = crate::kette::Kette::zuteilung_der_laufenden_epoche(z);
        let pod = &zuteilung.pods[0];
        let zeugnisse: Vec<myl_types::Segmentzeugnis> = (0..3u8)
            .map(|i| myl_types::Segmentzeugnis { id: myl_types::ids::SegmentId::new([i; 32]), spurwurzel: myl_types::ids::MerkleRoot::new([i + 9; 32]) })
            .collect();
        let ablage = myl_types::Spurablage {
            epoche: z.epoch,
            pod: myl_types::pod_kennung(z.epoch.0, pod.pod_index),
            segments_root: myl_types::segments_root(&zeugnisse).expect("Wurzel"),
            eintraege: zeugnisse.iter().map(|zg| (*zg, vec![myl_types::Hash::sha256(zg.id.as_bytes())])).collect(),
            eingaben: Vec::new(),
        };
        for m in pod.shards.iter().map(|s| &s.miner).chain(pod.reserve.iter()) {
            assert_eq!(ablage_pruefen(pod, &m.miner_id, &ablage), Ok(()), "jedes Mitglied, auch die Reserve");
        }
        // ⛔️ Gegenproben.
        let schief = myl_types::Spurablage { eingaben: vec![vec![1, 0]], ..ablage.clone() };
        assert!(ablage_pruefen(pod, &pod.shards[0].miner.miner_id, &schief).is_err(), "Eingaben passen nicht zu den Einträgen");
        let fremd = MinerId::new([0xEE; 32]);
        assert!(ablage_pruefen(pod, &fremd, &ablage).is_err(), "kein Mitglied");
        let ich = pod.shards[0].miner.miner_id;
        let mut falsch = ablage.clone();
        falsch.segments_root = myl_types::ids::MerkleRoot::new([7; 32]);
        assert!(ablage_pruefen(pod, &ich, &falsch).is_err(), "eine Wurzel, die die Zeugnisse nicht ergeben");
        let mut kuerzer = ablage.clone();
        kuerzer.eintraege.pop();
        assert!(ablage_pruefen(pod, &ich, &kuerzer).is_err(), "ein Eintrag fehlt");
        let mut leer = ablage;
        leer.eintraege.clear();
        assert!(ablage_pruefen(pod, &ich, &leer).is_err(), "ohne Einträge");
    }

    #[test]
    fn gleicher_bereich_ist_derselbe_shard() {
        assert!(auftrag(3, 7, 14).gleicher_shard(&auftrag(4, 7, 14)));
        assert!(!auftrag(3, 7, 14).gleicher_shard(&auftrag(4, 7, 15)));
    }

    /// ⚑ **Der Prozess folgt dem Soll** (Unix, mit `sleep` als Programm):
    /// gestartet, bei gleichem Bereich weiterlaufen lassen, bei anderem neu
    /// gestartet, ohne Soll beendet; die Netzlast steht und verschwindet.
    #[cfg(unix)]
    #[test]
    fn der_prozess_folgt_dem_soll() {
        let ordner = std::env::temp_dir().join(format!("myl-shardfuehrung-{}", std::process::id()));
        std::fs::create_dir_all(&ordner).expect("Ordner");
        // Ein Programm, das die Optionen ignoriert und wartet.
        let programm = ordner.join("shard.sh");
        std::fs::write(&programm, "#!/bin/sh\nsleep 30\n").expect("Programm");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&programm, std::fs::Permissions::from_mode(0o700)).expect("Rechte");
        let netzlast = ordner.join("netzlast.json");
        let mut f = Shardfuehrung::neu(Shardstart {
            programm,
            artefakte: ordner.join("artefakt"),
            konsensschluessel: ordner.join("konsens.key"),
            training: false,
            netzlast: netzlast.clone(),
        });
        assert_eq!(f.abgleichen(Some(auftrag(3, 7, 14))), Ok(Wechsel::Gestartet(auftrag(3, 7, 14))));
        let gelesen = myl_store::teilablage::Netzlast::lesen(&netzlast).expect("Netzlast");
        assert_eq!((gelesen.epoche, gelesen.von, gelesen.bis), (3, 7, 14));
        // ⚑ **Der Ausweis der Tür liegt nur für den Besitzer da** (Fund 611)
        //   und gleicht dem, den der Knoten bei der Probe mitschickt.
        let ausweisdatei = ordner.join(AUSWEIS_DATEI);
        let modus = std::fs::metadata(&ausweisdatei).expect("Ausweis").permissions().mode() & 0o777;
        assert_eq!(modus, 0o600, "der Ausweis liegt mit {modus:o} offen");
        let erster = myl_types::ortsleitung::schluessel_lesen(&ausweisdatei).expect("lesen");
        assert_eq!(f.abgleichen(Some(auftrag(4, 7, 14))), Ok(Wechsel::Unveraendert));
        assert_eq!(f.auftrag().map(|a| a.epoche), Some(4), "die Epoche wird vermerkt");
        assert_eq!(f.abgleichen(Some(auftrag(5, 7, 15))), Ok(Wechsel::Gestartet(auftrag(5, 7, 15))));
        assert_ne!(myl_types::ortsleitung::schluessel_lesen(&ausweisdatei).expect("lesen"), erster, "je Start ein neuer Ausweis");
        assert_eq!(f.abgleichen(None), Ok(Wechsel::Beendet));
        assert!(!netzlast.exists(), "ohne Shard keine Netzlast");
        assert!(!ausweisdatei.exists(), "ohne Shard kein Ausweis");
        assert_eq!(f.abgleichen(None), Ok(Wechsel::Unveraendert));
        let _ = std::fs::remove_dir_all(&ordner);
    }
}
