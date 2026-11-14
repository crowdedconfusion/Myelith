//! Die Startkonfiguration der installierten Platte auf sich selbst
//! festnageln.
//!
//! ⛔️ **Warum das noetig ist: Stick und Platte duerfen sich nicht
//! verwechseln.** Die Installation kopiert die EFI-Partition samt ihrer
//! `grub.cfg`, und die nennt die Wurzel und die Daten **des Sticks**.
//! Unveraendert startete die Platte also mit den Partitionen des Sticks,
//! sobald er steckt, und ohne ihn gar nicht.
//!
//! ⚑ **Die Abhilfe ist die PARTUUID**, die `sfdisk` der Platte neu und
//! zufaellig vergibt. Hier werden die Kennungen der Quelle gegen die der
//! Platte getauscht, und jede Kernzeile bekommt die Datenpartition mit
//! (`golemos.daten=PARTUUID=…`), die `S05daten` dann zuerst nimmt.
//!
//! 📌 **Fund 479:** Bis zum 2026-09-25 suchte auch der Stick seine
//! Wurzel ueber den Namen (`PARTLABEL`), und nach einer Installation gab
//! es jeden Namen zweimal. Seither traegt jedes Abbild Kennungen, die
//! beim Bau gewuerfelt werden. Aeltere Sticks nennen noch Namen; die
//! werden hier genauso umgeschrieben.

use crate::platten::{NAME_WURZEL_A, NAME_WURZEL_B};

/// Der Schalter auf der Kernzeile, den `S05daten` liest.
pub const DATEN_SCHALTER: &str = "golemos.daten=PARTUUID=";

/// Die Laenge von `grubenv`, die GRUB verlangt.
pub const UMGEBUNG_LAENGE: usize = 1024;

/// `grubenv` mit `seite` als geltender Seite, ohne Probe und mit `stand` auf
/// beiden Seiten, gefuellt wie `nach-dem-abbild.sh` und
/// `golem-aktualisieren` sie fuellen.
pub fn umgebung(seite: char, stand: &str) -> Vec<u8> {
    let mut u = format!(
        "# GRUB Environment Block\ngolem_seite={seite}\ngolem_probe=\ngolem_stand_A={stand}\ngolem_stand_B={stand}\n"
    )
    .into_bytes();
    u.resize(UMGEBUNG_LAENGE, b'#');
    u
}

/// Der Stand, den beide Seiten laut `grubenv` tragen; `None` bei einem
/// aelteren Stick, der keinen nennt.
///
/// ⛔️ **Tragen die Seiten verschiedene Staende, ist das ein Fehler.**
/// Kopiert wird die ruhende Seite, und nach einer misslungenen Probe ist sie
/// kaputt, nach einer gelungenen die alte; `-` heisst, ihr Schreiben brach
/// ab (`golem-aktualisieren`).
pub fn gemeinsamer_stand(umg: &str) -> Result<Option<String>, String> {
    let wert = |name: &str| {
        umg.lines().find_map(|z| z.strip_prefix(name).and_then(|r| r.strip_prefix('='))).map(str::to_string)
    };
    match (wert("golem_stand_A"), wert("golem_stand_B")) {
        (None, None) => Ok(None),
        // ⚑ Ein frischer Stick traegt B als `leer`, bis sein erster Start es
        //   aus A gefuellt hat (`golem-aktualisieren fuellen`).
        (_, Some(b)) if b == "leer" => Err(
            "die Seite B ist noch leer; GolemOS fuellt sie beim Start. Einmal neu starten und dann installieren".into(),
        ),
        (Some(a), Some(b)) if a == b && !a.is_empty() && a != "-" && a != "leer" => Ok(Some(a)),
        (a, b) => Err(format!(
            "die beiden Seiten tragen verschiedene Staende (A {}, B {}); erst das Paket der laufenden Seite noch einmal einspielen (golem-aktualisieren)",
            a.as_deref().unwrap_or("?"),
            b.as_deref().unwrap_or("?")
        )),
    }
}

/// Die Kennungen einer Platte: beide Wurzeln und die Daten.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kennungen {
    pub a: String,
    pub b: String,
    pub daten: String,
}

