//! **Welche Artefakte im Netz rechnen duerfen: eine Liste mit Abdruecken.**
//!
//! # ⚑ Die Regel (Festlegung des Projektinhabers, 2026-10-21)
//!
//! Gebaut und lokal genutzt werden darf jedes Modell, gleich von welchem
//! Anbieter seine Gewichte stammen. **Im Netz rechnet nur, was auf der
//! Netzliste steht, und nur, wenn sein Abdruck dort mit dem uebereinstimmt,
//! was auf der Platte liegt.** Alles andere ist privat.
//!
//! # ⛔️ Warum nicht mehr `betrieb` allein (Fund 585)
//!
//! Bis zum 2026-10-21 entschied das Feld `betrieb` in `model_config.json`,
//! also eine Angabe, die das Artefakt ueber sich selbst macht. Und genau
//! diese Datei deckte kein Hash: `theta_v.json` bindet Gewichtsmanifest,
//! Skalen und Tabellen, nicht die Konfiguration. Ein `"lokal"`, von Hand zu
//! `"netz"` geaendert, brachte ein privates Modell unbemerkt in den
//! Netzbetrieb. Ein Abdruck ueber die Konfiguration schliesst das: Jede
//! Aenderung an ihr aendert den Abdruck, und der steht dann auf keiner Liste.
//!
//! # ⚑ Was der Abdruck deckt
//!
//! SHA-256 ueber eine feste Folge von Zeilen `<datei> <sha256 der datei>`:
//! `theta_v.json`, `model_config.json`, `tokenizer.json` (fehlt sie, steht
//! dort `-`). `theta_v.json` traegt die Hashes von `weights_manifest.json`,
//! `scales.json` und `luts.json`, das Manifest den jedes Tensors und die
//! Tabellen den jeder Tabellendatei, und der Lader prueft beides beim Laden.
//! **Damit deckt der Abdruck das ganze Artefakt**, und er kostet Millisekunden
//! statt eines Durchgangs ueber Gigabyte.
//!
//! # ⚑ Wo die Liste steht
//!
//! Im Modellkatalog `MODELS/llm/KATALOG.json`: Ein freigegebenes Modell
//! traegt dort einen Block `"netz": {"abdruck": "...", "freigegeben": "..."}`.
//! Die Liste wird **beim Bauen eingebunden**: Was ein laufender Knoten
//! zulaesst, ist Teil seines Programms und aendert sich mit einer neuen
//! Fassung, nicht mit einer Datei daneben.
//!
//! # ⚑ Das Pruefnetz: kleine Modelle nur in Testbauten
//!
//! Im Netz rechnet seit dem 2026-10-21 nur das 35B (int8 und int4), als
//! untere Grenze der Leistungsfaehigkeit (Festlegung des Projektinhabers).
//! Die Proben von Pod, Testklient und Klient rechnen den Netzbetrieb aber
//! am 0,6B, denn das 35B passt in keinen Probenlauf. Dafuer steht ein
//! zweiter Block `"pruefnetz"` im Katalog, und er gilt **nur mit dem Merkmal
//! `pruefnetz`**, das die Kisten mit Netzproben allein als
//! Entwicklungsabhaengigkeit setzen. `cargo test` baut ihre Programme damit,
//! `cargo build --release` (und damit jede Installation) nicht: Dort fehlt
//! das 0,6B auf der Liste wie jedes andere private Modell.

use std::path::Path;

/// Die Dateien, die der Abdruck deckt, in dieser Reihenfolge.
pub const ABDRUCK_DATEIEN: [&str; 3] = ["theta_v.json", "model_config.json", "tokenizer.json"];

/// Die Kopfzeile des Abdrucks; eine neue Form bekaeme eine neue Nummer.
pub const ABDRUCK_FORM: &str = "myelith-artefakt-abdruck 1";

/// Der Katalog, wie er beim Bauen im Baum lag.
///
/// ⚑ `include_str!` verlangt ein Literal, daher der relative Pfad von dieser
/// Datei aus zur Wurzel (Abschnitt „Keine harten Pfade“: relativ, nie
/// absolut).
const KATALOG: &str = include_str!("../../../MODELS/llm/KATALOG.json");

/// Ein Eintrag der Netzliste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Netzfreigabe {
    /// Der Schluessel im Katalog, etwa `sparrow-0.6b-int8`.
    pub name: String,
    /// Der Abdruck des freigegebenen Artefakts, 64 Hexzeichen.
    pub abdruck: String,
    /// Wann und von wem freigegeben, wie im Katalog.
    pub freigegeben: String,
    /// Aus dem Block `pruefnetz`: gilt nur in Testbauten.
    pub pruefnetz: bool,
}

