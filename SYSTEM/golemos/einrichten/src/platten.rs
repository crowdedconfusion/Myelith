//! Welche Platte ist die, von der gestartet wurde, und welche darf
//! beschrieben werden?
//!
//! ⛔️ **Die wichtigste Frage der ganzen Kiste.** Alles andere laesst
//! sich wiederholen; eine falsch gewaehlte Platte nicht. Deshalb steht
//! hier nicht eine Liste der guten Platten, sondern ein **Urteil ueber
//! jede**, mit Grund. Der Assistent zeigt auch die abgelehnten, damit
//! niemand raetselt, wo seine Platte geblieben ist.

use crate::geraete::Geraet;

/// GPT-Typ einer EFI-Systempartition.
pub const TYP_EFI: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";

/// Die Partitionsnamen, die `genimage.cfg` vergibt.
pub const NAME_EFI: &str = "boot";
pub const NAME_WURZEL_A: &str = "golem-wurzelA";
pub const NAME_WURZEL_B: &str = "golem-wurzelB";
pub const NAME_DATEN: &str = "golem-daten";

/// Kleinster Platz, den die Datenpartition auf der Zielplatte haben muss.
///
/// ⚑ Ein Gigabyte: Das kleinste Artefakt wiegt 0,9 GB. Wer weniger hat,
/// bekommt ein System, das startet und kein Modell tragen kann, und das
/// soll der Assistent vorher sagen und nicht hinterher.
pub const MINDEST_DATEN: u64 = 1 << 30;

/// Platz fuer die Ausrichtung auf ganze MiB und die hintere GPT-Kopie.
pub const RESERVE: u64 = 8 << 20;

/// Die Platte, von der das laufende System kommt.
///
/// ⚑ **Erst die Wurzel, dann die Datenpartition.** Nach einem Start von
/// Platte ist `/` eine ihrer Partitionen. Beim Start mit Kern und
/// Speicherdateisystem (so laeuft die Selbstprobe) ist `/` gar kein
/// Blockgeraet; dann verraet `/daten`, welcher Stick gemeint ist.
pub fn startplatte(g: &[Geraet]) -> Option<String> {
    for ort in ["/", "/daten"] {
        if let Some(p) = g
            .iter()
            .find(|x| x.ist_partition() && x.einhaengung == ort && !x.eltern.is_empty())
        {
            return Some(p.eltern.clone());
        }
    }
    None
}

/// Die Partitionen, aus denen eine Installation kopiert.
#[derive(Clone, Debug)]
pub struct Quelle {
    pub platte: Geraet,
    pub efi: Geraet,
    pub wurzel_a: Geraet,
    pub wurzel_b: Geraet,
}

impl Quelle {
    /// Was die drei Systempartitionen zusammen belegen.
    pub fn system_bytes(&self) -> u64 {
        self.efi.groesse + self.wurzel_a.groesse + self.wurzel_b.groesse
    }

    /// Die kleinste Zielplatte, auf die das System samt Daten passt.
    pub fn mindestgroesse(&self) -> u64 {
        self.system_bytes() + MINDEST_DATEN + RESERVE
    }
}

/// Sucht auf der Startplatte die drei Systempartitionen.
pub fn quelle(g: &[Geraet], start: &str) -> Result<Quelle, String> {
    let platte = g
        .iter()
        .find(|x| x.ist_platte() && x.name == start)
        .ok_or_else(|| format!("die Startplatte {start} steht nicht in der Geraeteliste"))?
        .clone();
    let teil = |name: &str| {
        g.iter()
            .find(|x| x.ist_partition() && x.eltern == start && x.partname == name)
            .cloned()
    };
    // ⚠️ Die EFI-Partition auch am Typ erkennen: Der Name `boot` ist
    //    der aus `genimage.cfg`, und ein kuenftiges Abbild darf ihn
    //    aendern, ohne dass die Installation daran zerbricht.
    let efi = teil(NAME_EFI).or_else(|| {
        g.iter()
            .find(|x| x.ist_partition() && x.eltern == start && x.parttyp == TYP_EFI)
            .cloned()
    });
    match (efi, teil(NAME_WURZEL_A), teil(NAME_WURZEL_B)) {
        (Some(efi), Some(wurzel_a), Some(wurzel_b)) => Ok(Quelle { platte, efi, wurzel_a, wurzel_b }),
        _ => Err(format!(
            "auf {start} fehlen die Systempartitionen ({NAME_EFI}, {NAME_WURZEL_A}, {NAME_WURZEL_B}); \
             installiert werden kann nur von einem GolemOS-Datentraeger"
        )),
    }
}

/// Warum eine Platte in Frage kommt oder nicht.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Urteil {
    Geeignet,
    IstStartplatte,
    /// Eine Partition darauf ist eingehaengt, der Ort steht dabei.
    InBenutzung(String),
    ZuKlein { hat: u64, braucht: u64 },
}

impl Urteil {
    pub fn geeignet(&self) -> bool {
        *self == Urteil::Geeignet
    }

    pub fn grund(&self) -> String {
        self.grund_in(crate::sprache::Sprache::De)
    }

    pub fn grund_in(&self, s: crate::sprache::Sprache) -> String {
        if s == crate::sprache::Sprache::En {
            return match self {
                Urteil::Geeignet => "suitable".into(),
                Urteil::IstStartplatte => "GolemOS is running from this disk".into(),
                Urteil::InBenutzung(ort) => format!("mounted at {ort}"),
                Urteil::ZuKlein { hat, braucht } => format!(
                    "too small: {} instead of at least {}",
                    crate::geraete::gb(*hat),
                    crate::geraete::gb(*braucht)
                ),
            };
        }
        match self {
            Urteil::Geeignet => "geeignet".into(),
            Urteil::IstStartplatte => "von dieser Platte laeuft GolemOS gerade".into(),
            Urteil::InBenutzung(ort) => format!("eingehaengt unter {ort}"),
            Urteil::ZuKlein { hat, braucht } => format!(
                "zu klein: {} statt mindestens {}",
                crate::geraete::gb(*hat),
                crate::geraete::gb(*braucht)
            ),
        }
    }
}

