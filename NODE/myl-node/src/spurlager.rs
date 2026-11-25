//! **Die Spurablagen auf der Platte** (Entscheidung des Projektinhabers vom
//! 2026-11-23, Folgerechnungen).
//!
//! # Warum
//!
//! Ein Mitglied hebt die Spuren jedes Bündels seines Pods auf, damit Prüfer
//! der Stufe 2 den Verlauf eines gezogenen Segments bekommen. Bis hierher
//! lagen bis zu 32 Ablagen im Arbeitsspeicher, und eine Ablage durfte
//! höchstens 64 MiB groß sein. Ein Pod auf CPU-Hardware rechnet in einer
//! Epoche des Testnetzes (eine Stunde) rund 87 000 Positionen, je rund
//! 1,4 KB: 121 MB. Seine Ablage nahm kein Mitglied an, und das Bündel blieb
//! ungeprüft; ein Angreifer konnte sein Bündel absichtlich so groß machen.
//!
//! Jetzt begrenzt die Kette die Segmente eines Bündels
//! ([`myl_types::MAX_SEGMENTE_JE_BUENDEL`]), die Grenze einer Ablage folgt
//! daraus und aus der Ebenenzahl des Netzmodells
//! ([`myl_types::spurablage_hoechstens_bytes`]), und die Ablagen liegen
//! hier: je Bündel eine Datei, im Speicher nur der Index und die zwei
//! zuletzt gelesenen. Eingehende Stücke gehen direkt in eine Datei
//! ([`Plattenstuecke`]), mit den Regeln von
//! [`crate::nachschub::Belegstuecke`].
//!
//! ⚑ **Aufgehoben wird nach Epochen** ([`AUFHEBEN_EPOCHEN`]): Gezogen wird
//! nach dem Wechsel, Festlegen und Aufdecken dauern je eine Viertelepoche,
//! die Schiedsrunde eine Epoche. Was älter ist, braucht niemand mehr.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use myl_types::ids::{EpochId, MerkleRoot, PodId};

/// So viele Epochen hinter der laufenden bleibt eine Ablage liegen.
pub const AUFHEBEN_EPOCHEN: u64 = 3;

/// Höchstens so viele Ablagen, über alle Pods und Epochen (Platte).
pub const ABLAGEN_HOECHSTENS: usize = 16;

/// So viele gelesene Ablagen hält der Speicher zugleich.
pub const IM_SPEICHER: usize = 2;

/// Endung einer fertigen Ablage.
const ENDUNG: &str = "ablage";
/// Endung eines Zwischenstands.
const ENDUNG_TEIL: &str = "eingang";

/// Ein Eintrag des Index.
#[derive(Debug, Clone)]
struct Eintrag {
    von: Option<libp2p::PeerId>,
    epoche: u64,
    pod: PodId,
    wurzel: MerkleRoot,
    segmente: usize,
    pfad: PathBuf,
}

/// **Die Ablagen eines Knotens**, auf der Platte.
#[derive(Debug)]
pub struct Spurlager {
    ordner: PathBuf,
    index: Vec<Eintrag>,
    gelesen: Vec<(PathBuf, Arc<myl_types::Spurablage>)>,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn aus_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut aus = [0u8; 32];
    for (i, a) in aus.iter_mut().enumerate() {
        *a = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(aus)
}

fn dateiname(epoche: u64, pod: &PodId, wurzel: &MerkleRoot) -> String {
    format!("{epoche}-{}-{}.{ENDUNG}", hex(pod.as_bytes()), hex(wurzel.as_bytes()))
}

impl Spurlager {
    /// Das Lager in `ordner`; legt ihn an, räumt Zwischenstände weg und liest
    /// den Index aus den Dateinamen (nach einem Neustart ist der Absender
    /// jeder Ablage unbekannt, `None`).
    pub fn oeffnen(ordner: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(ordner)?;
        let mut index = Vec::new();
        for e in std::fs::read_dir(ordner)? {
            let pfad = e?.path();
            let name = pfad.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
            if name.ends_with(&format!(".{ENDUNG_TEIL}")) || name.ends_with(".tmp") {
                let _ = std::fs::remove_file(&pfad);
                continue;
            }
            let Some(stamm) = name.strip_suffix(&format!(".{ENDUNG}")) else { continue };
            let teile: Vec<&str> = stamm.split('-').collect();
            let (Some(epoche), Some(pod), Some(wurzel)) = (
                teile.first().and_then(|e| e.parse::<u64>().ok()),
                teile.get(1).and_then(|p| aus_hex(p)),
                teile.get(2).and_then(|w| aus_hex(w)),
            ) else {
                continue;
            };
            index.push(Eintrag { von: None, epoche, pod: PodId::new(pod), wurzel: MerkleRoot::new(wurzel), segmente: 0, pfad });
        }
        index.sort_by_key(|e| e.epoche);
        Ok(Self { ordner: ordner.to_path_buf(), index, gelesen: Vec::new() })
    }

