//! Einen Myelith-Klon auf den Laufwerken finden, und darin die Modelle.
//!
//! ⚑ **Der Mensch zieht seinen Myelith-Ordner auf den Stick, und GolemOS
//! bedient sich daran.** Dafuer muss nichts umgeschrieben und nichts
//! gesammelt werden: Der Client kennt `MYELITH_WURZEL` schon und loest
//! relative Artefaktpfade gegen den Klon auf. Diese Kiste muss den Klon
//! nur finden und den Ort nennen.
//!
//! ⚠️ **Dieselbe Marke wie im Client**, und sie steht hier ein zweites
//! Mal, weil diese Kiste ohne Fremdkisten auskommt. Damit die beiden
//! nicht auseinanderlaufen, liest `marke_wie_im_client` die Stelle im
//! Client und vergleicht.

use std::fs;
use std::path::{Path, PathBuf};

/// Die Datei, an der ein Klon erkannt wird (wie `myl_senses::ort::MARKE`).
pub const MARKE: &str = "INTEGER_LLM/scripts/build_artifacts.sh";

/// Die Umgebungsvariable, die den Klon nennt (wie `myl_senses::ort::UMGEBUNG`).
pub const UMGEBUNG: &str = "MYELITH_WURZEL";

/// Was ein Klon enthalten muss, damit GolemOS mit ihm alles kann.
///
/// ⛔️ **Der ganze Ordner kommt mit, nicht eine Auswahl** (Festlegung des
/// Projektinhabers, 2026-09-25). Nur dann kann GolemOS vom Stick und vom
/// installierten System aus selbst neue Sticks schreiben (`spread.sh`
/// mit den freigegebenen Abbildern) und hat den Quelltext bei sich. Die
/// Marke allein erkennt einen Klon; diese Liste sagt, ob er vollstaendig
/// ist. Stichproben, keine Inventur: je eine Datei oder ein Ordner fuer
/// das, was ein Teilkopierer am ehesten weglaesst.
pub const VOLLSTAENDIG: [&str; 6] = [
    "SYSTEM/golemos/spread.sh",
    "SYSTEM/golemos/abbild/fertig",
    "SYSTEM/crates-vorrat",
    "CLIENT/werkzeugkisten",
    "INTEGER_LLM/runtime/src",
    "COMPLIANCE",
];

/// Was von `VOLLSTAENDIG` im Klon fehlt.
pub fn fehlt(klon: &Path) -> Vec<&'static str> {
    VOLLSTAENDIG.iter().copied().filter(|p| !klon.join(p).exists()).collect()
}

/// Wo im Klon die Artefakte liegen.
pub const ARTEFAKTE_IM_KLON: &str = "INTEGER_LLM/artifacts";

/// Wie tief unter einem Laufwerk gesucht wird.
///
/// ⚑ **Zwei Ebenen**: `/daten/Myelith` und `/daten/Projekte/Myelith`.
/// Tiefer wird die Suche auf einem vollen Laufwerk langsam, und wer
/// seinen Klon tiefer ablegt, nennt ihn mit `MYELITH_WURZEL`.
pub const TIEFE: usize = 2;

/// Verzeichnisse, die kein Mensch fuer seinen Klon anlegt.
///
/// Die Papierkoerbe und Verwaltungsordner, die macOS und Windows auf
/// jedem FAT-Laufwerk hinterlassen, dazu alles mit Punkt vorn.
fn uebergehen(name: &str) -> bool {
    name.starts_with('.') || name == "System Volume Information" || name == "$RECYCLE.BIN"
}

/// Der erste Klon unter `orte`, in fester Reihenfolge.
pub fn finden(orte: &[PathBuf]) -> Option<PathBuf> {
    orte.iter().find_map(|o| unter(o, TIEFE))
}

fn unter(ort: &Path, tiefe: usize) -> Option<PathBuf> {
    if ort.join(MARKE).is_file() {
        return Some(ort.to_path_buf());
    }
    if tiefe == 0 {
        return None;
    }
    // ⚑ Sortiert, damit zwei Starts mit denselben Laufwerken denselben
    //   Klon waehlen. `read_dir` verspricht keine Reihenfolge.
    let mut kinder: Vec<PathBuf> = fs::read_dir(ort)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| !uebergehen(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect();
    kinder.sort();
    kinder.iter().find_map(|k| unter(k, tiefe - 1))
}

/// Ein Modell, so wie der Client es laedt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artefakt {
    pub name: String,
    pub pfad: PathBuf,
    pub groesse: u64,
}

/// Ein Verzeichnis ist ein Artefakt, wenn es `model_config.json` traegt.
pub fn ist_artefakt(p: &Path) -> bool {
    p.join("model_config.json").is_file()
}

