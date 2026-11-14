//! Einen Plan ausfuehren. Ab hier wird geschrieben.
//!
//! ⛔️ **Nichts hier entscheidet etwas.** Welche Platte, welche
//! Groessen, welche Quellen: Das steht im `Plan`, und der ist geprueft
//! und bestaetigt, bevor diese Datei ihn sieht. Was hier steht, ist
//! Handwerk, und jeder Schritt meldet sich, bevor er beginnt.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::geraete::{self, gb, Geraet};
use crate::grub;
use crate::plan::Plan;
use crate::platten::{NAME_DATEN, NAME_WURZEL_A, NAME_WURZEL_B};

/// Wo die Partitionen der Zielplatte waehrend der Installation haengen.
const EINHANG_EFI: &str = "/run/golemos/ziel-efi";
const EINHANG_DATEN: &str = "/run/golemos/ziel-daten";
const EINHANG_QUELLE_EFI: &str = "/run/golemos/quelle-efi";

/// Der Name, unter dem die Datenpartition an jedem Rechner erscheint.
///
/// ⚑ **„Golem", auf Wunsch des Projektinhabers**, vorher `GOLEM-DATEN`.
/// Kleinbuchstaben in einem FAT-Namen meldet `mkfs.fat` als Warnung
/// („might not work properly on some systems"); gemeint sind alte
/// DOS-Werkzeuge. macOS zeigt `/Volumes/Golem` (gemessen am 2026-09-25).
/// ⚠️ Derselbe Name steht in `overlay/etc/init.d/S05daten`; die Probe
/// `name_wie_im_startskript` haelt beide gleich.
pub const DATEN_NAME: &str = "Golem";

/// Die Marke, an der `S05daten` erkennt, dass nichts mehr zu dehnen ist.
///
/// ⛔️ **Ohne sie formatierte der erste Start die Datenpartition neu**,
/// und alles, was die Installation daraufkopiert hat, waere weg.
const GEDEHNT: &str = ".gedehnt";