/// Keine Platte im Sinne des Assistenten: Speicher, der wie eine aussieht.
fn scheinplatte(p: &Geraet) -> bool {
    p.groesse == 0 || ["zram", "ram", "loop"].iter().any(|v| p.name.starts_with(v))
}

/// Ein Urteil ueber jede Platte, in der Reihenfolge von `lsblk`.
pub fn beurteilen(g: &[Geraet], start: &str, mindest: u64) -> Vec<(Geraet, Urteil)> {
    g.iter()
        .filter(|p| p.ist_platte() && !scheinplatte(p))
        .map(|p| {
            let urteil = if p.name == start {
                Urteil::IstStartplatte
            } else if let Some(ort) = eingehaengt(g, &p.name) {
                Urteil::InBenutzung(ort)
            } else if p.groesse < mindest {
                Urteil::ZuKlein { hat: p.groesse, braucht: mindest }
            } else {
                Urteil::Geeignet
            };
            (p.clone(), urteil)
        })
        .collect()
}

/// Wo die Platte oder eine ihrer Partitionen eingehaengt ist.
fn eingehaengt(g: &[Geraet], platte: &str) -> Option<String> {
    g.iter()
        .find(|x| (x.name == platte || x.eltern == platte) && !x.einhaengung.is_empty())
        .map(|x| x.einhaengung.clone())
}

#[cfg(test)]
pub(crate) mod proben {
    use super::*;

    pub fn platte(name: &str, groesse: u64) -> Geraet {
        Geraet { name: name.into(), art: "disk".into(), groesse, ..Default::default() }
    }

    pub fn teil(name: &str, eltern: &str, partname: &str, groesse: u64, ort: &str) -> Geraet {
        Geraet {
            name: name.into(),
            eltern: eltern.into(),
            art: "part".into(),
            groesse,
            partname: partname.into(),
            einhaengung: ort.into(),
            ..Default::default()
        }
    }

    /// Ein Stick wie nach dem ersten Start, dazu eine leere Platte.
    pub fn stick_und_platte() -> Vec<Geraet> {
        vec![
            platte("vda", 3 << 30),
            teil("vda1", "vda", NAME_EFI, 64 << 20, ""),
            teil("vda2", "vda", NAME_WURZEL_A, 400 << 20, ""),
            teil("vda3", "vda", NAME_WURZEL_B, 400 << 20, ""),
            teil("vda4", "vda", NAME_DATEN, 2 << 30, "/daten"),
            platte("vdb", 4 << 30),
        ]
    }

    #[test]
    fn startplatte_aus_der_wurzel() {
        let mut g = stick_und_platte();
        g[2].einhaengung = "/".into();
        g[4].einhaengung.clear();
        assert_eq!(startplatte(&g).as_deref(), Some("vda"));
    }

    #[test]
    fn startplatte_aus_den_daten_wenn_die_wurzel_kein_geraet_ist() {
        assert_eq!(startplatte(&stick_und_platte()).as_deref(), Some("vda"));
    }

    #[test]
    fn ohne_hinweis_keine_startplatte() {
        let mut g = stick_und_platte();
        g[4].einhaengung.clear();
        assert_eq!(startplatte(&g), None);
    }

    #[test]
    fn quelle_findet_die_drei_systempartitionen() {
        let q = quelle(&stick_und_platte(), "vda").unwrap();
        assert_eq!(q.efi.name, "vda1");
        assert_eq!(q.wurzel_b.name, "vda3");
        assert_eq!(q.system_bytes(), 864 << 20);
    }

    #[test]
    fn quelle_ohne_systempartitionen_ist_ein_fehler() {
        let g = vec![platte("sda", 1 << 40), teil("sda1", "sda", "irgendwas", 1 << 30, "/")];
        assert!(quelle(&g, "sda").unwrap_err().contains("GolemOS-Datentraeger"));
    }

    #[test]
    fn die_startplatte_ist_nie_geeignet() {
        let g = stick_und_platte();
        let u = beurteilen(&g, "vda", 1 << 30);
        assert_eq!(u[0].1, Urteil::IstStartplatte);
        assert_eq!(u[1].1, Urteil::Geeignet);
    }

    /// ⛔️ Die Gegenprobe zur Startplatte: Auch ohne diesen Schutz duerfte
    /// eine Platte mit eingehaengter Partition nie angeboten werden.
    #[test]
    fn eingehaengte_platte_ist_nicht_geeignet() {
        let mut g = stick_und_platte();
        g.push(platte("sdc", 1 << 40));
        g.push(teil("sdc1", "sdc", "", 1 << 40, "/medien/sdc1"));
        let u = beurteilen(&g, "vda", 1 << 30);
        assert_eq!(u[2].1, Urteil::InBenutzung("/medien/sdc1".into()));
    }

    #[test]
    fn zu_kleine_platte_nennt_beide_zahlen() {
        let g = vec![platte("vda", 1), platte("sdb", 1_000_000_000)];
        let u = beurteilen(&g, "vda", 2_000_000_000);
        assert_eq!(u[1].1.grund(), "zu klein: 1,0 GB statt mindestens 2,0 GB");
    }

    #[test]
    fn scheinplatten_erscheinen_nicht() {
        let g = vec![platte("zram0", 1 << 30), platte("nbd0", 0), platte("loop0", 1 << 30)];
        assert!(beurteilen(&g, "vda", 1).is_empty());
    }
}
