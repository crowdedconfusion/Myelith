//! **Der Dateidienst zwischen Knoten** (Stufe 4 des Dispatchers, P11,
//! Teilablage nach Weg b, 2026-10-22).
//!
//! # ⚑ Wozu
//!
//! Ein Knoten muss nicht das ganze Artefakt halten (Festlegung E6). Weist
//! ihm die Kette für die nächste Epoche einen Bereich zu, dessen Dateien ihm
//! fehlen, holt er sie bis zur Epochengrenze von anderen Knoten. Wer nicht
//! tauschen will, nennt in seiner Zusage einen gehaltenen Bereich (Weg a);
//! dann teilt ihm die Kette nichts außerhalb davon zu.
//!
//! # ⚑ Jede Datei gegen das Manifest
//!
//! Geholt wird nur, was das Gewichtsmanifest nennt, und erst nach der
//! Prüfung seiner SHA-256 bekommt die Datei ihren Namen. Bis dahin liegt
//! sie als `<name>.teil` daneben; eine falsche Datei wird gelöscht, und die
//! Quelle, die sie lieferte, wird übergangen. **Der Dienst ist ein
//! Transportweg, kein Vertrauensweg**, wie die Blocknachforderung.
//!
//! # ⚑ Was ein Knoten ausliefert
//!
//! Nur Dateien, die sein eigenes Manifest nennt, nur mit flachem Namen, und
//! höchstens [`STUECK_BYTES`] je Antwort. Ein Name mit Trenner oder einer,
//! der nicht im Manifest steht, bekommt `Nichts`.

use std::collections::VecDeque;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use myl_store::teilablage::{Manifestdatei, Teilablagefehler};

/// Höchstens so viele Bytes je Antwort, unter der Grenze des Anfragekanals.
pub const STUECK_BYTES: u64 = 3 * 1024 * 1024;

const _: () = assert!(
    (STUECK_BYTES as usize) + 4096 < myl_types::protocol::MAX_ANFRAGE_BYTES,
    "ein Stück muss samt Rahmen durch den Anfragekanal passen"
);

/// Das Manifest eines Artefaktordners, als Text.
pub fn manifest_lesen(artefakte: &Path) -> Result<String, String> {
    let p = artefakte.join("weights_manifest.json");
    std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
}

/// **Ein Stück einer Datei für einen anderen Knoten**, oder `None`, wenn
/// der Name nicht im Manifest steht, nicht flach ist oder die Datei fehlt.
/// Zurück kommen die Bytes ab `ab` und die Größe der ganzen Datei.
pub fn stueck_lesen(artefakte: &Path, manifest: &str, name: &str, ab: u64, laenge: u64) -> Option<(Vec<u8>, u64)> {
    if !myl_store::teilablage::name_ist_flach(name) {
        return None;
    }
    let dateien = myl_store::teilablage::manifestdateien(manifest).ok()?;
    if !dateien.iter().any(|(_, d)| d.datei == name) {
        return None;
    }
    let mut f = std::fs::File::open(artefakte.join(name)).ok()?;
    let groesse = f.metadata().ok()?.len();
    if ab > groesse {
        return None;
    }
    let laenge = laenge.min(STUECK_BYTES).min(groesse - ab);
    f.seek(SeekFrom::Start(ab)).ok()?;
    let mut puffer = vec![0u8; laenge as usize];
    f.read_exact(&mut puffer).ok()?;
    Some((puffer, groesse))
}

/// **Welche Dateien eines Bereichs fehlen**: die das Manifest für ihn nennt
/// und die im Ordner nicht liegen.
///
/// ⚠️ **Nur auf Vorhandensein geprüft**, nicht auf die Summe: Die prüft die
/// Laufzeit beim Laden ohnehin, und alle Summen eines 35B je Epoche
/// nachzurechnen hieße, 19 GB zu lesen.
pub fn fehlende(
    artefakte: &Path,
    von: u32,
    bis: u32,
    mit_einbettung: bool,
    mit_kopf: bool,
) -> Result<Vec<Manifestdatei>, String> {
    let manifest = manifest_lesen(artefakte)?;
    let alle = myl_store::teilablage::dateien_des_bereichs(&manifest, von, bis, mit_einbettung, mit_kopf)
        .map_err(|e: Teilablagefehler| e.to_string())?;
    Ok(alle.into_iter().filter(|d| !artefakte.join(&d.datei).exists()).collect())
}

