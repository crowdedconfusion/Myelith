//! **Die Teile eines Splitters auf der Platte** (Archivknoten, Stufe A7,
//! 2026-10-23).
//!
//! Ein Ordner neben der Kettendatei, je Teil eine Datei
//! `<grenze>-<stelle>.teil` mit dem Teil und seinem Beweis. Geschrieben wird
//! über eine Zwischendatei und Umbenennen, damit ein Absturz nie einen
//! halben Teil hinterlässt.
//!
//! ⚑ **Teile früherer Abschnitte bleiben liegen.** Belegt wird nur der
//! laufende, aufgehoben werden alle: Je Abschnitt ist es ein
//! Zweiunddreissigstel einer Periode, und bei einem Ausfall aller Kopien
//! sind es genau diese Teile, aus denen die Geschichte zurückkommt; ihre
//! Teilwurzeln stehen im Zustand.

use std::path::{Path, PathBuf};

use myl_types::archivteile::Teilbeweis;

/// Die Ablage der Teile.
#[derive(Debug, Clone)]
pub struct Teilablage {
    ordner: PathBuf,
}

impl Teilablage {
    /// Die Ablage neben der Kettendatei `kette`: `<kette>.teile/`.
    pub fn neben(kette: &Path) -> Self {
        let mut n = kette.as_os_str().to_owned();
        n.push(".teile");
        Self { ordner: PathBuf::from(n) }
    }

    /// Der Ordner.
    pub fn ordner(&self) -> &Path {
        &self.ordner
    }

    fn pfad(&self, grenze: u64, stelle: u64) -> PathBuf {
        self.ordner.join(format!("{grenze}-{stelle}.teil"))
    }

    /// Ob der Teil an `stelle` des Abschnitts der Grenze `grenze` liegt.
    pub fn hat(&self, grenze: u64, stelle: u64) -> bool {
        self.pfad(grenze, stelle).exists()
    }

    /// Legt einen Teil ab.
    pub fn ablegen(&self, grenze: u64, stelle: u64, stueck: &[u8], beweis: &Teilbeweis) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.ordner)?;
        let bytes = borsh::to_vec(&(stueck.to_vec(), beweis.clone()))?;
        let ziel = self.pfad(grenze, stelle);
        let zwischen = ziel.with_extension("neu");
        std::fs::write(&zwischen, bytes)?;
        std::fs::rename(&zwischen, &ziel)
    }

    /// Liest einen Teil.
    pub fn lesen(&self, grenze: u64, stelle: u64) -> Option<(Vec<u8>, Teilbeweis)> {
        let bytes = std::fs::read(self.pfad(grenze, stelle)).ok()?;
        borsh::from_slice(&bytes).ok()
    }

    /// Alle abgelegten Teile, als (Grenze, Stelle).
    pub fn alle(&self) -> Vec<(u64, u64)> {
        let Ok(eintraege) = std::fs::read_dir(&self.ordner) else {
            return Vec::new();
        };
        let mut aus: Vec<(u64, u64)> = eintraege
            .filter_map(|e| {
                let name = e.ok()?.file_name().into_string().ok()?;
                let (g, s) = name.strip_suffix(".teil")?.split_once('-')?;
                Some((g.parse().ok()?, s.parse().ok()?))
            })
            .collect();
        aus.sort_unstable();
        aus
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::archivteile::{gruppenwurzel, kodieren};

    #[test]
    fn ein_teil_kommt_unveraendert_zurueck() {
        let d = std::env::temp_dir().join(format!("myl-teilablage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let a = Teilablage::neben(&d.join("kette.dat"));
        let teile = kodieren(&[b"eins".to_vec(), b"zwei".to_vec()]);
        let w = vec![gruppenwurzel(&teile).expect("Wurzel")];
        let bw = Teilbeweis::bauen(&teile, 5, &w, 0).expect("Beweis");
        assert!(!a.hat(7, 0));
        a.ablegen(7, 0, &teile[5], &bw).expect("ablegen");
        assert!(a.hat(7, 0));
        assert_eq!(a.lesen(7, 0), Some((teile[5].clone(), bw)));
        assert_eq!(a.alle(), vec![(7, 0)]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
