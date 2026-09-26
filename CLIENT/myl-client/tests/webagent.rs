//! Die Web-Recherche im Agenten: Werkzeuge da, und nichts Gelesenes geht
//! als Suchfrage hinaus.
//!
//! ⚑ **Eine eigene Datei, also ein eigener Prozess**, weil hier
//! `MYL_WEB_SUCHE` gesetzt wird: auf eine Adresse unter `.invalid`, damit
//! auch eine kaputte Schranke keine echte Anfrage hinausschickt. Eine
//! Umgebungsvariable neben anderen Proben im selben Prozess hat dieses
//! Projekt zweimal bezahlt (Funde 378 und 385).

use myl_client::einstellungen::Agenteneinstellung;
use myl_client::werkzeuge::Werkzeugkiste;
use myl_local_agent::werkzeug::Ansageform;

#[test]
fn der_agent_sucht_im_web_und_verraet_nichts_gelesenes() {
    if !myl_client::netzwerkzeuge::curl_vorhanden() {
        eprintln!("ohne curl: uebersprungen");
        return;
    }
    std::env::set_var("MYL_WEB_SUCHE", "https://myelith-probe.invalid/?q={q}");
    let d = tempfile::tempdir().expect("Verzeichnis");
    std::fs::write(
        d.path().join("geheim.txt"),
        "Das Kennwort der Lagerverwaltung lautet Kobaltblau-Sieben-Achtzehn.",
    )
    .expect("schreiben");
    let mit = Agenteneinstellung {
        wurzel: Some(d.path().display().to_string()),
        web_recherche: true,
        netzsaat: Some("Recherchiere, wie man eine Lagerverwaltung absichert.".into()),
        ..Agenteneinstellung::default()
    };

    // Ohne Haekchen gibt es die Werkzeuge nicht.
    let ohne = Agenteneinstellung { web_recherche: false, ..mit.clone() };
    let r = myl_client::ruestung::ruesten(&ohne, Ansageform::Amtlich, Werkzeugkiste::Base, Vec::new()).expect("Ruestung");
    assert!(r.kasten.angebot("web_search").is_none(), "Web ohne Haekchen");

    let r = myl_client::ruestung::ruesten(&mit, Ansageform::Amtlich, Werkzeugkiste::Base, Vec::new()).expect("Ruestung");
    assert!(r.kasten.angebot("web_search").is_some() && r.kasten.angebot("web_read").is_some(), "keine Web-Werkzeuge im Agenten");

    // Der Agent liest die Datei; ab jetzt kennt das Modell das Geheimnis.
    let gelesen = r
        .kasten
        .ausfuehren_ungeprueft("read_file", &serde_json::json!({"pfad": "geheim.txt"}))
        .expect("read_file")
        .expect("lesen");
    assert!(gelesen.contains("Kobaltblau"));

    // ⛔️ Eine Suchfrage mit einem woertlichen Stueck daraus geht nicht hinaus.
    let verrat = r.kasten.ausfuehren_ungeprueft(
        "web_search",
        &serde_json::json!({"frage": "Kennwort der Lagerverwaltung lautet Kobaltblau-Sieben"}),
    );
    let text = match verrat.expect("web_search") {
        Ok(t) => t,
        Err(f) => f.grund,
    };
    assert!(text.contains("woertlichen Abschnitt"), "die Verratsprobe griff nicht: {text}");
}
