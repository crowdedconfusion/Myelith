//! Wo ein Knoten ablegt: die Speicherorte und die Zusage ans Netz.
//!
//! # ⚑ Eine Datei, die ein Mensch schreibt
//!
//! `speicherorte.toml` nennt jeden Ort mit Namen, Pfad, Obergrenze und
//! Zweck. **Gelesen wird sie von allen, die etwas ablegen oder suchen**:
//! vom Client, der eigene Gewichte dort findet, und vom Knoten, der
//! dort haelt, was er fuer das Netz gegen Entgelt speichert. Deshalb
//! steht das Format hier und nicht in einem der beiden.
//!
//! ```toml
//! [[ort]]
//! name = "extern"
//! pfad = "/Volumes/Platte/Myelith"
//! volumen_gib = 500
//! zwecke = ["gewichte", "netz"]
//! ```
//!
//! Unter dem Pfad liegen zwei Ordner, je nach Zweck: `gewichte/` fuer
//! eigene Modelle (ein Artefakt je Unterordner) und `netz/` fuer
//! Gegenstaende (einer je Unterordner, mit `manifest.borsh`).
//!
//! ⚑ **Die Obergrenze gilt fuer den ganzen Ort**, beide Ordner zusammen.
//! Zwei Grenzen fuer dieselbe Platte liefen auseinander, und keiner der
//! beiden Nutzer wuesste, was der andere schon belegt.
//!
//! # ⚑ Die Zusage
//!
//! Was ein Knoten ans Netz hergibt, ist ein Versprechen und keine
//! Einstellung: Wer es aendert, aendert die Verguetung. Es steht deshalb
//! als eigene Struktur da ([`Zusage`]), wird vom Kontoschluessel des
//! Knotens unterschrieben und nennt die Epoche, ab der es gilt
//! ([`botschaft`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Der uebliche Name der Datei.
pub const DATEINAME: &str = "speicherorte.toml";
/// Der Ordner fuer eigene Gewichte unter einem Ort.
pub const ORDNER_GEWICHTE: &str = "gewichte";
/// Der Ordner fuer Gegenstaende des Netzes unter einem Ort.
pub const ORDNER_NETZ: &str = "netz";
/// Die Datei, an der ein Gegenstand im Ordner `netz/` erkannt wird.
pub const MANIFESTDATEI: &str = "manifest.borsh";
/// Ein Gibibyte.
pub const GIB: u64 = 1 << 30;

/// Was an einem Ort liegen darf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Zweck {
    /// Eigene Modelle: Artefakte und Gewichte, die der Client dort sucht.
    Gewichte,
    /// Gegenstaende, die der Knoten fuer das Netz haelt.
    Netz,
}

impl Zweck {
    /// Der Unterordner, in dem dieser Zweck ablegt.
    pub fn ordner(self) -> &'static str {
        match self {
            Zweck::Gewichte => ORDNER_GEWICHTE,
            Zweck::Netz => ORDNER_NETZ,
        }
    }
}

/// Ein Ort: ein Ordner mit einer Obergrenze.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speicherort {
    /// Eindeutig in der Datei; daran haengen Verweise.
    pub name: String,
    /// Absolut. Ein relativer Pfad hinge am Arbeitsverzeichnis dessen,
    /// der die Datei gerade liest, und Client und Knoten haben
    /// verschiedene.
    pub pfad: PathBuf,
    /// Wie viel hier hoechstens liegen darf, beide Zwecke zusammen.
    pub volumen_gib: u64,
    /// Was hier liegen darf.
    pub zwecke: Vec<Zweck>,
}

impl Speicherort {
    /// Die Obergrenze in Bytes.
    pub fn volumen_bytes(&self) -> u64 {
        self.volumen_gib.saturating_mul(GIB)
    }

    /// Ob dieser Ort dem Zweck dient.
    pub fn dient(&self, z: Zweck) -> bool {
        self.zwecke.contains(&z)
    }

    /// Der Ordner fuer einen Zweck unter diesem Ort.
    pub fn ordner(&self, z: Zweck) -> PathBuf {
        self.pfad.join(z.ordner())
    }
}