/// Schreibt `cfg` von den Kennungen `alt` auf `neu` um.
///
/// ⚠️ **Findet sie eine Wurzel nicht, ist das ein Fehler**, kein leerer
/// Erfolg. Eine `grub.cfg`, die nach der Installation noch auf den Stick
/// zeigt, sieht genauso aus wie eine festgenagelte, bis zu dem Tag, an
/// dem der Stick fehlt oder steckt.
pub fn festnageln(cfg: &str, alt: &Kennungen, neu: &Kennungen) -> Result<String, String> {
    let ersetzungen = [
        (format!("root=PARTUUID={}", alt.a), format!("root=PARTUUID={}", neu.a)),
        (format!("root=PARTUUID={}", alt.b), format!("root=PARTUUID={}", neu.b)),
        (format!("root=PARTLABEL={NAME_WURZEL_A}"), format!("root=PARTUUID={}", neu.a)),
        (format!("root=PARTLABEL={NAME_WURZEL_B}"), format!("root=PARTUUID={}", neu.b)),
    ];
    let ziel_a = format!("root=PARTUUID={}", neu.a);
    let ziel_b = format!("root=PARTUUID={}", neu.b);
    let mut aus = String::with_capacity(cfg.len() + 256);
    let (mut zu_a, mut zu_b) = (0, 0);
    for zeile in cfg.lines() {
        if !zeile.trim_start().starts_with("linux ") {
            aus.push_str(zeile);
            aus.push('\n');
            continue;
        }
        let mut woerter: Vec<String> = Vec::new();
        for w in zeile.split(' ') {
            let mut w = w.to_string();
            if let Some((_, n)) = ersetzungen.iter().find(|(a, _)| *a == w) {
                w = n.clone();
            } else if w.starts_with(DATEN_SCHALTER) {
                // Die Datenpartition gibt es einmal; welche Kennung hier
                // stand, ist gleich, sie wird die der Platte.
                w = format!("{DATEN_SCHALTER}{}", neu.daten);
            }
            woerter.push(w);
        }
        if !woerter.iter().any(|w| w.starts_with(DATEN_SCHALTER)) {
            woerter.push(format!("{DATEN_SCHALTER}{}", neu.daten));
        }
        // ⚑ **Jede Seite startet ihren eigenen Kern** (`…-A`, `…-B`);
        //    aeltere Sticks haben einen fuer beide und kein Suffix.
        let kern = woerter.iter().map(|w| w.trim()).find(|w| !w.is_empty() && *w != "linux").unwrap_or("");
        let kern_seite = kern.strip_suffix("-A").map(|_| 'A').or_else(|| kern.strip_suffix("-B").map(|_| 'B'));
        let seite = if woerter.contains(&ziel_a) {
            zu_a += 1;
            'A'
        } else if woerter.contains(&ziel_b) {
            zu_b += 1;
            'B'
        } else {
            return Err(format!(
                "eine Kernzeile nennt keine der Wurzeln der Quelle ({}, {}): {}",
                alt.a,
                alt.b,
                zeile.trim()
            ));
        };
        if kern_seite.is_some_and(|k| k != seite) {
            return Err(format!("eine Kernzeile startet die Wurzel {seite} mit dem Kern der anderen Seite: {}", zeile.trim()));
        }
        aus.push_str(&woerter.join(" "));
        aus.push('\n');
    }
    if zu_a == 0 || zu_b == 0 {
        return Err(format!("grub.cfg startet nicht beide Wurzeln (A: {zu_a}, B: {zu_b})"));
    }
    Ok(aus)
}

#[cfg(test)]
mod proben {
    use super::*;

    fn k(a: &str, b: &str, d: &str) -> Kennungen {
        Kennungen { a: a.into(), b: b.into(), daten: d.into() }
    }

    /// Die Vorlage aus `abbild/grub.cfg`, gelesen und nicht abgeschrieben,
    /// gefuellt, wie `nach-dem-abbild.sh` sie fuellt.
    fn vorlage() -> String {
        let pfad = concat!(env!("CARGO_MANIFEST_DIR"), "/../abbild/grub.cfg");
        std::fs::read_to_string(pfad)
            .expect("abbild/grub.cfg")
            .replace("KERNNAME", "Image")
            .replace("SERIELL", "ttyAMA0")
            .replace("WURZEL_A_UUID", "stick-a")
            .replace("WURZEL_B_UUID", "stick-b")
            .replace("DATEN_UUID", "stick-d")
    }