/// **Der Abdruck eines Artefakts.**
pub fn abdruck(artefakt: &Path) -> Result<String, String> {
    let mut text = format!("{ABDRUCK_FORM}\n");
    for datei in ABDRUCK_DATEIEN {
        let p = artefakt.join(datei);
        match std::fs::read(&p) {
            Ok(inhalt) => text.push_str(&format!("{datei} {}\n", crate::loader::sha256_hex(&inhalt))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && datei == "tokenizer.json" => {
                text.push_str(&format!("{datei} -\n"))
            }
            Err(e) => return Err(format!("{}: {e}", p.display())),
        }
    }
    Ok(crate::loader::sha256_hex(text.as_bytes()))
}

/// **Die Netzliste aus einem Katalogtext.** Eintraege ohne `netz`-Block
/// fehlen, ebenso die Bloecke `pruefnetz`, ausser `mit_pruefnetz`; ein Block
/// ohne gueltigen Abdruck ist ein Fehler, denn er saehe aus wie eine
/// Freigabe und waere keine.
pub fn netzliste_aus(katalog: &str, mit_pruefnetz: bool) -> Result<Vec<Netzfreigabe>, String> {
    let wurzel: serde_json::Value =
        serde_json::from_str(katalog).map_err(|e| format!("Katalog nicht lesbar: {e}"))?;
    let eintraege = wurzel.as_object().ok_or("Katalog ist kein Objekt")?;
    let mut liste = Vec::new();
    for (name, e) in eintraege {
        for (block, pruefnetz) in [("netz", false), ("pruefnetz", true)] {
            let Some(netz) = e.get(block) else { continue };
            let abdruck = netz.get("abdruck").and_then(|a| a.as_str()).unwrap_or_default();
            if abdruck.len() != 64 || !abdruck.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
                return Err(format!("{name}: {block}.abdruck ist kein SHA-256 in Kleinbuchstaben"));
            }
            if e.get("betrieb").and_then(|b| b.as_str()) == Some("lokal") {
                return Err(format!("{name}: steht als privat im Katalog und zugleich auf der Netzliste"));
            }
            if pruefnetz && !mit_pruefnetz {
                continue;
            }
            liste.push(Netzfreigabe {
                name: name.clone(),
                abdruck: abdruck.to_string(),
                freigegeben: netz.get("freigegeben").and_then(|f| f.as_str()).unwrap_or_default().to_string(),
                pruefnetz,
            });
        }
    }
    Ok(liste)
}

/// **Die Netzliste dieses Programms**, aus dem beim Bauen eingebundenen
/// Katalog; die Bloecke `pruefnetz` nur mit dem Merkmal `pruefnetz`.
pub fn netzliste() -> Vec<Netzfreigabe> {
    // Ein unlesbarer Katalog ist ein Baufehler, den die Probe
    // `der_katalog_ergibt_eine_gueltige_netzliste` faengt; hier gilt dann
    // die leere Liste, und nichts rechnet im Netz.
    netzliste_aus(KATALOG, cfg!(feature = "pruefnetz")).unwrap_or_default()
}

/// **Darf dieses Artefakt im Netz rechnen?** Der passende Eintrag, sonst
/// ein Fehler mit dem gemessenen Abdruck, damit ihn jemand pruefen und
/// eintragen kann.
pub fn pruefen(artefakt: &Path, liste: &[Netzfreigabe]) -> Result<Netzfreigabe, String> {
    let gemessen = abdruck(artefakt)?;
    liste.iter().find(|f| f.abdruck == gemessen).cloned().ok_or_else(|| {
        format!(
            "{} steht nicht auf der Netzliste: Abdruck {gemessen} ist fuer den Netzbetrieb nicht \
             freigegeben. Lokal laesst es sich nutzen; fuer das Netz muss es freigegeben und mit \
             diesem Abdruck im Katalog eingetragen werden",
            artefakt.display()
        )
    })
}