/// Alle Orte eines Knotens.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speicherorte {
    #[serde(default, rename = "ort")]
    pub orte: Vec<Speicherort>,
}

/// Warum eine Datei nicht taugt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrtFehler {
    /// Lesen oder Schreiben ging nicht.
    Datei { pfad: PathBuf, grund: String },
    /// Kein gueltiges TOML oder falsche Felder.
    Format(String),
    /// Ein Eintrag widerspricht den Regeln.
    Eintrag { name: String, grund: String },
}

impl std::fmt::Display for OrtFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrtFehler::Datei { pfad, grund } => write!(f, "{}: {grund}", pfad.display()),
            OrtFehler::Format(g) => write!(f, "{DATEINAME}: {g}"),
            OrtFehler::Eintrag { name, grund } => write!(f, "{DATEINAME}, Ort \"{name}\": {grund}"),
        }
    }
}

impl std::error::Error for OrtFehler {}

/// Der Kopf, den jede geschriebene Datei traegt.
const KOPF: &str = "\
# Speicherorte dieses Knotens.
#
# Jeder Ort ist ein Ordner mit einer Obergrenze (volumen_gib, beide Zwecke
# zusammen). zwecke sagt, was dort liegen darf:
#   \"gewichte\"  eigene Modelle; der Client sucht Artefakte unter <pfad>/gewichte/
#   \"netz\"      was der Knoten fuer das Netz gegen Entgelt haelt, unter <pfad>/netz/
#
# Die Datei darf von Hand bearbeitet werden; pfad muss absolut sein.

";

impl Speicherorte {
    /// Liest die Datei. ⚑ **Fehlt sie, gibt es keine Orte**, und das ist
    /// kein Fehler; eine kaputte Datei dagegen schon, denn sie still zu
    /// uebergehen hiesse, Orte zu vergessen, die jemand eingetragen hat.
    pub fn lesen(pfad: &Path) -> Result<Self, OrtFehler> {
        match std::fs::read_to_string(pfad) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(OrtFehler::Datei { pfad: pfad.to_path_buf(), grund: e.to_string() }),
            Ok(t) => Self::aus_text(&t),
        }
    }

    /// Zerlegt und prueft den Text.
    pub fn aus_text(text: &str) -> Result<Self, OrtFehler> {
        let s: Self = toml::from_str(text).map_err(|e| OrtFehler::Format(e.message().to_string()))?;
        s.pruefen()?;
        Ok(s)
    }

    /// Die Regeln: Namen eindeutig und nicht leer, Pfad absolut,
    /// Obergrenze ueber null, mindestens ein Zweck.
    pub fn pruefen(&self) -> Result<(), OrtFehler> {
        let mut namen = std::collections::BTreeSet::new();
        for o in &self.orte {
            let fehler = |grund: &str| Err(OrtFehler::Eintrag { name: o.name.clone(), grund: grund.to_string() });
            if o.name.trim().is_empty() {
                return fehler("ohne Namen");
            }
            if !namen.insert(o.name.as_str()) {
                return fehler("der Name steht zweimal da");
            }
            if !o.pfad.is_absolute() {
                return fehler("der Pfad ist nicht absolut");
            }
            if o.volumen_gib == 0 {
                return fehler("volumen_gib ist null");
            }
            if o.zwecke.is_empty() {
                return fehler("ohne Zweck (\"gewichte\", \"netz\")");
            }
        }
        Ok(())
    }

    /// Der Text der Datei, mit Kopf.
    pub fn als_text(&self) -> String {
        let rumpf = toml::to_string(self).unwrap_or_default();
        format!("{KOPF}{rumpf}")
    }

    /// Schreibt die Datei, ⚑ **erst daneben, dann an ihren Platz**: Ein
    /// Abbruch mitten im Schreiben hinterlaesst so die alte Datei und
    /// keine halbe.
    pub fn schreiben(&self, pfad: &Path) -> Result<(), OrtFehler> {
        self.pruefen()?;
        let fehler = |p: &Path, e: std::io::Error| OrtFehler::Datei { pfad: p.to_path_buf(), grund: e.to_string() };
        if let Some(v) = pfad.parent() {
            std::fs::create_dir_all(v).map_err(|e| fehler(v, e))?;
        }
        let neben = pfad.with_extension("toml.neu");
        std::fs::write(&neben, self.als_text()).map_err(|e| fehler(&neben, e))?;
        std::fs::rename(&neben, pfad).map_err(|e| fehler(pfad, e))
    }

    /// Die Orte, die einem Zweck dienen, in der Reihenfolge der Datei.
    pub fn fuer(&self, z: Zweck) -> impl Iterator<Item = &Speicherort> {
        self.orte.iter().filter(move |o| o.dient(z))
    }

    /// Ein Ort nach Namen.
    pub fn ort(&self, name: &str) -> Option<&Speicherort> {
        self.orte.iter().find(|o| o.name == name)
    }

    /// Setzt einen Ort: ersetzt den gleichnamigen oder haengt ihn an.
    pub fn setzen(&mut self, ort: Speicherort) {
        match self.orte.iter_mut().find(|o| o.name == ort.name) {
            Some(alt) => *alt = ort,
            None => self.orte.push(ort),
        }
    }
}