    fn kernzeilen(cfg: &str) -> Vec<String> {
        cfg.lines().filter(|z| z.trim_start().starts_with("linux ")).map(String::from).collect()
    }

    #[test]
    fn jede_kernzeile_zeigt_auf_die_platte() {
        let neu = festnageln(&vorlage(), &k("stick-a", "stick-b", "stick-d"), &k("platte-a", "platte-b", "platte-d")).unwrap();
        let z = kernzeilen(&neu);
        assert_eq!(z.len(), 3, "A, B und die Selbstprobe");
        for zeile in &z {
            assert!(!zeile.contains("stick-"), "{zeile}");
            assert_eq!(zeile.matches(DATEN_SCHALTER).count(), 1, "{zeile}");
            assert!(zeile.contains("golemos.daten=PARTUUID=platte-d"), "{zeile}");
        }
        assert!(z[0].contains("root=PARTUUID=platte-a"));
        assert!(z[1].contains("root=PARTUUID=platte-b"));
        assert!(z[2].contains("root=PARTUUID=platte-a") && z[2].contains("golemos.probe"));
        assert!(z[0].contains("linux /Image-A "), "{}", z[0]);
        assert!(z[1].contains("linux /Image-B "), "{}", z[1]);
        assert!(z[2].contains("linux /Image-A "), "{}", z[2]);
    }

    /// ⛔️ Ein Rueckfall auf A mit dem Kern von B waere keiner.
    #[test]
    fn eine_seite_mit_dem_kern_der_anderen_ist_ein_fehler() {
        let cfg = "linux /Image-A root=PARTUUID=a rootwait\n\
                   linux /Image-A root=PARTUUID=b rootwait\n";
        let f = festnageln(cfg, &k("a", "b", "d"), &k("pa", "pb", "pd")).unwrap_err();
        assert!(f.contains("Kern der anderen Seite"), "{f}");
    }