/// Fuehrt `plan` aus. `daten_von` ist der Ordner, dessen ganzer Inhalt
/// auf die neue Datenpartition kommt; `klon` der Myelith-Ordner, der in
/// jedem Fall mitkommt.
pub fn installieren(
    plan: &Plan,
    daten_von: Option<&Path>,
    klon: Option<&Path>,
    aus: &mut dyn Write,
) -> io::Result<()> {
    let ziel = format!("/dev/{}", plan.ziel);
    // ⛔️ **Nicht mitten in einer Aktualisierung.** Die ruhende Seite waere
    //    dann die eben eingespielte, ungepruefte, oder eine, deren Probe
    //    nicht hochkam; genau sie kaeme auf beide Seiten der Platte.
    if Path::new(crate::start::DATEN).join(ERWARTET).exists() {
        return Err(io::Error::other(
            "eine Aktualisierung ist nicht bestaetigt (golem-aktualisieren status); erst neu starten oder bestaetigen",
        ));
    }

    // ⛔️ **Und nur, wenn beide Seiten der Quelle denselben Stand tragen**;
    //    nach einer Probe, ob gelungen oder nicht, ist die ruhende eine andere.
    let stand = {
        let q = plan.teile[0].quelle.as_deref().ok_or_else(|| io::Error::other("keine Bootpartition der Quelle"))?;
        let efi = Eingehaengt::neu(q, EINHANG_QUELLE_EFI, "vfat", "ro")?;
        let umg = fs::read_to_string(efi.ort.join("EFI/BOOT/grubenv")).unwrap_or_default();
        grub::gemeinsamer_stand(&umg).map_err(io::Error::other)?
    };

    schritt(aus, &format!("Partitionstabelle auf {ziel} schreiben"))?;
    mit_eingabe(
        "sfdisk",
        &["--wipe", "always", "--wipe-partitions", "always", "-q", &ziel],
        &plan.sfdisk_eingabe(),
    )?;
    // ⚠️ `sfdisk` bittet den Kern selbst um das Neulesen. `partx` ist
    //    der zweite Anlauf fuer den Fall, dass es abgelehnt wurde; sein
    //    Fehler ist deshalb kein Fehler der Installation.
    let _ = Command::new("partx").args(["-u", &ziel]).stdout(Stdio::null()).stderr(Stdio::null()).status();
    let teile = warten_auf_teile(plan)?;

    for (t, g) in plan.teile.iter().zip(&teile) {
        if let (Some(q), Some(bytes)) = (&t.quelle, t.groesse) {
            schritt(aus, &format!("{} nach {} kopieren ({})", q, g.pfad(), gb(bytes)))?;
            kopieren(Path::new(q), Path::new(&g.pfad()), bytes, aus)?;
        }
    }

    let daten = &teile[3];
    // ⚑ **exFAT** (GolemOS 0.6.1): Windows und macOS lesen und schreiben es
    //   von Haus aus, und eine Datei darf groesser als 4 GB sein.
    schritt(aus, &format!("Datenpartition {} anlegen (exFAT)", daten.pfad()))?;
    befehl("mkfs.exfat", &["-L", DATEN_NAME, &daten.pfad()])?;

    schritt(aus, "Start auf diese Platte festnageln (PARTUUID)")?;
    {
        let efi = Eingehaengt::neu(&teile[0].pfad(), EINHANG_EFI, "vfat", "rw")?;
        let cfg = efi.ort.join("EFI/BOOT/grub.cfg");
        let alt = fs::read_to_string(&cfg)?;
        let platte = grub::Kennungen {
            a: teile[1].partuuid.clone(),
            b: teile[2].partuuid.clone(),
            daten: daten.partuuid.clone(),
        };
        let neu = grub::festnageln(&alt, &plan.alt, &platte).map_err(io::Error::other)?;
        fs::write(&cfg, neu)?;
        seiten_angleichen(&efi.ort, plan.seite, stand.as_deref())?;
    }

    {
        let d = Eingehaengt::neu(&daten.pfad(), EINHANG_DATEN, "exfat", "rw")?;
        fs::write(d.ort.join(GEDEHNT), "")?;
        if let Some(von) = daten_von {
            let gesamt = crate::klon::belegung(von);
            schritt(aus, &format!("Daten mitnehmen ({})", gb(gesamt)))?;
            let mut fort = Fortschritt::neu(gesamt);
            baum_kopieren(von, &d.ort, &mut fort, aus)?;
        }
        // ⛔️ **Der Myelith-Ordner kommt immer mit**, auch wenn der Rest
        //    der Daten bleibt oder er auf einem anderen Laufwerk lag.
        //    Liegt er unter dem, was eben kopiert wurde, ist er schon da.
        if let Some(k) = klon {
            if !daten_von.is_some_and(|v| k.starts_with(v)) {
                let ziel = d.ort.join(klon_ziel(k));
                let gesamt = crate::klon::belegung(k);
                schritt(aus, &format!("Myelith-Ordner mitnehmen ({})", gb(gesamt)))?;
                let mut fort = Fortschritt::neu(gesamt);
                baum_kopieren(k, &ziel, &mut fort, aus)?;
            }
        }
    }
    befehl("sync", &[])?;
    schritt(aus, "fertig")?;
    Ok(())
}

/// Wohin der Myelith-Ordner auf der neuen Datenpartition kommt.
///
/// ⚑ Lag er auf der Datenpartition, an denselben Ort (`/daten/Myelith`
/// bleibt `Myelith`); lag er auf einem anderen Laufwerk, unter seinem
/// Namen. Beides findet die Suche beim naechsten Start.
fn klon_ziel(k: &Path) -> PathBuf {
    match k.strip_prefix(crate::start::DATEN) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.to_path_buf(),
        _ => PathBuf::from(k.file_name().unwrap_or_else(|| std::ffi::OsStr::new("Myelith"))),
    }
}

fn schritt(aus: &mut dyn Write, text: &str) -> io::Result<()> {
    writeln!(aus, "── {text}")?;
    aus.flush()
}

fn befehl(prog: &str, args: &[&str]) -> io::Result<()> {
    let aus = Command::new(prog).args(args).output()?;
    if aus.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "{prog} {} endete mit {}: {}",
        args.join(" "),
        aus.status,
        String::from_utf8_lossy(&aus.stderr).trim()
    )))
}

