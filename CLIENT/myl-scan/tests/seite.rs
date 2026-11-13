//! Eine echte Seite: zweispaltig, mit Abbildung, als Scan verschlechtert.

use myl_scan::{ausfuehren, Erkenner, ABBILDUNG, SCHRIFTEN};

const SEITE: &[u8] = include_bytes!("seite.jpg");

#[test]
fn eine_echte_seite_wird_gelesen() {
    let bild = image::load_from_memory(SEITE).unwrap().to_rgb8();
    let e = Erkenner::neu(None).unwrap();
    let s = e.seite(&bild, false).unwrap();
    let t = &s.markdown;
    assert!(t.contains("## 3.2 Schichtenmodell"), "der Titel fehlt: {t}");
    assert!(t.contains("Abbildung 1: Schichtenmodell"), "{t}");
    assert_eq!(t.matches(ABBILDUNG).count(), s.abbildungen.len());
    assert_eq!(s.abbildungen.len(), 1, "genau die eine Abbildung: {t}");

    // Spalten: Links kommt vor rechts, auch wenn rechts weiter oben steht.
    let links = t.find("Schichtenmodell\n").expect("links");
    let rechts = t.find("DeServe").expect("rechts");
    assert!(links < rechts, "die Spalten sind vertauscht: {t}");

    // Eine Umlaut-Probe; ohne das Lateinmodell fehlen sie.
    assert!(t.contains("können") && t.contains("Gegenstand"), "{t}");

    // Ohne Text bleibt die Abbildung, und es wird nichts gelesen.
    let nur = e.seite(&bild, true).unwrap();
    assert!(nur.markdown.is_empty());
    assert_eq!(nur.abbildungen.len(), 1);
}

#[test]
fn der_text_einer_abbildung() {
    let seite = image::load_from_memory(SEITE).unwrap().to_rgb8();
    let e = Erkenner::neu(None).unwrap();
    let s = e.seite(&seite, true).unwrap();
    let bild = image::load_from_memory(&s.abbildungen[0]).unwrap().to_rgb8();
    let t = e.bildtext(&bild).unwrap().join(" | ");
    assert!(t.contains("Agent Layer") && t.contains("Compute Layer") && t.contains("Networking"), "{t}");
}

fn pdf_mit_bild(jpeg: &[u8], b: u32, h: u32) -> Vec<u8> {
    let inhalt = format!("q {b} 0 0 {h} 0 0 cm /B Do Q");
    let teile: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {b} {h}] /Resources << /XObject << /B 4 0 R >> >> /Contents 5 0 R >>").into_bytes(),
        [
            format!("<< /Type /XObject /Subtype /Image /Width {b} /Height {h} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n", jpeg.len()).into_bytes(),
            jpeg.to_vec(),
            b"\nendstream".to_vec(),
        ]
        .concat(),
        format!("<< /Length {} >>\nstream\n{inhalt}\nendstream", inhalt.len()).into_bytes(),
    ];
    let mut aus = b"%PDF-1.5\n".to_vec();
    let mut orte = Vec::new();
    for (i, t) in teile.iter().enumerate() {
        orte.push(aus.len());
        aus.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        aus.extend_from_slice(t);
        aus.extend_from_slice(b"\nendobj\n");
    }
    let x = aus.len();
    aus.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", teile.len() + 1).as_bytes());
    for o in orte {
        aus.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    aus.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n", teile.len() + 1).as_bytes());
    aus
}

#[test]
fn ein_auftrag_schreibt_je_seite_eine_datei() {
    let ort = std::env::temp_dir().join(format!("myl-scan-probe-{}", std::process::id()));
    std::fs::create_dir_all(&ort).unwrap();
    let bild = image::load_from_memory(SEITE).unwrap();
    let pdf = ort.join("scan.pdf");
    std::fs::write(&pdf, pdf_mit_bild(SEITE, bild.width(), bild.height())).unwrap();
    let auftrag = ort.join("auftrag.json");
    let inhalt = serde_json::json!({ "arbeit": ort, "pdf": pdf, "pakete": [{ "seiten": [1], "ocr": true }] });
    std::fs::write(&auftrag, inhalt.to_string()).unwrap();
    ausfuehren(&auftrag).unwrap();
    let s: serde_json::Value = serde_json::from_slice(&std::fs::read(ort.join("seite-000001.json")).unwrap()).unwrap();
    assert!(s["md"].as_str().unwrap().contains("Schichtenmodell"), "{s}");
    let name = s["bilder"][0].as_str().unwrap();
    assert!(ort.join(name).exists());

    // Eine fehlende Seite ist ein Fehler und keine leere Seite.
    let inhalt = serde_json::json!({ "arbeit": ort, "pdf": pdf, "pakete": [{ "seiten": [9], "ocr": true }] });
    std::fs::write(&auftrag, inhalt.to_string()).unwrap();
    assert!(ausfuehren(&auftrag).unwrap_err().contains("Seite 9"));
    std::fs::remove_dir_all(&ort).unwrap();
}