/// Was an einem Ort liegt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Belegung {
    /// Gegenstaende im Ordner `netz/`, erkannt am Manifest.
    pub gegenstaende: u64,
    /// Davon Shard-Gewichte.
    pub shards: u64,
    /// Unterordner in `netz/`, deren Manifest fehlt oder sich nicht lesen
    /// laesst. ⚑ Sie zaehlen nicht als Gegenstand, werden aber genannt:
    /// Ein halb geschriebener Gegenstand ist kein gehaltener.
    pub unlesbar: u64,
    /// Bytes unter dem ganzen Ort, beide Zwecke zusammen.
    pub bytes: u64,
}

/// Zaehlt, was an einem Ort liegt.
///
/// ⚑ **Gezaehlt wird, was auf der Platte steht**, nicht was der Knoten
/// zu halten glaubt. Wer von Hand einen Ordner loescht, sieht das in der
/// naechsten Zahl.
pub fn belegung(ort: &Speicherort) -> Belegung {
    let mut b = Belegung { bytes: groesse(&ort.pfad), ..Default::default() };
    let Ok(eintraege) = std::fs::read_dir(ort.ordner(Zweck::Netz)) else {
        return b;
    };
    for e in eintraege.flatten() {
        if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let manifest = std::fs::read(e.path().join(MANIFESTDATEI))
            .ok()
            .and_then(|roh| borsh::from_slice::<crate::Manifest>(&roh).ok());
        match manifest {
            Some(m) => {
                b.gegenstaende += 1;
                if m.art == crate::Gegenstandsart::Shardgewichte {
                    b.shards += 1;
                }
            }
            None => b.unlesbar += 1,
        }
    }
    b
}

/// Bytes unter einem Pfad, Verknuepfungen nicht verfolgt.
///
/// ⚑ **Nicht verfolgt**, sonst zaehlte ein Verweis auf die halbe Platte
/// als Belegung dieses Orts, oder eine Schleife zaehlte ohne Ende.
pub fn groesse(pfad: &Path) -> u64 {
    let Ok(m) = std::fs::symlink_metadata(pfad) else { return 0 };
    if m.is_file() {
        return m.len();
    }
    if !m.is_dir() {
        return 0;
    }
    let Ok(eintraege) = std::fs::read_dir(pfad) else { return 0 };
    eintraege.flatten().map(|e| groesse(&e.path())).fold(0u64, u64::saturating_add)
}

/// Domaenentrenner fuer die Unterschrift unter einer Zusage.
pub const DST_ZUSAGE: &[u8] = b"MYELITH-KAPAZITAETSZUSAGE-V1";

