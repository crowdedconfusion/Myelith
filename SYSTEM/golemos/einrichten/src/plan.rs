//! Was eine Installation tun wird, bevor sie es tut.
//!
//! ⚑ **Plan und Ausfuehrung sind getrennt.** Der Plan ist eine reine
//! Rechnung aus der Geraeteliste und laesst sich pruefen, ohne eine
//! Platte anzufassen. Der Assistent zeigt ihn dem Menschen, und erst
//! nach dessen Bestaetigung fuehrt `ausfuehren` genau diesen Plan aus.

use crate::geraete::{gb, Geraet};
use crate::grub::Kennungen;
use crate::platten::{self, Quelle, Urteil, NAME_DATEN, NAME_WURZEL_A, NAME_WURZEL_B, RESERVE};

/// GPT-Typ „Linux-Dateisystem", derselbe wie in `genimage.cfg`.
pub const TYP_LINUX: &str = "0fc63daf-8483-4772-8e79-3d69d8477de4";

/// GPT-Typ „Microsoft Basic Data" fuer die Datenpartition.
///
/// ⛔️ **Fund 476: Mit dem Linux-Typ sah kein fremdes System die Daten.**
/// Die Partition trug FAT32, damit jeder Rechner daraufschreiben kann,
/// aber macOS und Windows entscheiden am Typ, ob sie ueberhaupt
/// nachsehen. Mit dem Linux-Typ meldete macOS „kein Dateisystem", mit
/// diesem haengt es die Datenpartition sofort ein (gemessen am 2026-09-25).
/// Linux selbst schaut auf das Dateisystem und nicht auf den Typ.
pub const TYP_DATEN: &str = "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7";

/// Eine Partition auf der Zielplatte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Teil {
    /// Der GPT-Name, zugleich die Marke, an der GRUB und `S05daten` suchen.
    pub name: String,
    pub typ: String,
    /// Groesse in Bytes; `None` heisst: der ganze Rest der Platte.
    pub groesse: Option<u64>,
    /// Die Partition, deren Inhalt Byte fuer Byte hierher kopiert wird.
    pub quelle: Option<String>,
}

/// Die ganze Installation, bevor sie geschieht.
#[derive(Clone, Debug)]
pub struct Plan {
    /// Kernname der Zielplatte, etwa `sda`.
    pub ziel: String,
    pub ziel_groesse: u64,
    pub ziel_modell: String,
    pub teile: Vec<Teil>,
    /// Was die Datenpartition auf der Zielplatte fassen wird.
    pub daten_platz: u64,
    /// Die Kennungen der Quelle, auf die die kopierte `grub.cfg` zeigt.
    pub alt: Kennungen,
}

/// Entwirft die Installation von `q` auf `ziel`.
///
/// ⛔️ **Die Pruefungen aus `platten::beurteilen` stehen hier ein zweites
/// Mal**, und das ist die eine Stelle, an der die Wiederholung gewollt
/// ist. Der Assistent bietet nur geeignete Platten an; der
/// Kommandozeilenweg aber nimmt einen Namen, den ein Mensch getippt hat.
/// Wer dort die Startplatte nennt, soll hier scheitern und nicht erst
/// an `sfdisk`.
pub fn entwerfen(q: &Quelle, ziel: &Geraet, alle: &[Geraet]) -> Result<Plan, String> {
    let urteil = platten::beurteilen(alle, &q.platte.name, q.mindestgroesse())
        .into_iter()
        .find(|(p, _)| p.name == ziel.name)
        .map(|(_, u)| u)
        .ok_or_else(|| format!("{} ist keine Platte, die sich beschreiben liesse", ziel.name))?;
    if urteil != Urteil::Geeignet {
        return Err(format!("{}: {}", ziel.name, urteil.grund()));
    }

    // ⚑ **Die Wurzel wird aus der Partition kopiert, die NICHT laeuft.**
    //    Nach einem Start von Platte ist eine der beiden eingehaengt und
    //    beschreibbar; eine Kopie davon koennte mitten in einem
    //    Schreibvorgang entstehen. Die andere traegt dasselbe Abbild und
    //    ist in Ruhe. Beide Zielpartitionen bekommen diese eine Quelle.
    let wurzel = [&q.wurzel_b, &q.wurzel_a]
        .into_iter()
        .find(|w| w.einhaengung.is_empty())
        .ok_or("beide Wurzelpartitionen sind eingehaengt; so startet GolemOS nie")?;

    let teile = vec![
        Teil {
            name: q.efi.partname.clone(),
            typ: platten::TYP_EFI.into(),
            groesse: Some(q.efi.groesse),
            quelle: Some(q.efi.pfad()),
        },
        Teil {
            name: NAME_WURZEL_A.into(),
            typ: TYP_LINUX.into(),
            groesse: Some(wurzel.groesse),
            quelle: Some(wurzel.pfad()),
        },
        Teil {
            name: NAME_WURZEL_B.into(),
            typ: TYP_LINUX.into(),
            groesse: Some(wurzel.groesse),
            quelle: Some(wurzel.pfad()),
        },
        Teil { name: NAME_DATEN.into(), typ: TYP_DATEN.into(), groesse: None, quelle: None },
    ];
    let belegt: u64 = teile.iter().filter_map(|t| t.groesse).sum();
    Ok(Plan {
        ziel: ziel.name.clone(),
        ziel_groesse: ziel.groesse,
        ziel_modell: ziel.modell.clone(),
        teile,
        daten_platz: ziel.groesse.saturating_sub(belegt + RESERVE),
        alt: Kennungen {
            a: q.wurzel_a.partuuid.clone(),
            b: q.wurzel_b.partuuid.clone(),
            // Die Datenkennung wird in jeder Kernzeile ersetzt, gleich
            // welche dort stand; es gibt nur eine Datenpartition.
            daten: String::new(),
        },
    })
}