    /// Der Ordner des Lagers.
    pub fn ordner(&self) -> &Path {
        &self.ordner
    }

    /// Wie viele Ablagen liegen.
    pub fn anzahl(&self) -> usize {
        self.index.len()
    }

    /// Wie viele Segmente die seit dem Start abgelegten tragen.
    pub fn segmente(&self) -> usize {
        self.index.iter().map(|e| e.segmente).sum()
    }

    /// **Eine geprüfte Ablage aufnehmen**, als Datei: aus `bytes`, wenn sie
    /// schon serialisiert vorliegt (`quelle`, eine Datei dieses Lagers, wird
    /// dann umbenannt), sonst neu serialisiert. Je Bündel eine; danach wird
    /// aufgeräumt ([`Self::aufraeumen`]).
    pub fn aufnehmen(
        &mut self,
        von: Option<libp2p::PeerId>,
        ablage: myl_types::Spurablage,
        quelle: Option<&Path>,
        laufende_epoche: u64,
    ) -> std::io::Result<()> {
        let pfad = self.ordner.join(dateiname(ablage.epoche.0, &ablage.pod, &ablage.segments_root));
        match quelle {
            Some(q) => std::fs::rename(q, &pfad)?,
            None => {
                let bytes = borsh::to_vec(&ablage).map_err(std::io::Error::other)?;
                let zwischen = pfad.with_extension("tmp");
                {
                    let mut f = std::fs::File::create(&zwischen)?;
                    f.write_all(&bytes)?;
                    f.sync_all()?;
                }
                std::fs::rename(&zwischen, &pfad)?;
            }
        }
        self.index.retain(|e| e.pfad != pfad);
        self.gelesen.retain(|(p, _)| *p != pfad);
        self.index.push(Eintrag {
            von,
            epoche: ablage.epoche.0,
            pod: ablage.pod,
            wurzel: ablage.segments_root,
            segmente: ablage.eintraege.len(),
            pfad: pfad.clone(),
        });
        // Die eben abgelegte ist die wahrscheinlich nächste gefragte.
        self.gelesen.insert(0, (pfad, Arc::new(ablage)));
        self.gelesen.truncate(IM_SPEICHER);
        self.aufraeumen(laufende_epoche);
        Ok(())
    }

    /// Löscht, was älter ist als [`AUFHEBEN_EPOCHEN`] hinter `laufende`, und
    /// über [`ABLAGEN_HOECHSTENS`] die älteste Epoche zuerst.
    pub fn aufraeumen(&mut self, laufende: u64) {
        let grenze = laufende.saturating_sub(AUFHEBEN_EPOCHEN);
        let mut weg: Vec<PathBuf> = self.index.iter().filter(|e| e.epoche < grenze).map(|e| e.pfad.clone()).collect();
        self.index.retain(|e| e.epoche >= grenze);
        self.index.sort_by_key(|e| e.epoche);
        while self.index.len() > ABLAGEN_HOECHSTENS {
            weg.push(self.index.remove(0).pfad);
        }
        for p in weg {
            let _ = std::fs::remove_file(&p);
            self.gelesen.retain(|(q, _)| *q != p);
        }
    }