/// Alle Artefakte im Klon und unter `weitere` (etwa `/daten/artefakte`).
pub fn artefakte(klon: Option<&Path>, weitere: &[PathBuf]) -> Vec<Artefakt> {
    let mut orte: Vec<PathBuf> = klon.map(|k| vec![k.join(ARTEFAKTE_IM_KLON)]).unwrap_or_default();
    orte.extend(weitere.iter().cloned());
    let mut aus = Vec::new();
    for ort in orte {
        let Ok(eintraege) = fs::read_dir(&ort) else { continue };
        let mut gefunden: Vec<PathBuf> = eintraege
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| ist_artefakt(p))
            .collect();
        gefunden.sort();
        for p in gefunden {
            aus.push(Artefakt {
                name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                groesse: belegung(&p),
                pfad: p,
            });
        }
    }
    aus
}

/// Was ein Verzeichnis mit allem darunter belegt.
pub fn belegung(p: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(p) else { return 0 };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    fs::read_dir(p)
        .map(|es| es.filter_map(|e| e.ok()).map(|e| belegung(&e.path())).sum())
        .unwrap_or(0)
}

/// Welches Modell GolemOS ohne Einstellungen nimmt.
///
/// ⚑ **Das groesste, das in sechs Zehntel des Arbeitsspeichers passt.**
/// Ein Artefakt wird eingeblendet und nicht ganz gelesen, groesser ginge
/// also. Aber GolemOS hat keine Auslagerung, und ein Modell, das den
/// Speicher fuellt, laesst fuer den Rest des Systems nichts. Passt
/// keines, nimmt es das kleinste: Ein langsames Modell ist besser als
/// eine Meldung, dass keines geht.
pub fn waehlen(liste: &[Artefakt], speicher: u64) -> Option<&Artefakt> {
    let grenze = speicher / 10 * 6;
    liste
        .iter()
        .filter(|a| a.groesse <= grenze)
        .max_by_key(|a| a.groesse)
        .or_else(|| liste.iter().min_by_key(|a| a.groesse))
}

/// Der Pfad, der in die Einstellungen kommt.
///
/// ⚑ **Relativ, wenn das Artefakt im Klon liegt.** Ein Laufwerk wird bei
/// jedem Start neu eingehaengt und nicht immer am selben Ort; ein
/// relativer Pfad loest der Client jedes Mal gegen den gefundenen Klon
/// auf, ein absoluter zeigte nach einem Umstecken ins Leere.
pub fn einstellpfad(a: &Artefakt, klon: Option<&Path>) -> String {
    if let Some(k) = klon {
        if let Ok(rel) = a.pfad.strip_prefix(k) {
            return rel.to_string_lossy().into_owned();
        }
    }
    a.pfad.to_string_lossy().into_owned()
}

/// Der Arbeitsspeicher laut `/proc/meminfo`, in Bytes.
pub fn arbeitsspeicher() -> u64 {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|z| z.starts_with("MemTotal:"))
                .and_then(|z| z.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<u64>().ok())
        })
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

#[cfg(test)]
pub(crate) mod proben {
    use super::*;

    /// Ein frisches Verzeichnis je Probe, damit parallele Proben sich
    /// nicht in die Quere kommen.
    pub fn ordner(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("golem-einrichten-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    pub fn scheinklon(ort: &Path) {
        let m = ort.join(MARKE);
        fs::create_dir_all(m.parent().unwrap()).unwrap();
        fs::write(m, "#!/bin/sh\n").unwrap();
    }

    pub fn scheinartefakt(ort: &Path, name: &str, bytes: usize) -> PathBuf {
        let p = ort.join(name);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("model_config.json"), "{}").unwrap();
        fs::write(p.join("gewichte.bin"), vec![0u8; bytes]).unwrap();
        p
    }

    #[test]
    fn vollstaendigkeit_nennt_was_fehlt() {
        let o = ordner("vollstaendig");
        scheinklon(&o);
        assert_eq!(fehlt(&o).len(), VOLLSTAENDIG.len(), "nur die Marke: alles fehlt");
        for p in VOLLSTAENDIG {
            fs::create_dir_all(o.join(p)).unwrap();
        }
        assert!(fehlt(&o).is_empty());
        fs::remove_dir_all(o.join("SYSTEM/golemos")).unwrap();
        assert_eq!(fehlt(&o), ["SYSTEM/golemos/spread.sh", "SYSTEM/golemos/abbild/fertig"]);
    }

    /// Die Liste zeigt auf Pfade, die es im Repositorium wirklich gibt;
    /// sonst meldete GolemOS jeden echten Klon als unvollstaendig.
    #[test]
    fn vollstaendigkeit_passt_zum_repositorium() {
        let wurzel = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."));
        assert!(fehlt(wurzel).is_empty(), "{:?}", fehlt(wurzel));
    }