macro_rules! probe {
    ($name:literal) => {
        ($name, image::load_from_memory(include_bytes!(concat!("schrift-", $name, ".jpg"))).unwrap().to_rgb8())
    };
}

#[test]
fn jede_schrift_wird_erkannt_und_gelesen() {
    let faelle = [
        (probe!("kyrillisch"), "kyrillisch", "Все люди рождаются свободными"),
        (probe!("arabisch"), "arabisch", "يولد جميع الناس"),
        (probe!("griechisch"), "griechisch", "γεννιούνται ελεύθεροι"),
        (probe!("japanisch"), "chinesisch", "すべての人間は"),
        (probe!("thai"), "thai", "เราทุกคนเกิดมา"),
        (probe!("altgriechisch"), "altgriechisch", "ἀρχῇ"),
        (probe!("hebraeisch"), "hebraeisch", "בראשית ברא אלהים"),
        (probe!("bengalisch"), "bengalisch", "সমস্ত মানুষ"),
        (probe!("tibetisch"), "tibetisch", "འགྲོ་བ་མིའི་རིགས"),
        (probe!("syrisch"), "syrisch", "ܬܐܬܐ ܡܠܟܘܬܟ"),
        (probe!("fraktur"), "fraktur", "Unmündigkeit"),
    ];
    for ((name, bild), schrift, satz) in faelle {
        let mut e = Erkenner::neu(None).unwrap();
        e.schrift_waehlen(std::slice::from_ref(&bild)).unwrap();
        assert_eq!(e.schrift(), schrift, "{name}");
        let t = e.bildtext(&bild).unwrap().join(" ");
        assert!(t.contains(satz), "{name}: {t}");
    }
}

#[test]
fn ein_auftrag_bestimmt_die_schrift_selbst_oder_folgt_der_vorgabe() {
    let ort = std::env::temp_dir().join(format!("myl-scan-schrift-{}", std::process::id()));
    std::fs::create_dir_all(&ort).unwrap();
    let jpeg: &[u8] = include_bytes!("schrift-kyrillisch.jpg");
    let bild = image::load_from_memory(jpeg).unwrap();
    let pdf = ort.join("scan.pdf");
    std::fs::write(&pdf, pdf_mit_bild(jpeg, bild.width(), bild.height())).unwrap();
    let auftrag = ort.join("auftrag.json");

    let inhalt = serde_json::json!({ "arbeit": ort, "pdf": pdf, "schrift": "auto", "pakete": [{ "seiten": [1], "ocr": true }] });
    std::fs::write(&auftrag, inhalt.to_string()).unwrap();
    ausfuehren(&auftrag).unwrap();
    assert_eq!(std::fs::read_to_string(ort.join("schrift.txt")).unwrap(), "kyrillisch");
    let s = std::fs::read_to_string(ort.join("seite-000001.json")).unwrap();
    assert!(s.contains("люди"), "{s}");

    let inhalt = serde_json::json!({ "arbeit": ort, "pdf": pdf, "schrift": "latein", "pakete": [{ "seiten": [1], "ocr": true }] });
    std::fs::write(&auftrag, inhalt.to_string()).unwrap();
    ausfuehren(&auftrag).unwrap();
    assert_eq!(std::fs::read_to_string(ort.join("schrift.txt")).unwrap(), "latein", "die Vorgabe gilt");
    std::fs::remove_dir_all(&ort).unwrap();
}

#[test]
fn latein_bleibt_latein() {
    let seite = image::load_from_memory(SEITE).unwrap().to_rgb8();
    let mut e = Erkenner::neu(None).unwrap();
    e.schrift_waehlen(std::slice::from_ref(&seite)).unwrap();
    assert_eq!(e.schrift(), "latein");
}

#[test]
fn eine_unbekannte_schrift_ist_ein_fehler() {
    let f = Erkenner::neu(Some("klingonisch")).err().unwrap();
    assert!(f.contains("klingonisch") && f.contains("latein"), "{f}");
}

#[test]
fn jedes_modell_passt_zu_seiner_zeichenliste() {
    for s in SCHRIFTEN {
        s.pruefen().unwrap_or_else(|f| panic!("{}: {f}", s.name));
    }
}

#[test]
fn senkrechter_satz_laeuft_von_rechts_nach_links() {
    let bild = image::load_from_memory(include_bytes!("senkrecht.jpg")).unwrap().to_rgb8();
    let mut e = Erkenner::neu(None).unwrap();
    e.schrift_waehlen(std::slice::from_ref(&bild)).unwrap();
    assert_eq!(e.schrift(), "chinesisch");
    let t = e.bildtext(&bild).unwrap().join("");
    assert!(t.starts_with("道可道非常道名可名非常名"), "{t}");
    let s = e.seite(&bild, false).unwrap();
    assert!(s.markdown.find("道可道").unwrap() < s.markdown.find("玄之又玄").unwrap(), "{}", s.markdown);
}