/// Was ein Knoten dem Netz zusagt.
///
/// ⚠️ **Im Probelauf ist sie eine Uebung.** `probelauf` steht deshalb im
/// Text selbst und damit unter der Unterschrift: Eine Zusage aus einem
/// Probelauf laesst sich nicht als eine fuers Netz ausgeben, ohne die
/// Unterschrift zu brechen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zusage {
    /// Fassung dieses Formats.
    pub fassung: u32,
    pub probelauf: bool,
    /// Die Orte fuer das Netz, mit ihrer Obergrenze.
    pub orte: Vec<Zusageort>,
    /// Wie viele CPU-Kerne.
    pub kerne: u32,
    /// Kennungen der freigegebenen Rechenwerke aus dem Hardwarescan.
    pub rechenwerke: Vec<String>,
    /// Wann sie entstand, in Millisekunden seit der Epoche (Unix).
    pub erstellt_ms: u64,
}

/// Ein Ort in einer Zusage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zusageort {
    pub name: String,
    pub pfad: PathBuf,
    pub volumen_bytes: u64,
}

impl Zusage {
    /// Die Fassung, die dieser Code schreibt.
    pub const FASSUNG: u32 = 1;

    /// Wie viele Bytes insgesamt zugesagt sind.
    pub fn volumen_bytes(&self) -> u64 {
        self.orte.iter().map(|o| o.volumen_bytes).fold(0u64, u64::saturating_add)
    }