    /// Die Umgebung hat die Form, die `nach-dem-abbild.sh` und
    /// `golem-aktualisieren` schreiben, und die Laenge, die GRUB verlangt.
    #[test]
    fn die_umgebung_gleicht_der_des_abbilds() {
        let u = umgebung('A', "0123abcd");
        assert_eq!(u.len(), UMGEBUNG_LAENGE);
        let lies = |p: &str| std::fs::read_to_string(format!("{}/../{p}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let form = r"# GRUB Environment Block\ngolem_seite=%s\ngolem_probe=%s\ngolem_stand_A=%s\ngolem_stand_B=%s\n";
        assert!(lies("overlay/usr/sbin/golem-aktualisieren").contains(form), "golem-aktualisieren");
        assert!(lies("abbild/nach-dem-abbild.sh").contains(&form.replacen("%s", "A", 1).replacen("%s", "", 1)), "nach-dem-abbild.sh");
        let kopf = b"# GRUB Environment Block\ngolem_seite=A\ngolem_probe=\ngolem_stand_A=0123abcd\ngolem_stand_B=0123abcd\n";
        assert!(u.starts_with(kopf));
        assert!(u[kopf.len()..].iter().all(|&b| b == b'#'));
        assert!(umgebung('B', "x").starts_with(b"# GRUB Environment Block\ngolem_seite=B\n"));
    }

    #[test]
    fn gleiche_staende_gelten() {
        let u = String::from_utf8(umgebung('B', "0123abcd")).unwrap();
        assert_eq!(gemeinsamer_stand(&u), Ok(Some("0123abcd".into())));
        // Ein aelterer Stick nennt keinen Stand.
        assert_eq!(gemeinsamer_stand("# GRUB Environment Block\ngolem_seite=A\n"), Ok(None));
        assert_eq!(gemeinsamer_stand(""), Ok(None));
    }

    /// ⛔️ Verschiedene Staende, ein abgebrochenes Schreiben (`-`) und eine
    /// fehlende Seite halten die Installation auf.
    #[test]
    fn verschiedene_staende_halten_auf() {
        for umg in [
            "golem_stand_A=aaaa\ngolem_stand_B=bbbb\n",
            "golem_stand_A=-\ngolem_stand_B=-\n",
            "golem_stand_A=aaaa\n",
            "golem_stand_A=\ngolem_stand_B=\n",
        ] {
            let f = gemeinsamer_stand(umg).unwrap_err();
            assert!(f.contains("verschiedene Staende"), "{umg}: {f}");
        }
    }

    /// ⛔️ Eine noch leere Seite B haelt die Installation auf, mit eigenem
    /// Grund; sonst kopierte sie eine Partition ohne Inhalt auf die Platte.
    #[test]
    fn eine_leere_seite_b_haelt_auf() {
        for umg in ["golem_stand_A=aaaa\ngolem_stand_B=leer\n", "golem_stand_A=leer\ngolem_stand_B=leer\n"] {
            let f = gemeinsamer_stand(umg).unwrap_err();
            assert!(f.contains("noch leer"), "{umg}: {f}");
        }
        // Und ein `leer` auf A allein gilt auch nicht als gemeinsamer Stand.
        assert!(gemeinsamer_stand("golem_stand_A=leer\ngolem_stand_B=aaaa\n").is_err());
    }

    /// Die Vorlage selbst: Jede Kernzeile nennt eine Kennung und die
    /// Datenpartition, keine mehr einen Namen (Fund 479).
    #[test]
    fn die_vorlage_nennt_kennungen_und_keine_namen() {
        for zeile in kernzeilen(&vorlage()) {
            assert!(!zeile.contains("PARTLABEL"), "{zeile}");
            assert!(zeile.contains("root=PARTUUID=stick-"), "{zeile}");
            assert!(zeile.contains("golemos.daten=PARTUUID=stick-d"), "{zeile}");
        }
    }

    /// Aeltere Sticks nennen ihre Wurzeln beim Namen; auch die werden
    /// festgenagelt, und der Datenschalter kommt dazu.
    #[test]
    fn alte_sticks_mit_namen() {
        let alt = "linux /Image root=PARTLABEL=golem-wurzelA rootwait\nlinux /Image root=PARTLABEL=golem-wurzelB rootwait\n";
        let neu = festnageln(alt, &k("x", "y", "z"), &k("pa", "pb", "pd")).unwrap();
        assert_eq!(
            neu,
            "linux /Image root=PARTUUID=pa rootwait golemos.daten=PARTUUID=pd\n\
             linux /Image root=PARTUUID=pb rootwait golemos.daten=PARTUUID=pd\n"
        );
    }

    /// ⛔️ Die Gegenprobe: Eine Kernzeile mit fremder Kennung ist kein
    /// Erfolg, auch wenn A und B daneben stimmen.
    ///
    /// 📌 Die erste Fassung hatte nur die fremde Zeile und verlangte nur
    /// „irgendein Fehler". Den lieferte dann die Pruefung auf beide
    /// Wurzeln, und die Probe blieb gruen, als diese hier ausgeschaltet
    /// war. Jetzt stehen A und B daneben, und verlangt wird die Meldung.
    #[test]
    fn fremde_kennung_ist_ein_fehler() {
        let cfg = "linux /Image root=PARTUUID=a rootwait\n\
                   linux /Image root=PARTUUID=b rootwait\n\
                   linux /Image root=PARTUUID=irgendwer rootwait\n";
        let f = festnageln(cfg, &k("a", "b", "d"), &k("pa", "pb", "pd")).unwrap_err();
        assert!(f.contains("keine der Wurzeln der Quelle"), "{f}");
    }

    #[test]
    fn ohne_zweite_wurzel_ist_es_ein_fehler() {
        let cfg = "linux /Image root=PARTUUID=a rootwait\n";
        let f = festnageln(cfg, &k("a", "b", "d"), &k("pa", "pb", "pd")).unwrap_err();
        assert!(f.contains("nicht beide Wurzeln"), "{f}");
    }

    /// Zweimal festnageln mit denselben Kennungen aendert nichts mehr.
    #[test]
    fn zweimal_ist_einmal() {
        let alt = k("stick-a", "stick-b", "stick-d");
        let neu = k("pa", "pb", "pd");
        let einmal = festnageln(&vorlage(), &alt, &neu).unwrap();
        assert_eq!(festnageln(&einmal, &neu, &neu).unwrap(), einmal);
    }
}