    #[test]
    fn findet_auf_ebene_null_eins_und_zwei() {
        for (i, tief) in ["", "Myelith", "Projekte/Myelith"].iter().enumerate() {
            let o = ordner(&format!("ebene{i}"));
            scheinklon(&o.join(tief));
            assert_eq!(finden(std::slice::from_ref(&o)), Some(o.join(tief)), "Ebene {i}");
        }
    }

    /// Die Gegenprobe zur Tiefe: eine Ebene mehr wird nicht gefunden.
    #[test]
    fn findet_nicht_auf_ebene_drei() {
        let o = ordner("ebene3");
        scheinklon(&o.join("a/b/Myelith"));
        assert_eq!(finden(&[o]), None);
    }

    #[test]
    fn uebergeht_versteckte_und_papierkoerbe() {
        let o = ordner("versteckt");
        scheinklon(&o.join(".Trashes/Myelith"));
        scheinklon(&o.join("$RECYCLE.BIN"));
        assert_eq!(finden(std::slice::from_ref(&o)), None);
        scheinklon(&o.join("Myelith"));
        assert_eq!(finden(std::slice::from_ref(&o)), Some(o.join("Myelith")));
    }

    #[test]
    fn erster_ort_gewinnt_und_die_wahl_ist_fest() {
        let a = ordner("ort-a");
        let b = ordner("ort-b");
        scheinklon(&a.join("zwei"));
        scheinklon(&a.join("eins"));
        scheinklon(&b);
        assert_eq!(finden(&[a.clone(), b.clone()]), Some(a.join("eins")));
        assert_eq!(finden(&[b.clone(), a]), Some(b));
    }

    #[test]
    fn artefakte_aus_klon_und_datenordner() {
        let o = ordner("artefakte");
        scheinklon(&o);
        scheinartefakt(&o.join(ARTEFAKTE_IM_KLON), "myelith-4b", 400);
        fs::write(o.join(ARTEFAKTE_IM_KLON).join("README.md"), "kein Artefakt").unwrap();
        let daten = o.join("daten-artefakte");
        scheinartefakt(&daten, "myelith-0.6b", 100);
        let l = artefakte(Some(&o), &[daten]);
        let namen: Vec<_> = l.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(namen, ["myelith-4b", "myelith-0.6b"]);
        assert_eq!(l[0].groesse, 402, "Gewichte plus model_config.json");
    }

    fn a(name: &str, groesse: u64) -> Artefakt {
        Artefakt { name: name.into(), pfad: PathBuf::from(name), groesse }
    }

    #[test]
    fn waehlt_das_groesste_das_passt() {
        let l = [a("klein", 1), a("mittel", 5), a("gross", 9)];
        assert_eq!(waehlen(&l, 10).map(|x| x.name.as_str()), Some("mittel"));
        assert_eq!(waehlen(&l, 100).map(|x| x.name.as_str()), Some("gross"));
    }

    #[test]
    fn passt_keines_nimmt_es_das_kleinste() {
        let l = [a("gross", 90), a("klein", 50)];
        assert_eq!(waehlen(&l, 10).map(|x| x.name.as_str()), Some("klein"));
        assert_eq!(waehlen(&[], 10), None);
    }

    #[test]
    fn einstellpfad_relativ_im_klon_sonst_absolut() {
        let k = PathBuf::from("/medien/sdb1/Myelith");
        let im = Artefakt { name: "x".into(), pfad: k.join("INTEGER_LLM/artifacts/x"), groesse: 0 };
        assert_eq!(einstellpfad(&im, Some(&k)), "INTEGER_LLM/artifacts/x");
        let aussen = Artefakt { name: "y".into(), pfad: "/daten/artefakte/y".into(), groesse: 0 };
        assert_eq!(einstellpfad(&aussen, Some(&k)), "/daten/artefakte/y");
    }

    /// ⛔️ **Die Bindung an den Client.** Steht dort eine andere Marke
    /// oder Variable, findet GolemOS einen Klon, den `myl` nicht als
    /// solchen erkennt, oder setzt eine Variable, die niemand liest.
    #[test]
    fn marke_wie_im_client() {
        let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../CLIENT/myl-senses/src/ort.rs");
        let text = fs::read_to_string(pfad).expect("CLIENT/myl-senses/src/ort.rs");
        assert!(text.contains(&format!("pub const MARKE: &str = \"{MARKE}\";")), "MARKE weicht ab");
        assert!(text.contains(&format!("pub const UMGEBUNG: &str = \"{UMGEBUNG}\";")), "UMGEBUNG weicht ab");
    }
}