    /// **Sucht in den Ablagen**, die neueste zuerst, nur unter der Wurzel
    /// `wurzel`, wenn sie bekannt ist. Gelesen wird eine Datei erst, wenn
    /// sie gebraucht wird; die zwei zuletzt gelesenen hält der Speicher.
    ///
    /// Das Lesen geschieht im Takt des Knotens. Gemessen (2026-11-23,
    /// Freigabebau, M5 Pro): eine Ablage am Deckel (100 000 Segmente,
    /// 138 MB) in 34 ms, danach aus dem Speicher; nötig nur nach einem
    /// Neustart oder wenn mehr als [`IM_SPEICHER`] gefragt werden, denn eine
    /// eben abgelegte liegt schon im Speicher.
    pub fn finden<T>(&mut self, wurzel: Option<MerkleRoot>, mut f: impl FnMut(&myl_types::Spurablage) -> Option<T>) -> Option<T> {
        let kandidaten: Vec<PathBuf> = self
            .index
            .iter()
            .rev()
            .filter(|e| wurzel.is_none_or(|w| e.wurzel == w))
            .map(|e| e.pfad.clone())
            .collect();
        for pfad in kandidaten {
            let Some(ablage) = self.lesen(&pfad) else { continue };
            if let Some(t) = f(&ablage) {
                return Some(t);
            }
        }
        None
    }

    fn lesen(&mut self, pfad: &Path) -> Option<Arc<myl_types::Spurablage>> {
        if let Some(i) = self.gelesen.iter().position(|(p, _)| p == pfad) {
            let e = self.gelesen.remove(i);
            let a = Arc::clone(&e.1);
            self.gelesen.insert(0, e);
            return Some(a);
        }
        let bytes = std::fs::read(pfad).ok()?;
        let ablage = Arc::new(borsh::from_slice::<myl_types::Spurablage>(&bytes).ok()?);
        self.gelesen.insert(0, (pfad.to_path_buf(), Arc::clone(&ablage)));
        self.gelesen.truncate(IM_SPEICHER);
        Some(ablage)
    }

    /// Ein Zwischenstand für eingehende Stücke, in diesem Lager.
    pub fn eingang(&self, abdruck: &myl_types::Hash, gesamt: u32, hoechstens: u64, kennung: u64) -> Option<Plattenstuecke> {
        Plattenstuecke::neu(&self.ordner.join(format!("{}-{kennung}.{ENDUNG_TEIL}", hex(&abdruck.0))), *abdruck, gesamt, hoechstens)
    }

    /// Wer die Ablage zu `(epoche, pod)` geschickt hat, wenn sie aus dem
    /// Netz kam und seit dem Start abgelegt wurde.
    pub fn absender(&self, epoche: EpochId, pod: &PodId) -> Option<libp2p::PeerId> {
        self.index.iter().rev().find(|e| e.epoche == epoche.0 && e.pod == *pod).and_then(|e| e.von)
    }
}

/// Was ein Stück bewirkt.
#[derive(Debug, PartialEq, Eq)]
pub enum Plattenfolge {
    /// Angenommen; so viele Bytes sind da.
    Weiter(u32),
    /// Vollständig, und der Abdruck stimmt: die Datei.
    Fertig(PathBuf),
    /// Passt nicht; der Zwischenstand ist gelöscht.
    Verworfen,
}

/// **Eingehende Stücke einer Ablage, direkt in eine Datei**: lückenlos,
/// alle mit demselben Abdruck und derselben Grösse, keines über
/// [`crate::nachschub::SCHIEDSBELEG_STUECK_BYTES`], das Ganze höchstens
/// `hoechstens`; am Ende muss der SHA-256 stimmen. Ein verworfener oder
/// fallengelassener Zwischenstand löscht seine Datei.
#[derive(Debug)]
pub struct Plattenstuecke {
    pfad: PathBuf,
    datei: Option<std::fs::File>,
    abdruck: myl_types::Hash,
    gesamt: u32,
    stand: u32,
    hasher: sha2::Sha256,
}

impl Plattenstuecke {
    /// Beginnt, wenn `gesamt` in `(0, hoechstens]` liegt und die Datei
    /// angelegt werden kann.
    pub fn neu(pfad: &Path, abdruck: myl_types::Hash, gesamt: u32, hoechstens: u64) -> Option<Self> {
        if gesamt == 0 || u64::from(gesamt) > hoechstens {
            return None;
        }
        let datei = std::fs::File::create(pfad).ok()?;
        Some(Self {
            pfad: pfad.to_path_buf(),
            datei: Some(datei),
            abdruck,
            gesamt,
            stand: 0,
            hasher: <sha2::Sha256 as sha2::Digest>::new(),
        })
    }