fn mit_eingabe(prog: &str, args: &[&str], eingabe: &str) -> io::Result<()> {
    let mut kind = Command::new(prog)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    kind.stdin.take().expect("stdin").write_all(eingabe.as_bytes())?;
    let aus = kind.wait_with_output()?;
    if aus.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "{prog} endete mit {}: {}",
        aus.status,
        String::from_utf8_lossy(&aus.stderr).trim()
    )))
}

/// Wartet, bis der Kern die neuen Partitionen samt Kennungen zeigt, und
/// liefert sie in der Reihenfolge des Plans.
///
/// ⚠️ **Gesucht wird unter der Zielplatte**, nicht nach Namen allein: Der
/// Stick traegt dieselben Namen, und `lsblk` listet ihn zuerst.
fn warten_auf_teile(plan: &Plan) -> io::Result<Vec<Geraet>> {
    let start = Instant::now();
    loop {
        let g = geraete::lesen()?;
        let gefunden: Vec<Option<Geraet>> = plan
            .teile
            .iter()
            .map(|t| {
                g.iter()
                    .find(|x| x.ist_partition() && x.eltern == plan.ziel && x.partname == t.name)
                    .filter(|x| !x.partuuid.is_empty())
                    .cloned()
            })
            .collect();
        if gefunden.iter().all(Option::is_some) {
            let teile: Vec<Geraet> = gefunden.into_iter().flatten().collect();
            debug_assert_eq!(teile[1].partname, NAME_WURZEL_A);
            debug_assert_eq!(teile[2].partname, NAME_WURZEL_B);
            debug_assert_eq!(teile[3].partname, NAME_DATEN);
            return Ok(teile);
        }
        if start.elapsed() > Duration::from_secs(15) {
            return Err(io::Error::other(format!(
                "nach 15 s zeigt der Kern die neuen Partitionen auf /dev/{} nicht",
                plan.ziel
            )));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Meldet in Zehnteln, damit eine lange Kopie nicht stumm aussieht.
struct Fortschritt {
    gesamt: u64,
    bisher: u64,
    gemeldet: u64,
}

impl Fortschritt {
    fn neu(gesamt: u64) -> Self {
        Fortschritt { gesamt, bisher: 0, gemeldet: 0 }
    }

    fn weiter(&mut self, n: u64, aus: &mut dyn Write) -> io::Result<()> {
        self.bisher += n;
        if self.gesamt == 0 {
            return Ok(());
        }
        let zehntel = self.bisher * 10 / self.gesamt;
        if zehntel > self.gemeldet {
            self.gemeldet = zehntel;
            writeln!(aus, "   {:>3} %", zehntel * 10)?;
            aus.flush()?;
        }
        Ok(())
    }
}

/// Kopiert genau `bytes` Bytes von einem Geraet auf ein anderes.
///
/// ⚑ **Genau so viele, nicht „bis zum Ende".** Die Zielpartition ist
/// gleich gross angelegt; liest die Quelle weniger, ist etwas faul, und
/// das soll ein Fehler sein und keine halbe Wurzel.
fn kopieren(von: &Path, nach: &Path, bytes: u64, aus: &mut dyn Write) -> io::Result<()> {
    let mut q = File::open(von)?;
    let mut z = OpenOptions::new().write(true).open(nach)?;
    let mut puffer = vec![0u8; 4 << 20];
    let mut fort = Fortschritt::neu(bytes);
    let mut rest = bytes;
    while rest > 0 {
        let stueck = puffer.len().min(usize::try_from(rest).unwrap_or(usize::MAX));
        let n = q.read(&mut puffer[..stueck])?;
        if n == 0 {
            return Err(io::Error::other(format!(
                "{} endete {} vor der erwarteten Groesse",
                von.display(),
                gb(rest)
            )));
        }
        z.write_all(&puffer[..n])?;
        rest -= n as u64;
        fort.weiter(n as u64, aus)?;
    }
    z.sync_all()
}

/// Kopiert einen Verzeichnisbaum auf FAT.
///
/// ⚠️ **Ohne Rechte und ohne Verweise.** `fs::copy` versucht die Rechte
/// mitzunehmen, und FAT lehnt das ab; ein symbolischer Verweis laesst
/// sich dort gar nicht anlegen. Also Inhalt allein, und Verweise werden
/// uebergangen (ein Klon enthaelt keine).
fn baum_kopieren(von: &Path, nach: &Path, fort: &mut Fortschritt, aus: &mut dyn Write) -> io::Result<()> {
    fs::create_dir_all(nach)?;
    let mut eintraege: Vec<_> = fs::read_dir(von)?.filter_map(|e| e.ok()).collect();
    eintraege.sort_by_key(|e| e.file_name());
    for e in eintraege {
        let name = e.file_name();
        if name == GEDEHNT {
            continue;
        }
        let art = e.file_type()?;
        let ziel = nach.join(&name);
        if art.is_dir() {
            baum_kopieren(&e.path(), &ziel, fort, aus)?;
        } else if art.is_file() {
            let n = io::copy(&mut File::open(e.path())?, &mut File::create(&ziel)?)?;
            fort.weiter(n, aus)?;
        }
    }
    Ok(())
}

/// Die Marke von `golem-aktualisieren`, unter der Datenpartition: Eine
/// eingespielte Seite wartet auf ihre Bestaetigung.
const ERWARTET: &str = "golemos/aktualisierung.erwartet";

/// Die Kerne, die GolemOS je Seite auf die Bootpartition legt.
const KERNE: [&str; 2] = ["bzImage", "Image"];

/// Beide Seiten der Platte tragen die Wurzel der Quellseite `seite`; hier
/// bekommen sie auch deren Kern, und `grubenv` setzt A als geltend, beide
/// mit dem `stand` der Quelle.
///
/// ⛔️ **Ohne das passten nach einer Aktualisierung Wurzel und Kern nicht
/// mehr zusammen**: Kopiert wird die Bootpartition, wie sie ist, mit dem
/// Kern jeder Seite, die Wurzel aber nur von einer.
///
/// ⚠️ Aeltere Sticks haben einen Kern fuer beide und kein `grubenv`; dann
/// gibt es nichts anzugleichen.
fn seiten_angleichen(efi: &Path, seite: char, stand: Option<&str>) -> io::Result<()> {
    let andere = if seite == 'A' { 'B' } else { 'A' };
    for k in KERNE {
        let von = efi.join(format!("{k}-{seite}"));
        if von.exists() {
            let neu = efi.join(format!("{k}-{andere}.neu"));
            fs::copy(&von, &neu)?;
            fs::rename(&neu, efi.join(format!("{k}-{andere}")))?;
        }
    }
    let umg = efi.join("EFI/BOOT/grubenv");
    if let (true, Some(stand)) = (umg.exists(), stand) {
        // An Ort und Stelle wie `golem-aktualisieren`, ohne zu kuerzen.
        let mut f = OpenOptions::new().write(true).open(&umg)?;
        f.write_all(&grub::umgebung('A', stand))?;
        f.sync_all()?;
    }
    Ok(())
}

/// Eine eingehaengte Partition, die beim Verlassen wieder ausgehaengt wird,
/// auch wenn dazwischen ein Fehler auftrat.
struct Eingehaengt {
    ort: PathBuf,
}

impl Eingehaengt {
    /// ⚠️ `flush` kennt nur `vfat`; `exfat` lehnt die Option ab.
    fn neu(geraet: &str, ort: &str, typ: &str, art: &str) -> io::Result<Self> {
        fs::create_dir_all(ort)?;
        let optionen = match (typ, art) {
            (_, "ro") => "ro",
            ("vfat", _) => "rw,flush",
            _ => "rw",
        };
        befehl("mount", &["-t", typ, "-o", optionen, geraet, ort])?;
        Ok(Eingehaengt { ort: PathBuf::from(ort) })
    }
}

impl Drop for Eingehaengt {
    fn drop(&mut self) {
        let _ = Command::new("umount").arg(&self.ort).status();
    }
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::klon::proben::ordner;

    /// ⚑ Von B kopiert: Beide Seiten bekommen den Kern von B, und A gilt.
    #[test]
    fn die_seiten_bekommen_den_kern_ihrer_wurzel() {
        let o = ordner("angleichen");
        fs::create_dir_all(o.join("EFI/BOOT")).unwrap();
        fs::write(o.join("Image-A"), "alt").unwrap();
        fs::write(o.join("Image-B"), "neu").unwrap();
        fs::write(o.join("EFI/BOOT/grubenv"), grub::umgebung('B', "neu")).unwrap();
        seiten_angleichen(&o, 'B', Some("neu")).unwrap();
        assert_eq!(fs::read_to_string(o.join("Image-A")).unwrap(), "neu");
        assert_eq!(fs::read_to_string(o.join("Image-B")).unwrap(), "neu");
        assert!(!o.join("Image-A.neu").exists());
        assert_eq!(fs::read(o.join("EFI/BOOT/grubenv")).unwrap(), grub::umgebung('A', "neu"));
    }

    /// Ein aelterer Stick ohne Kerne je Seite und ohne `grubenv` bleibt,
    /// wie er ist.
    #[test]
    fn ein_aelterer_stick_bleibt_wie_er_ist() {
        let o = ordner("angleichen-alt");
        fs::write(o.join("Image"), "einer").unwrap();
        seiten_angleichen(&o, 'A', None).unwrap();
        assert_eq!(fs::read_to_string(o.join("Image")).unwrap(), "einer");
        assert!(!o.join("EFI").exists() && !o.join("Image-B").exists());
    }

    /// ⛔️ **Die Bindung an das Startskript.** Formatiert der Assistent
    /// unter einem anderen Namen als `S05daten`, heisst die Partition
    /// nach einer Installation anders als auf dem Stick, und jede
    /// Anleitung stimmt nur fuer einen der beiden.
    #[test]
    fn name_wie_im_startskript() {
        let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../overlay/etc/init.d/S05daten");
        let text = fs::read_to_string(pfad).expect("S05daten");
        assert!(text.contains(&format!("NAME={DATEN_NAME}\n")), "S05daten nennt einen anderen Namen");
    }

    #[test]
    fn ordner_behaelt_seinen_ort_oder_seinen_namen() {
        assert_eq!(klon_ziel(Path::new("/daten/Myelith")), PathBuf::from("Myelith"));
        assert_eq!(klon_ziel(Path::new("/daten/Projekte/Myelith")), PathBuf::from("Projekte/Myelith"));
        assert_eq!(klon_ziel(Path::new("/medien/sdb1/Repository")), PathBuf::from("Repository"));
    }

    #[test]
    fn kopiert_genau_die_verlangten_bytes() {
        let o = ordner("kopie");
        fs::write(o.join("quelle"), vec![7u8; 10_000]).unwrap();
        fs::write(o.join("ziel"), vec![0u8; 10_000]).unwrap();
        let mut aus = Vec::new();
        kopieren(&o.join("quelle"), &o.join("ziel"), 6_000, &mut aus).unwrap();
        let z = fs::read(o.join("ziel")).unwrap();
        assert!(z[..6_000].iter().all(|&b| b == 7));
        assert!(z[6_000..].iter().all(|&b| b == 0), "dahinter bleibt es unberuehrt");
        assert!(String::from_utf8(aus).unwrap().contains("100 %"));
    }

    /// ⛔️ Die Gegenprobe: Eine zu kurze Quelle ist ein Fehler.
    #[test]
    fn zu_kurze_quelle_ist_ein_fehler() {
        let o = ordner("kurz");
        fs::write(o.join("quelle"), vec![7u8; 100]).unwrap();
        fs::write(o.join("ziel"), vec![0u8; 1_000]).unwrap();
        let f = kopieren(&o.join("quelle"), &o.join("ziel"), 1_000, &mut Vec::new()).unwrap_err();
        assert!(f.to_string().contains("vor der erwarteten Groesse"), "{f}");
    }

    #[test]
    fn baum_ohne_marke_und_mit_inhalt() {
        let o = ordner("baum");
        let von = o.join("von");
        fs::create_dir_all(von.join("Myelith/INTEGER_LLM")).unwrap();
        fs::write(von.join("Myelith/INTEGER_LLM/a.bin"), vec![1u8; 300]).unwrap();
        fs::write(von.join(GEDEHNT), "").unwrap();
        let nach = o.join("nach");
        let mut fort = Fortschritt::neu(300);
        let mut aus = Vec::new();
        baum_kopieren(&von, &nach, &mut fort, &mut aus).unwrap();
        assert_eq!(fs::read(nach.join("Myelith/INTEGER_LLM/a.bin")).unwrap().len(), 300);
        assert!(!nach.join(GEDEHNT).exists(), "die Marke schreibt die Installation selbst");
        assert!(String::from_utf8(aus).unwrap().contains("100 %"));
    }
}
