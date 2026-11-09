//! **Das kleinste abgeschottete Modul**: Vorlage fuer eigene und Pruefgast des
//! Wirts. Auf einen Befehl hin probiert es jede Wirtsfunktion einmal und
//! schreibt, was herauskam.

use myl_module::gast::{Ausgang, Modul};
use myl_module::nachricht::{AnModul, Stil};

#[derive(Default)]
pub struct Beispiel;

impl Modul for Beispiel {
    fn name(&self) -> (&str, &str) {
        ("beispiel", env!("CARGO_PKG_VERSION"))
    }

    fn ereignis(&mut self, e: AnModul, aus: &mut Ausgang) {
        if let AnModul::Befehl { .. } = e {
            proben(aus);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn proben(aus: &mut Ausgang) {
    use myl_module::gast::wirt_ruf as w;
    use myl_module::nachricht::Httpanfrage;
    let datei = w::datei_ablegen("notiz.txt", b"gespeichert")
        .and_then(|_| w::datei_holen("notiz.txt"))
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    aus.zeilen(format!("datei: {}", datei.unwrap_or_else(|f| f)), Stil::Normal);
    let g = w::geheimnis_holen("schluessel").map(|s| format!("{} Bytes", s.len()));
    aus.zeilen(format!("geheimnis: {}", g.unwrap_or_else(|f| f)), Stil::Normal);
    aus.zeilen(format!("uhr: {}", if w::jetzt() > 0 { "ja" } else { "nein" }), Stil::Normal);
    w::protokollieren("Probe gelaufen");
    let a = Httpanfrage { url: "https://example.com/".into(), methode: None, kopf: vec![], koerper: None };
    let n = w::http_anfrage(&a).map(|r| r.status.to_string());
    aus.zeilen(format!("netz: {}", n.unwrap_or_else(|f| f)), Stil::Normal);
}

#[cfg(not(target_arch = "wasm32"))]
fn proben(aus: &mut Ausgang) {
    aus.zeilen("nur unter WebAssembly", Stil::Normal);
}

#[cfg(target_arch = "wasm32")]
myl_module::wasm_modul!(Beispiel, Beispiel);