    /// Wie viele Bytes schon da sind.
    pub fn stand(&self) -> usize {
        self.stand as usize
    }

    fn verwerfen(&mut self) -> Plattenfolge {
        self.datei = None;
        let _ = std::fs::remove_file(&self.pfad);
        Plattenfolge::Verworfen
    }

    /// Hängt ein Stück an.
    pub fn anhaengen(&mut self, abdruck: &myl_types::Hash, gesamt: u32, ab: u32, daten: &[u8]) -> Plattenfolge {
        let passt = *abdruck == self.abdruck
            && gesamt == self.gesamt
            && ab == self.stand
            && !daten.is_empty()
            && daten.len() <= crate::nachschub::SCHIEDSBELEG_STUECK_BYTES
            && u64::from(self.stand) + daten.len() as u64 <= u64::from(gesamt);
        if !passt {
            return self.verwerfen();
        }
        let Some(datei) = self.datei.as_mut() else {
            return self.verwerfen();
        };
        if datei.write_all(daten).is_err() {
            return self.verwerfen();
        }
        sha2::Digest::update(&mut self.hasher, daten);
        self.stand += daten.len() as u32;
        if self.stand < self.gesamt {
            return Plattenfolge::Weiter(self.stand);
        }
        let hasher = std::mem::replace(&mut self.hasher, <sha2::Sha256 as sha2::Digest>::new());
        let summe: [u8; 32] = sha2::Digest::finalize(hasher).into();
        if summe != self.abdruck.0 || datei.sync_all().is_err() {
            return self.verwerfen();
        }
        self.datei = None;
        Plattenfolge::Fertig(std::mem::take(&mut self.pfad))
    }
}

impl Drop for Plattenstuecke {
    fn drop(&mut self) {
        if !self.pfad.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.pfad);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordner(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-spurlager-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn ablage(epoche: u64, pod: u8, wurzel: u8, segmente: usize) -> myl_types::Spurablage {
        myl_types::Spurablage {
            epoche: EpochId(epoche),
            pod: PodId::new([pod; 32]),
            segments_root: MerkleRoot::new([wurzel; 32]),
            eintraege: (0..segmente)
                .map(|i| {
                    (
                        myl_types::Segmentzeugnis { id: myl_types::ids::SegmentId::new([i as u8; 32]), spurwurzel: MerkleRoot::new([1; 32]) },
                        vec![myl_types::Hash([2; 32]); 5],
                    )
                })
                .collect(),
            eingaben: (0..segmente).map(|_| vec![1i16, 2]).collect(),
            buendel: None,
        }
    }

    /// Ablegen, wiederfinden (auch nach einem Neustart aus den Dateinamen),
    /// nur unter der richtigen Wurzel; aufgeräumt nach Epochen.
    #[test]
    fn abgelegt_wird_gefunden_auch_nach_dem_neustart_und_alt_faellt_weg() {
        let d = ordner("finden");
        let mut l = Spurlager::oeffnen(&d).expect("Lager");
        l.aufnehmen(None, ablage(5, 1, 7, 3), None, 5).expect("ablegen");
        l.aufnehmen(None, ablage(6, 1, 8, 4), None, 6).expect("ablegen");
        assert_eq!(l.anzahl(), 2);
        assert_eq!(l.finden(Some(MerkleRoot::new([7; 32])), |a| Some(a.eintraege.len())), Some(3));
        assert_eq!(l.finden(Some(MerkleRoot::new([9; 32])), |a| Some(a.eintraege.len())), None);
        drop(l);
        let mut l = Spurlager::oeffnen(&d).expect("wieder");
        assert_eq!(l.anzahl(), 2);
        assert_eq!(l.finden(None, |a| Some(a.epoche.0)), Some(6), "die neueste zuerst");
        l.aufraeumen(5 + AUFHEBEN_EPOCHEN + 1);
        assert_eq!(l.anzahl(), 1, "Epoche 5 liegt mehr als drei hinter 9");
        assert_eq!(std::fs::read_dir(&d).expect("Ordner").count(), 1, "die Datei ist weg");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn mehr_als_die_grenze_wird_geraeumt_die_aelteste_zuerst() {
        let d = ordner("grenze");
        let mut l = Spurlager::oeffnen(&d).expect("Lager");
        for i in 0..(ABLAGEN_HOECHSTENS as u64 + 2) {
            l.aufnehmen(None, ablage(10, i as u8, i as u8, 1), None, 10).expect("ablegen");
        }
        assert_eq!(l.anzahl(), ABLAGEN_HOECHSTENS);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Stücke direkt auf die Platte: in Folge angenommen, am Ende mit Abdruck;
    /// eine Lücke, ein falscher Abdruck, eine Grösse über der Grenze verwirft
    /// und löscht die Datei. Gegenprobe zu `Belegstuecke`: dieselben Regeln.
    #[test]
    fn stuecke_gehen_auf_die_platte_mit_denselben_regeln() {
        let d = ordner("stuecke");
        std::fs::create_dir_all(&d).expect("Ordner");
        let bytes: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let abdruck = myl_types::Hash::sha256(&bytes);
        let gesamt = bytes.len() as u32;
        assert!(Plattenstuecke::neu(&d.join("a"), abdruck, gesamt, 9_999).is_none(), "über der Grenze");
        let mut s = Plattenstuecke::neu(&d.join("a"), abdruck, gesamt, 1 << 20).expect("neu");
        assert_eq!(s.anhaengen(&abdruck, gesamt, 0, &bytes[..4_000]), Plattenfolge::Weiter(4_000));
        let Plattenfolge::Fertig(pfad) = s.anhaengen(&abdruck, gesamt, 4_000, &bytes[4_000..]) else {
            panic!("nicht fertig");
        };
        assert_eq!(std::fs::read(&pfad).expect("Datei"), bytes);
        drop(s);
        assert!(pfad.exists(), "eine fertige Datei bleibt");
        let mut s = Plattenstuecke::neu(&d.join("b"), abdruck, gesamt, 1 << 20).expect("neu");
        assert_eq!(s.anhaengen(&abdruck, gesamt, 10, &bytes[..10]), Plattenfolge::Verworfen, "Lücke");
        assert!(!d.join("b").exists(), "verworfen heisst gelöscht");
        let falsch = myl_types::Hash::sha256(b"anders");
        let mut s = Plattenstuecke::neu(&d.join("c"), falsch, gesamt, 1 << 20).expect("neu");
        assert_eq!(s.anhaengen(&falsch, gesamt, 0, &bytes[..4_000]), Plattenfolge::Weiter(4_000));
        assert_eq!(s.anhaengen(&falsch, gesamt, 4_000, &bytes[4_000..]), Plattenfolge::Verworfen, "Abdruck");
        let s = Plattenstuecke::neu(&d.join("e"), abdruck, gesamt, 1 << 20).expect("neu");
        drop(s);
        assert!(!d.join("e").exists(), "ein fallengelassener Zwischenstand löscht seine Datei");
        let _ = std::fs::remove_dir_all(&d);
    }
}