/// Was ein empfangenes Stück bewirkt hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stueckergebnis {
    /// Weiter mit dem nächsten Stück derselben Datei.
    Weiter,
    /// Die Datei ist vollständig, geprüft und liegt unter ihrem Namen.
    Fertig(String),
    /// Die Datei passte nicht zum Manifest und ist gelöscht; die Quelle
    /// wird übergangen.
    Verworfen(String),
    /// Ein Stück, das nicht zur laufenden Anfrage gehört.
    Fremd,
}

/// **Holt fehlende Dateien**, eine nach der anderen, Stück für Stück.
#[derive(Debug)]
pub struct Nachholer {
    artefakte: PathBuf,
    fehlend: VecDeque<Manifestdatei>,
    /// Die Datei in Arbeit und wie viel davon schon da ist.
    aktuell: Option<(Manifestdatei, u64)>,
}

impl Nachholer {
    /// Ein Nachholer für die gegebenen Dateien.
    pub fn neu(artefakte: PathBuf, fehlend: Vec<Manifestdatei>) -> Self {
        Self { artefakte, fehlend: fehlend.into(), aktuell: None }
    }

    /// Ist alles da?
    pub fn fertig(&self) -> bool {
        self.aktuell.is_none() && self.fehlend.is_empty()
    }

    /// Wie viele Dateien noch fehlen, die laufende mitgezählt.
    pub fn offen(&self) -> usize {
        self.fehlend.len() + usize::from(self.aktuell.is_some())
    }

    /// **Die nächste Anfrage**: Name, ab welchem Byte, wie viele.
    pub fn naechste_anfrage(&mut self) -> Option<(String, u64, u64)> {
        if self.aktuell.is_none() {
            let d = self.fehlend.pop_front()?;
            let _ = std::fs::remove_file(self.teil(&d.datei));
            self.aktuell = Some((d, 0));
        }
        let (d, ab) = self.aktuell.as_ref()?;
        Some((d.datei.clone(), *ab, STUECK_BYTES))
    }

    fn teil(&self, name: &str) -> PathBuf {
        self.artefakte.join(format!("{name}.teil"))
    }

    /// **Ein Stück ist angekommen.** Es wird an `<name>.teil` gehängt; ist
    /// die Datei vollständig, wird sie gegen das Manifest geprüft und
    /// umbenannt oder gelöscht.
    pub fn stueck_erhalten(&mut self, name: &str, ab: u64, daten: &[u8], groesse: u64) -> Stueckergebnis {
        let Some((d, schon)) = self.aktuell.as_mut() else {
            return Stueckergebnis::Fremd;
        };
        if d.datei != name || ab != *schon || daten.is_empty() && ab < groesse {
            return Stueckergebnis::Fremd;
        }
        let teil = self.artefakte.join(format!("{name}.teil"));
        let geschrieben = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&teil)
            .and_then(|mut f| f.write_all(daten));
        if geschrieben.is_err() {
            return Stueckergebnis::Fremd;
        }
        *schon += daten.len() as u64;
        if *schon < groesse {
            return Stueckergebnis::Weiter;
        }
        let datei = d.clone();
        self.aktuell = None;
        if summe_der_datei(&teil).as_deref() == Some(datei.sha256.as_str())
            && std::fs::rename(&teil, self.artefakte.join(&datei.datei)).is_ok()
        {
            Stueckergebnis::Fertig(datei.datei)
        } else {
            let _ = std::fs::remove_file(&teil);
            // ⚑ Wieder hinten anstellen: Eine andere Quelle liefert sie
            //   vielleicht richtig.
            self.fehlend.push_back(datei.clone());
            Stueckergebnis::Verworfen(datei.datei)
        }
    }

    /// Die Quelle hat die laufende Datei nicht: von vorn, bei der nächsten.
    pub fn quelle_hat_nicht(&mut self) {
        if let Some((d, _)) = self.aktuell.take() {
            let _ = std::fs::remove_file(self.teil(&d.datei));
            self.fehlend.push_back(d);
        }
    }
}