/// **Steht das Artefakt auf der Netzliste dieses Programms?**
pub fn im_netz_zugelassen(artefakt: &Path) -> bool {
    pruefen(artefakt, &netzliste()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artefakt(name: &str, konfig: &str, tokenizer: bool) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("myl-netz-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        std::fs::write(d.join("theta_v.json"), "{\"version\":\"0.22.0\"}").expect("theta_v");
        std::fs::write(d.join("model_config.json"), konfig).expect("model_config");
        if tokenizer {
            std::fs::write(d.join("tokenizer.json"), "{}").expect("tokenizer");
        }
        d
    }

    /// ⚑ **Die Form ist festgehalten.** Derselbe Wert steht in der Probe des
    /// Werkzeugs, das Abdruecke in den Katalog eintraegt
    /// (`INTEGER_LLM/tests/test_netzfreigabe.py`); weichen die beiden ab,
    /// faellt eine der Proben.
    #[test]
    fn der_abdruck_hat_eine_feste_form() {
        let d = artefakt("form", "{\"family\":\"qwen3\"}", true);
        // Unabhaengig nachgerechnet (mit Python aus den drei Dateiinhalten).
        assert_eq!(abdruck(&d).expect("Abdruck"), "5ed8ae39e54827172da0389bea0164058bedfb3ca77de633d6ad1c9c239d6e7e");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// ⛔️ **Fund 585: Eine geaenderte Konfiguration ist ein anderes Artefakt.**
    #[test]
    fn eine_geaenderte_konfiguration_steht_nicht_mehr_auf_der_liste() {
        let d = artefakt("konfig", "{\"family\":\"qwen3\",\"betrieb\":\"lokal\"}", true);
        let liste =
            vec![Netzfreigabe { name: "probe".into(), abdruck: abdruck(&d).unwrap(), freigegeben: "x".into(), pruefnetz: false }];
        assert!(pruefen(&d, &liste).is_ok(), "das eingetragene Artefakt");
        std::fs::write(d.join("model_config.json"), "{\"family\":\"qwen3\",\"betrieb\":\"netz\"}").unwrap();
        let f = pruefen(&d, &liste).expect_err("aus lokal von Hand netz gemacht");
        assert!(f.contains("nicht auf der Netzliste"), "{f}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ohne_tokenizer_ist_der_abdruck_ein_anderer_und_nicht_leer() {
        let mit = artefakt("mit", "{}", true);
        let ohne = artefakt("ohne", "{}", false);
        assert_ne!(abdruck(&mit).unwrap(), abdruck(&ohne).unwrap());
        assert!(abdruck(&mit.join("gibt-es-nicht")).is_err(), "ohne theta_v kein Abdruck");
        let _ = std::fs::remove_dir_all(&mit);
        let _ = std::fs::remove_dir_all(&ohne);
    }

    #[test]
    fn eine_kaputte_freigabe_ist_ein_fehler_und_keine_luecke() {
        assert!(netzliste_aus(r#"{"a": {"netz": {"abdruck": "xyz"}}}"#, false).is_err());
        assert!(netzliste_aus(r#"{"a": {"pruefnetz": {"abdruck": "xyz"}}}"#, false).is_err(), "auch ohne Pruefnetz");
        let gross = "A".repeat(64);
        assert!(netzliste_aus(&format!(r#"{{"a": {{"netz": {{"abdruck": "{gross}"}}}}}}"#), false).is_err());
        let ok = "a".repeat(64);
        assert!(netzliste_aus(&format!(r#"{{"a": {{"betrieb": "lokal", "netz": {{"abdruck": "{ok}"}}}}}}"#), false).is_err());
        assert!(netzliste_aus(&format!(r#"{{"a": {{"betrieb": "lokal", "pruefnetz": {{"abdruck": "{ok}"}}}}}}"#), false).is_err());
        assert_eq!(netzliste_aus(&format!(r#"{{"_hinweis": "x", "a": {{"netz": {{"abdruck": "{ok}"}}}}}}"#), false).unwrap().len(), 1);
    }

    /// ⚑ **Ein Block `pruefnetz` gilt nur, wenn danach gefragt ist.**
    #[test]
    fn das_pruefnetz_gilt_nur_im_testbau() {
        let ok = "b".repeat(64);
        let k = format!(r#"{{"klein": {{"pruefnetz": {{"abdruck": "{ok}"}}}}}}"#);
        assert!(netzliste_aus(&k, false).unwrap().is_empty());
        let mit = netzliste_aus(&k, true).unwrap();
        assert_eq!(mit.len(), 1);
        assert!(mit[0].pruefnetz);
    }

    /// ⚑ **Das echte 0,6B steht mit seinem Abdruck im Pruefnetz**, und nur
    /// dort: Die Laufzeit rechnet ihn wie das Werkzeug, das ihn eingetragen
    /// hat. Ohne Artefakt (CI) oder mit `MYL_OHNE_ARTEFAKTE` uebersprungen.
    #[test]
    fn das_echte_kleine_modell_rechnet_nur_im_pruefnetz() {
        if std::env::var_os("MYL_OHNE_ARTEFAKTE").is_some() {
            return;
        }
        let a = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../artifacts/sparrow-0.6b-int8");
        if !a.join("theta_v.json").is_file() {
            return;
        }
        let f = pruefen(&a, &netzliste_aus(KATALOG, true).unwrap()).expect("sparrow-0.6b-int8 im Pruefnetz");
        assert_eq!(f.name, "sparrow-0.6b-int8");
        assert!(f.pruefnetz);
        assert!(pruefen(&a, &netzliste_aus(KATALOG, false).unwrap()).is_err(), "nicht im Netz");
    }

    /// ⚑ **Der echte Katalog ergibt eine gueltige Liste**, kein privates
    /// Modell steht darauf, und ohne Pruefnetz nur das 35B.
    #[test]
    fn der_katalog_ergibt_eine_gueltige_netzliste() {
        let liste = netzliste_aus(KATALOG, cfg!(feature = "pruefnetz")).expect("Netzliste aus dem Katalog");
        assert_eq!(liste, netzliste());
        assert!(!liste.iter().any(|f| f.name.contains("125b")), "Flash-Next ist privat");
        let netz = netzliste_aus(KATALOG, false).unwrap();
        assert!(!netz.is_empty() && netz.iter().all(|f| f.name.starts_with("raven-35b-a3b-")), "{netz:?}");
    }
}