impl Plan {
    /// Die Eingabe fuer `sfdisk`.
    ///
    /// ⚑ **Groessen in Sektoren, Anfaenge offen.** `sfdisk` richtet dann
    /// jeden Anfang auf ganze MiB aus, und die letzte Partition ohne
    /// Groesse bekommt den Rest. Die Kennungen (PARTUUID) vergibt es
    /// selbst, zufaellig; `ausfuehren` liest sie danach aus.
    pub fn sfdisk_eingabe(&self) -> String {
        let mut s = String::from("label: gpt\nunit: sectors\n\n");
        for t in &self.teile {
            match t.groesse {
                Some(b) => s.push_str(&format!(
                    "size={}, type={}, name=\"{}\"\n",
                    b.div_ceil(512),
                    t.typ.to_uppercase(),
                    t.name
                )),
                None => s.push_str(&format!("type={}, name=\"{}\"\n", t.typ.to_uppercase(), t.name)),
            }
        }
        s
    }

    /// Der Plan fuer Menschen, so wie der Assistent ihn zeigt.
    pub fn beschreiben(&self) -> String {
        self.beschreiben_in(crate::sprache::Sprache::De)
    }

    pub fn beschreiben_in(&self, sp: crate::sprache::Sprache) -> String {
        let mut s = format!(
            "{} /dev/{} ({}{})\n",
            sp.t("Zielplatte", "Target disk"),
            self.ziel,
            gb(self.ziel_groesse),
            if self.ziel_modell.is_empty() { String::new() } else { format!(", {}", self.ziel_modell) }
        );
        for (i, t) in self.teile.iter().enumerate() {
            let groesse = t
                .groesse
                .map(gb)
                .unwrap_or_else(|| format!("{}, {}", sp.t("Rest", "rest"), gb(self.daten_platz)));
            let inhalt = match &t.quelle {
                Some(q) => format!("{} {q}", sp.t("Kopie von", "copy of")),
                None => sp.t("neu, FAT32", "new, FAT32").into(),
            };
            s.push_str(&format!("  {}  {:<14} {:>16}   {}\n", i + 1, t.name, groesse, inhalt));
        }
        s
    }
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::platten::proben::stick_und_platte;
    use crate::platten::quelle;

    fn plan() -> Plan {
        let g = stick_und_platte();
        let q = quelle(&g, "vda").unwrap();
        entwerfen(&q, &g[5], &g).unwrap()
    }

    #[test]
    fn sfdisk_eingabe_steht_fest() {
        assert_eq!(
            plan().sfdisk_eingabe(),
            "label: gpt\nunit: sectors\n\n\
             size=131072, type=C12A7328-F81F-11D2-BA4B-00A0C93EC93B, name=\"boot\"\n\
             size=819200, type=0FC63DAF-8483-4772-8E79-3D69D8477DE4, name=\"golem-wurzelA\"\n\
             size=819200, type=0FC63DAF-8483-4772-8E79-3D69D8477DE4, name=\"golem-wurzelB\"\n\
             type=EBD0A0A2-B9E5-4433-87C0-68B6B72699C7, name=\"golem-daten\"\n"
        );
    }

    #[test]
    fn datenplatz_ist_der_rest() {
        assert_eq!(plan().daten_platz, (4u64 << 30) - (864 << 20) - RESERVE);
    }

    /// ⚑ Laeuft das System von A, wird aus B kopiert, und umgekehrt.
    #[test]
    fn kopiert_die_ruhende_wurzel() {
        let mut g = stick_und_platte();
        g[3].einhaengung = "/".into(); // B laeuft
        let q = quelle(&g, "vda").unwrap();
        let p = entwerfen(&q, &g[5], &g).unwrap();
        assert_eq!(p.teile[1].quelle.as_deref(), Some("/dev/vda2"));
        assert_eq!(p.teile[2].quelle.as_deref(), Some("/dev/vda2"));
        // Ohne laufende Wurzel (Selbstprobe) nimmt er B.
        assert_eq!(plan().teile[1].quelle.as_deref(), Some("/dev/vda3"));
    }

    /// ⛔️ Der getippte Name der Startplatte scheitert im Plan selbst.
    #[test]
    fn startplatte_als_ziel_scheitert() {
        let g = stick_und_platte();
        let q = quelle(&g, "vda").unwrap();
        let f = entwerfen(&q, &g[0], &g).unwrap_err();
        assert!(f.contains("laeuft GolemOS gerade"), "{f}");
    }

    #[test]
    fn zu_kleines_ziel_scheitert() {
        let mut g = stick_und_platte();
        g[5].groesse = 1 << 30;
        let q = quelle(&g, "vda").unwrap();
        assert!(entwerfen(&q, &g[5], &g).unwrap_err().contains("zu klein"));
    }

    #[test]
    fn partition_als_ziel_scheitert() {
        let g = stick_und_platte();
        let q = quelle(&g, "vda").unwrap();
        assert!(entwerfen(&q, &g[4], &g).unwrap_err().contains("keine Platte"));
    }

    #[test]
    fn beschreibung_nennt_ziel_und_quellen() {
        let b = plan().beschreiben();
        assert!(b.contains("/dev/vdb (4,3 GB)"), "{b}");
        assert!(b.contains("Kopie von /dev/vda1"), "{b}");
        assert!(b.contains("neu, FAT32"), "{b}");
    }
}
