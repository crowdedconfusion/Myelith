//! ⛔️ **Die Netzseite des Agenten kennt nichts, was nur oertlich wirkt.**
//!
//! `myl-agent` beschreibt, was ein Agent im Netz tut und was davon
//! nachrechenbar oder bezeugt ist: Sitzungskontrakt, Herkunftsstufe,
//! Segmentkette. Was nur auf dem Rechner eines Nutzers wirkt, liegt in
//! Kisten mit `local-` im Ordnernamen, und im Client. Haengt die Netzseite
//! an einer davon, kann eine oertliche Aenderung eine pruefbare Aktion
//! veraendern, ohne dass es jemand sieht.
//!
//! ⚑ Geprueft wird die Sperrdatei, denn sie nennt auch jede mittelbare
//! Abhaengigkeit, und das Manifest, denn es nennt die Pfade.

/// Was die Netzseite nie ziehen darf, als Paketname. Die Kiste in
/// `local-agent/` heisst aus der Zeit davor `myl-local-agent`.
const OERTLICH: [&str; 4] = ["myl-local-agent", "myl-client", "myl-console", "myl-oberflaeche"];

/// Die Verstoesse in einer Sperrdatei und einem Manifest.
fn verstoesse(sperre: &str, manifest: &str) -> Vec<String> {
    let mut aus = Vec::new();
    for zeile in sperre.lines() {
        if let Some(name) = zeile.strip_prefix("name = ") {
            let name = name.trim_matches('"');
            if OERTLICH.contains(&name) {
                aus.push(format!("Cargo.lock zieht {name}"));
            }
        }
    }
    for zeile in manifest.lines().filter(|z| !z.trim_start().starts_with('#')) {
        if let Some(i) = zeile.find("path = \"") {
            let pfad = &zeile[i + 8..];
            let pfad = &pfad[..pfad.find('"').unwrap_or(pfad.len())];
            if pfad.split('/').any(|teil| teil.starts_with("local-")) || pfad.contains("CLIENT/") {
                aus.push(format!("Cargo.toml nennt {pfad}"));
            }
        }
    }
    aus
}

#[test]
fn die_netzseite_haengt_an_nichts_oertlichem() {
    let v = verstoesse(include_str!("../Cargo.lock"), include_str!("../Cargo.toml"));
    assert!(v.is_empty(), "myl-agent haengt an Oertlichem: {v:?}");
}

/// Die Gegenprobe: Ein Pfad nach `local-` und ein oertliches Paket in der
/// Sperrdatei werden gefunden.
#[test]
fn die_probe_findet_einen_verstoss() {
    let sperre = "[[package]]\nname = \"myl-local-agent\"\nversion = \"0.18.0\"\n";
    let manifest = "[dependencies]\nx = { path = \"../local-agent\" }\n# y = { path = \"../local-skills\" }\n";
    let v = verstoesse(sperre, manifest);
    assert_eq!(v.len(), 2, "{v:?}");
}