    /// Der Text der Datei, TOML wie die Speicherorte.
    pub fn als_text(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    /// Zerlegt eine Zusage; ⚑ **eine fremde Fassung wird abgelehnt**, denn
    /// wer sie unterschreibt, verspricht, was darin steht.
    pub fn aus_text(text: &str) -> Result<Self, OrtFehler> {
        let z: Self = toml::from_str(text).map_err(|e| OrtFehler::Format(e.message().to_string()))?;
        if z.fassung != Self::FASSUNG {
            return Err(OrtFehler::Format(format!("Zusage in Fassung {}, erwartet {}", z.fassung, Self::FASSUNG)));
        }
        Ok(z)
    }
}

/// Was unterschrieben wird: Trenner, SHA-256 der Datei, Epoche.
///
/// ⚑ **Ueber die Bytes der Datei, nicht ueber eine neu kodierte
/// Struktur.** Wer nachprueft, hat die Datei; muesste er sie erst
/// zerlegen und gleich wieder kodieren, haenge die Pruefung an der
/// Fassung seiner Bibliothek.
pub fn botschaft(datei: &[u8], epoche: u64) -> Vec<u8> {
    let mut b = DST_ZUSAGE.to_vec();
    b.extend_from_slice(myl_types::hash::Hash::sha256(datei).as_bytes());
    b.extend_from_slice(&epoche.to_le_bytes());
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ort(name: &str, pfad: &str) -> Speicherort {
        Speicherort { name: name.into(), pfad: pfad.into(), volumen_gib: 10, zwecke: vec![Zweck::Netz] }
    }

    #[test]
    fn eine_datei_kommt_so_zurueck_wie_sie_geschrieben_wurde() {
        let mut s = Speicherorte::default();
        s.setzen(Speicherort { zwecke: vec![Zweck::Gewichte, Zweck::Netz], ..ort("extern", "/Volumes/Platte/Myelith") });
        s.setzen(ort("intern", "/home/a/myelith"));
        let text = s.als_text();
        assert!(text.starts_with("# Speicherorte"), "der Kopf fehlt");
        assert!(text.contains("zwecke = [\"gewichte\", \"netz\"]"), "{text}");
        assert_eq!(Speicherorte::aus_text(&text).unwrap(), s);
    }

    #[test]
    fn setzen_ersetzt_den_gleichnamigen() {
        let mut s = Speicherorte::default();
        s.setzen(ort("a", "/x"));
        s.setzen(Speicherort { volumen_gib: 99, ..ort("a", "/y") });
        assert_eq!(s.orte.len(), 1);
        assert_eq!(s.orte[0].volumen_gib, 99);
    }

    #[test]
    fn die_regeln_greifen() {
        let fall = |t: &str| Speicherorte::aus_text(t).unwrap_err().to_string();
        assert!(fall("[[ort]]\nname=\"a\"\npfad=\"rel\"\nvolumen_gib=1\nzwecke=[\"netz\"]").contains("nicht absolut"));
        assert!(fall("[[ort]]\nname=\"a\"\npfad=\"/x\"\nvolumen_gib=0\nzwecke=[\"netz\"]").contains("null"));
        assert!(fall("[[ort]]\nname=\"a\"\npfad=\"/x\"\nvolumen_gib=1\nzwecke=[]").contains("ohne Zweck"));
        assert!(fall("[[ort]]\nname=\"a\"\npfad=\"/x\"\nvolumen_gib=1\nzwecke=[\"netz\"]\n[[ort]]\nname=\"a\"\npfad=\"/y\"\nvolumen_gib=1\nzwecke=[\"netz\"]").contains("zweimal"));
        assert!(fall("[[ort]]\nname=\"a\"\npfad=\"/x\"\nvolumen_gib=1\nzwecke=[\"kaffee\"]").starts_with(DATEINAME));
    }

    #[test]
    fn eine_fehlende_datei_ist_leer_und_kein_fehler() {
        let p = std::env::temp_dir().join(format!("myl-orte-fehlt-{}", std::process::id())).join(DATEINAME);
        assert_eq!(Speicherorte::lesen(&p).unwrap(), Speicherorte::default());
    }

    /// ⚑ **Gezaehlt wird am Manifest**: Ein Ordner ohne Manifest ist kein
    /// Gegenstand, ein Shard wird an seiner Art erkannt, und die Bytes
    /// umfassen den ganzen Ort.
    #[test]
    fn die_belegung_zaehlt_was_auf_der_platte_steht() {
        let wurzel = std::env::temp_dir().join(format!("myl-orte-beleg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&wurzel);
        let o = Speicherort { pfad: wurzel.clone(), ..ort("t", "/") };
        let teile = crate::teile_bilden(&[7u8; 100]).unwrap();
        let manifest = |art| crate::Manifest::neu(art, 1, &teile, crate::Redundanzform::Kopien { anzahl: 1 }).unwrap();
        for (n, art) in [("a", crate::Gegenstandsart::Shardgewichte), ("b", crate::Gegenstandsart::Skalenpaket)] {
            let d = o.ordner(Zweck::Netz).join(n);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(MANIFESTDATEI), borsh::to_vec(&manifest(art)).unwrap()).unwrap();
        }
        std::fs::create_dir_all(o.ordner(Zweck::Netz).join("halb")).unwrap();
        std::fs::create_dir_all(o.ordner(Zweck::Gewichte)).unwrap();
        std::fs::write(o.ordner(Zweck::Gewichte).join("w.bin"), [0u8; 1000]).unwrap();
        let b = belegung(&o);
        assert_eq!((b.gegenstaende, b.shards, b.unlesbar), (2, 1, 1));
        assert!(b.bytes >= 1000, "{b:?}");
        let _ = std::fs::remove_dir_all(&wurzel);
    }

    #[test]
    fn eine_zusage_kommt_so_zurueck_wie_sie_geschrieben_wurde() {
        let z = Zusage {
            fassung: Zusage::FASSUNG,
            probelauf: true,
            orte: vec![Zusageort { name: "a".into(), pfad: "/x".into(), volumen_bytes: 3 * GIB }],
            kerne: 4,
            rechenwerke: vec!["gpu0".into()],
            erstellt_ms: 1,
        };
        assert_eq!(Zusage::aus_text(&z.als_text()).unwrap(), z);
        assert_eq!(z.volumen_bytes(), 3 * GIB);
        let fremd = z.als_text().replace("fassung = 1", "fassung = 9");
        assert!(Zusage::aus_text(&fremd).unwrap_err().to_string().contains("Fassung 9"));
    }

    /// ⚑ **Die Epoche geht in die Botschaft**: Dieselbe Datei in einer
    /// anderen Epoche ist eine andere Zusage.
    #[test]
    fn die_botschaft_haengt_an_datei_und_epoche() {
        let a = botschaft(b"x", 1);
        assert!(a.starts_with(DST_ZUSAGE));
        assert_ne!(a, botschaft(b"x", 2));
        assert_ne!(a, botschaft(b"y", 1));
        assert_eq!(a, botschaft(b"x", 1));
    }
}