/// Die SHA-256 einer Datei als Hex, in Stücken gelesen.
fn summe_der_datei(pfad: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(pfad).ok()?;
    let mut h = Sha256::new();
    let mut puffer = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut puffer).ok()?;
        if n == 0 {
            break;
        }
        h.update(&puffer[..n]);
    }
    Some(hex::encode(h.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordner(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("myl-dateidienst-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("Ordner");
        p
    }

    /// Ein Artefakt mit zwei Ebenen und Kopf; die zweite Ebene ist größer
    /// als ein Stück.
    fn artefakt(dir: &Path) -> Vec<u8> {
        let gross: Vec<u8> = (0..(STUECK_BYTES as usize + 1000)).map(|i| (i % 251) as u8).collect();
        std::fs::write(dir.join("model_layers_0_a.bin"), b"ebene null").unwrap();
        std::fs::write(dir.join("model_layers_1_a.bin"), &gross).unwrap();
        std::fs::write(dir.join("lm_head.bin"), b"kopf").unwrap();
        std::fs::write(dir.join("geheim.txt"), b"nicht im Manifest").unwrap();
        let h = |b: &[u8]| myl_types::hash::Hash::sha256(b).to_hex();
        let manifest = format!(
            r#"{{"model_layers_0_a": {{"file": "model_layers_0_a.bin", "hash": "{}"}},
"model_layers_1_a": {{"file": "model_layers_1_a.bin", "hash": "{}"}},
"lm_head": {{"file": "lm_head.bin", "hash": "{}"}}}}"#,
            h(b"ebene null"),
            h(&gross),
            h(b"kopf")
        );
        std::fs::write(dir.join("weights_manifest.json"), manifest).unwrap();
        gross
    }

    /// ⚑ **Ausgeliefert wird nur, was das Manifest nennt**, in Stücken.
    #[test]
    fn ausgeliefert_wird_nur_was_das_manifest_nennt() {
        let quelle = ordner("quelle");
        let gross = artefakt(&quelle);
        let m = manifest_lesen(&quelle).unwrap();
        let (erstes, groesse) = stueck_lesen(&quelle, &m, "model_layers_1_a.bin", 0, u64::MAX).expect("Stück");
        assert_eq!((erstes.len() as u64, groesse), (STUECK_BYTES, gross.len() as u64));
        assert!(stueck_lesen(&quelle, &m, "geheim.txt", 0, 10).is_none(), "nicht im Manifest");
        assert!(stueck_lesen(&quelle, &m, "../geheim.txt", 0, 10).is_none(), "nicht flach");
        let _ = std::fs::remove_dir_all(&quelle);
    }

    /// ⚑ **Holen über Stücke, geprüft gegen das Manifest**: Was fehlt, wird
    /// geholt und liegt danach unter seinem Namen; eine verfälschte
    /// Lieferung wird gelöscht und wieder angestellt.
    #[test]
    fn fehlende_dateien_werden_geholt_und_geprueft() {
        let quelle = ordner("von");
        artefakt(&quelle);
        let ziel = ordner("nach");
        std::fs::copy(quelle.join("weights_manifest.json"), ziel.join("weights_manifest.json")).unwrap();
        std::fs::copy(quelle.join("model_layers_0_a.bin"), ziel.join("model_layers_0_a.bin")).unwrap();
        let fehlt = fehlende(&ziel, 0, 2, false, true).expect("Manifest");
        let namen: Vec<&str> = fehlt.iter().map(|d| d.datei.as_str()).collect();
        assert_eq!(namen, ["lm_head.bin", "model_layers_1_a.bin"], "Ebene 0 liegt schon da");
        let m = manifest_lesen(&quelle).unwrap();
        let mut n = Nachholer::neu(ziel.clone(), fehlt);
        // Erst eine verfälschte Lieferung des Kopfs.
        let (name, ab, _) = n.naechste_anfrage().expect("Anfrage");
        assert_eq!(n.stueck_erhalten(&name, ab, b"kopX", 4), Stueckergebnis::Verworfen("lm_head.bin".into()));
        assert!(!ziel.join("lm_head.bin").exists() && !ziel.join("lm_head.bin.teil").exists());
        // Dann richtig, über die Quelle.
        let mut runden = 0;
        while let Some((name, ab, laenge)) = n.naechste_anfrage() {
            let (daten, groesse) = stueck_lesen(&quelle, &m, &name, ab, laenge).expect("Stück");
            match n.stueck_erhalten(&name, ab, &daten, groesse) {
                Stueckergebnis::Weiter | Stueckergebnis::Fertig(_) => {}
                anders => panic!("{anders:?}"),
            }
            runden += 1;
            assert!(runden < 10, "kommt nicht voran");
        }
        assert!(n.fertig());
        assert_eq!(runden, 3, "die große Ebene in zwei Stücken, danach der wieder angestellte Kopf");
        assert!(fehlende(&ziel, 0, 2, false, true).unwrap().is_empty(), "alles da");
        assert_eq!(std::fs::read(ziel.join("model_layers_1_a.bin")).unwrap(), std::fs::read(quelle.join("model_layers_1_a.bin")).unwrap());
        let _ = std::fs::remove_dir_all(&quelle);
        let _ = std::fs::remove_dir_all(&ziel);
    }
}
