//! `myl-test netz` — Betriebsprotokolle mehrerer Knoten auswerten.
//!
//! # Wofür das da ist
//!
//! Ein Testlauf über mehrere Maschinen endet mit einem Stapel Dateien.
//! Dieses Modul macht daraus ein Urteil, so wie [`crate::vergleich`] es
//! für den Determinismusnachweis tut.
//!
//! Die Fragen, die danach jemand hat, sind immer dieselben:
//!
//! 1. **Wer war dabei?** Welche Knoten haben protokolliert, wie lange.
//! 2. **Haben sie einander gesehen?** Nicht „gab es Verbindungen",
//!    sondern: hat *jeder* mindestens einen anderen gesehen, und
//!    welchen.
//! 3. **Fehlt etwas?** Lücken in der Folgenummer heißen, dass Zeilen
//!    fehlen, und dann trägt der Rest des Urteils weniger weit.
//! 4. **Wurde jemand abgewiesen?** Verbindungsgrenzen sind erwünscht;
//!    dass sie greifen, muss trotzdem sichtbar sein.
//!
//! # ⚑ Was dieses Modul ausdrücklich nicht tut
//!
//! Es urteilt **nicht** über die Uhr. Die Zeitstempel stammen von
//! verschiedenen Maschinen und sind nicht verlässlich synchron. Ein
//! Werkzeug, das daraus eine Reihenfolge über Maschinengrenzen hinweg
//! ableitet, würde eine Genauigkeit vortäuschen, die es nicht gibt.
//! Verglichen werden deshalb **Aussagen über Verbindungen**, und die
//! trägt jede Seite selbst bei: Sieht A eine Verbindung zu B und B eine
//! zu A, ist das eine Übereinstimmung, ganz ohne gemeinsame Uhr.
//!
//! Es urteilt auch nicht über den Konsens. Die Knoten produzieren heute
//! keine Blöcke, und ein Urteil über eine Kette, die es nicht gibt,
//! wäre erfunden.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Was ein Knoten in seinem Protokoll über sich sagt.
#[derive(Debug, Clone)]
pub struct Knotenbild {
    pub name: String,
    pub peer: String,
    pub datei: PathBuf,
    pub zeilen: u64,
    /// Die höchste gesehene Folgenummer. Weicht sie von `zeilen` ab,
    /// fehlen Zeilen.
    pub hoechste_folge: u64,
    pub erste_zeit_ms: i64,
    pub letzte_zeit_ms: i64,
    /// Peers, zu denen dieser Knoten eine Verbindung vermerkt hat.
    pub gesehen: BTreeSet<String>,
    /// Zahl der abgewiesenen Verbindungsversuche.
    pub abgewiesen: u64,
    /// Zahl der empfangenen Gossip-Nachrichten.
    pub empfangen: u64,
    /// Zahl der gesendeten und angenommenen Nachrichten.
    pub gesendet: u64,
    /// Ob eine Verbindung über ein Relais lief.
    pub vermittelt: bool,
}

impl Knotenbild {
    /// Ob die Folge lückenlos ist.
    pub fn lueckenlos(&self) -> bool {
        self.zeilen == self.hoechste_folge
    }

    /// Laufzeit in Sekunden, wie der Knoten selbst sie sah.
    pub fn laufzeit_s(&self) -> i64 {
        (self.letzte_zeit_ms - self.erste_zeit_ms).max(0) / 1000
    }
}

/// Das Urteil über einen Netzlauf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Urteil {
    /// Kein einziges Protokoll gefunden.
    KeineProtokolle,
    /// Nur ein Knoten. Über ein Netz sagt das nichts, genauso wenig wie
    /// zwei gleiche Digests von derselben Maschine über Determinismus.
    EinKnoten,
    /// Mindestens ein Knoten hat keinen anderen gesehen.
    NichtAlleVerbunden { einsam: Vec<String> },
    /// Alle Knoten haben einander gesehen, aber Protokollzeilen fehlen.
    VerbundenMitLuecken { unvollstaendig: Vec<String> },
    /// Alle Knoten haben mindestens einen anderen gesehen, alle
    /// Protokolle sind lückenlos.
    Verbunden,
}

impl Urteil {
    /// Ob der Lauf als gelungen zu werten ist.
    pub fn gelungen(&self) -> bool {
        matches!(self, Urteil::Verbunden)
    }

    pub fn als_text(&self) -> String {
        match self {
            Urteil::KeineProtokolle => "keine Protokolle gefunden".to_string(),
            Urteil::EinKnoten => {
                "nur ein Knoten: über ein Netz sagt das nichts".to_string()
            }
            Urteil::NichtAlleVerbunden { einsam } => format!(
                "{} Knoten ohne jede Verbindung: {}",
                einsam.len(),
                einsam.join(", ")
            ),
            Urteil::VerbundenMitLuecken { unvollstaendig } => format!(
                "verbunden, aber {} Protokoll(e) mit fehlenden Zeilen: {}",
                unvollstaendig.len(),
                unvollstaendig.join(", ")
            ),
            Urteil::Verbunden => "alle Knoten verbunden, alle Protokolle vollständig".to_string(),
        }
    }
}

/// Ein einzelnes Feld aus einer Protokollzeile lesen.
///
/// Eigener Leser aus demselben Grund wie in [`crate::vergleich`]: Das
/// Format verspricht flache Objekte aus Zeichenketten, Zahlen und
/// Wahrheitswerten. Braucht dieser Leser mehr, hat das Format sein
/// Versprechen gebrochen, und das soll auffallen.
fn text_feld(zeile: &str, name: &str) -> Option<String> {
    let muster = format!("\"{name}\":\"");
    let start = zeile.find(&muster)? + muster.len();
    let rest = &zeile[start..];
    let mut wert = String::new();
    let mut zeichen = rest.chars();
    while let Some(c) = zeichen.next() {
        match c {
            '"' => return Some(wert),
            '\\' => match zeichen.next() {
                Some('n') => wert.push('\n'),
                Some('t') => wert.push('\t'),
                Some('r') => wert.push('\r'),
                Some(anderes) => wert.push(anderes),
                None => return Some(wert),
            },
            c => wert.push(c),
        }
    }
    Some(wert)
}

fn zahl_feld(zeile: &str, name: &str) -> Option<i64> {
    let muster = format!("\"{name}\":");
    let start = zeile.find(&muster)? + muster.len();
    let rest = &zeile[start..];
    if rest.starts_with('"') {
        return None;
    }
    let ende = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(rest.len());
    rest[..ende].parse().ok()
}

fn wahr_feld(zeile: &str, name: &str) -> bool {
    zeile.contains(&format!("\"{name}\":true"))
}

/// Liest ein einzelnes Betriebsprotokoll.
pub fn lies_protokoll(pfad: &Path) -> Result<Knotenbild, String> {
    let inhalt = std::fs::read_to_string(pfad)
        .map_err(|e| format!("{}: {e}", pfad.display()))?;
    let mut bild = Knotenbild {
        name: String::new(),
        peer: String::new(),
        datei: pfad.to_path_buf(),
        zeilen: 0,
        hoechste_folge: 0,
        erste_zeit_ms: 0,
        letzte_zeit_ms: 0,
        gesehen: BTreeSet::new(),
        abgewiesen: 0,
        empfangen: 0,
        gesendet: 0,
        vermittelt: false,
    };
    for zeile in inhalt.lines().filter(|z| !z.trim().is_empty()) {
        bild.zeilen += 1;
        if let Some(f) = zahl_feld(zeile, "folge") {
            bild.hoechste_folge = bild.hoechste_folge.max(f.max(0) as u64);
        }
        if let Some(t) = zahl_feld(zeile, "zeit_ms") {
            if bild.erste_zeit_ms == 0 {
                bild.erste_zeit_ms = t;
            }
            bild.letzte_zeit_ms = t;
        }
        if bild.name.is_empty() {
            if let Some(n) = text_feld(zeile, "knoten") {
                bild.name = n;
            }
        }
        if bild.peer.is_empty() {
            if let Some(p) = text_feld(zeile, "peer") {
                bild.peer = p;
            }
        }
        match text_feld(zeile, "art").as_deref() {
            Some("verbunden") => {
                if let Some(g) = text_feld(zeile, "gegenstelle") {
                    if !g.is_empty() {
                        bild.gesehen.insert(g);
                    }
                }
                if wahr_feld(zeile, "vermittelt") {
                    bild.vermittelt = true;
                }
            }
            Some("abgewiesen") => bild.abgewiesen += 1,
            Some("empfangen") => bild.empfangen += 1,
            Some("gesendet") if wahr_feld(zeile, "angenommen") => bild.gesendet += 1,
            _ => {}
        }
    }
    if bild.name.is_empty() {
        return Err(format!(
            "{}: keine Zeile nennt einen Knoten, das ist kein Betriebsprotokoll",
            pfad.display()
        ));
    }
    Ok(bild)
}

/// Sammelt alle Protokolle eines Verzeichnisses ein.
///
/// **Je Knoten das jüngste.** In einem Sammelverzeichnis liegen nach
/// mehreren Läufen mehrere Dateien desselben Knotens, und die alten
/// würden das Urteil verfälschen: Ein Knoten, der im letzten Lauf
/// verbunden war, sähe wegen eines älteren Protokolls „einsam" aus.
pub fn sammle(verzeichnis: &Path) -> Result<Vec<Knotenbild>, String> {
    let eintraege = std::fs::read_dir(verzeichnis)
        .map_err(|e| format!("{}: {e}", verzeichnis.display()))?;
    let mut je_knoten: BTreeMap<String, Knotenbild> = BTreeMap::new();
    for e in eintraege.flatten() {
        let pfad = e.path();
        if pfad.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        let bild = match lies_protokoll(&pfad) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let schluessel = if bild.peer.is_empty() { bild.name.clone() } else { bild.peer.clone() };
        match je_knoten.get(&schluessel) {
            Some(vorhanden) if vorhanden.erste_zeit_ms >= bild.erste_zeit_ms => {}
            _ => {
                je_knoten.insert(schluessel, bild);
            }
        }
    }
    Ok(je_knoten.into_values().collect())
}

/// Fällt das Urteil über eine Sammlung von Knotenbildern.
pub fn beurteile(bilder: &[Knotenbild]) -> Urteil {
    if bilder.is_empty() {
        return Urteil::KeineProtokolle;
    }
    if bilder.len() == 1 {
        return Urteil::EinKnoten;
    }
    let einsam: Vec<String> = bilder
        .iter()
        .filter(|b| b.gesehen.is_empty())
        .map(|b| b.name.clone())
        .collect();
    if !einsam.is_empty() {
        return Urteil::NichtAlleVerbunden { einsam };
    }
    let unvollstaendig: Vec<String> = bilder
        .iter()
        .filter(|b| !b.lueckenlos())
        .map(|b| b.name.clone())
        .collect();
    if !unvollstaendig.is_empty() {
        return Urteil::VerbundenMitLuecken { unvollstaendig };
    }
    Urteil::Verbunden
}

/// Wer hat wen gesehen: die Gegenseitigkeit prüfen.
///
/// **Ohne gemeinsame Uhr auskommend**, und das ist der Punkt: Beide
/// Seiten sagen unabhängig voneinander etwas über dieselbe Verbindung.
/// Sagt nur eine davon etwas, ist das eine Auffälligkeit, kein Beweis
/// für einen Fehler: Ein Knoten kann früher beendet worden sein.
pub fn einseitige_sichten(bilder: &[Knotenbild]) -> Vec<(String, String)> {
    let mut ergebnis = Vec::new();
    for a in bilder {
        for peer in &a.gesehen {
            if let Some(b) = bilder.iter().find(|b| &b.peer == peer) {
                if !b.gesehen.contains(&a.peer) {
                    ergebnis.push((a.name.clone(), b.name.clone()));
                }
            }
        }
    }
    ergebnis
}

/// Schreibt den Bericht auf den Bildschirm und meldet, ob der Lauf
/// gelungen ist.
pub fn run(verzeichnis: &Path) -> bool {
    println!("Netzlauf-Auswertung: {}", verzeichnis.display());
    let bilder = match sammle(verzeichnis) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("  Fehler: {e}");
            return false;
        }
    };
    let urteil = beurteile(&bilder);

    println!();
    println!(
        "  {:<12} {:>7} {:>7} {:>6} {:>6} {:>6} {:>5}",
        "Knoten", "Zeilen", "Dauer s", "sah", "abgew", "empf", "gesd"
    );
    for b in &bilder {
        println!(
            "  {:<12} {:>7} {:>7} {:>6} {:>6} {:>6} {:>5}{}",
            b.name,
            b.zeilen,
            b.laufzeit_s(),
            b.gesehen.len(),
            b.abgewiesen,
            b.empfangen,
            b.gesendet,
            if b.vermittelt { "  (über Relais)" } else { "" }
        );
        if !b.lueckenlos() {
            println!(
                "      ⚠ Lücke: {} Zeilen, höchste Folge {}",
                b.zeilen, b.hoechste_folge
            );
        }
    }

    let einseitig = einseitige_sichten(&bilder);
    if !einseitig.is_empty() {
        println!();
        println!("  Einseitige Sichten (eine Seite sah die Verbindung, die andere nicht):");
        for (a, b) in &einseitig {
            println!("    {a} sah {b}, umgekehrt nicht");
        }
        println!("    Das ist kein Fehler an sich: Ein früher beendeter Knoten");
        println!("    sieht das Ende der Verbindung nicht mehr.");
    }

    println!();
    println!("  Urteil: {}", urteil.als_text());
    urteil.gelungen()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schreibe(verz: &Path, name: &str, zeilen: &[String]) -> PathBuf {
        let p = verz.join(format!("{name}.jsonl"));
        std::fs::write(&p, zeilen.join("\n") + "\n").unwrap();
        p
    }

    fn zeile(folge: u64, zeit: i64, knoten: &str, peer: &str, art: &str, extra: &str) -> String {
        format!(
            "{{\"folge\":{folge},\"zeit_ms\":{zeit},\"knoten\":\"{knoten}\",\
             \"peer\":\"{peer}\",\"art\":\"{art}\"{extra}}}"
        )
    }

    fn temp(marke: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("myl-netz-{marke}-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn ein_knoten_allein_ergibt_kein_netzurteil() {
        // Dieselbe Haltung wie beim Determinismusnachweis: Ein Werkzeug,
        // das aus einer Quelle ein Urteil über mehrere macht, täuscht.
        let bilder = vec![Knotenbild {
            name: "a".into(), peer: "p1".into(), datei: PathBuf::new(),
            zeilen: 5, hoechste_folge: 5, erste_zeit_ms: 0, letzte_zeit_ms: 1000,
            gesehen: BTreeSet::new(), abgewiesen: 0, empfangen: 0, gesendet: 0,
            vermittelt: false,
        }];
        assert_eq!(beurteile(&bilder), Urteil::EinKnoten);
        assert!(!beurteile(&bilder).gelungen());
    }

    #[test]
    fn ohne_protokolle_gibt_es_kein_urteil() {
        assert_eq!(beurteile(&[]), Urteil::KeineProtokolle);
    }

    #[test]
    fn ein_einsamer_knoten_faellt_auf() {
        let verz = temp("einsam");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 200, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\""),
        ]);
        schreibe(&verz, "b", &[zeile(1, 100, "b", "p2", "start", "")]);
        let bilder = sammle(&verz).unwrap();
        assert_eq!(bilder.len(), 2);
        match beurteile(&bilder) {
            Urteil::NichtAlleVerbunden { einsam } => assert_eq!(einsam, vec!["b".to_string()]),
            anderes => panic!("erwartet war NichtAlleVerbunden, war {anderes:?}"),
        }
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn zwei_verbundene_knoten_ergeben_ein_gelungenes_urteil() {
        let verz = temp("verbunden");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 200, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\",\"eingehend\":true"),
        ]);
        schreibe(&verz, "b", &[
            zeile(1, 100, "b", "p2", "start", ""),
            zeile(2, 200, "b", "p2", "verbunden", ",\"gegenstelle\":\"p1\",\"eingehend\":false"),
        ]);
        let bilder = sammle(&verz).unwrap();
        assert_eq!(beurteile(&bilder), Urteil::Verbunden);
        assert!(einseitige_sichten(&bilder).is_empty());
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn eine_luecke_in_der_folge_wird_gemeldet() {
        // Fehlende Zeilen heißen, dass der Rest des Urteils weniger weit
        // trägt. Das gehört gesagt, nicht überlesen.
        let verz = temp("luecke");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(7, 200, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\""),
        ]);
        schreibe(&verz, "b", &[
            zeile(1, 100, "b", "p2", "start", ""),
            zeile(2, 200, "b", "p2", "verbunden", ",\"gegenstelle\":\"p1\""),
        ]);
        let bilder = sammle(&verz).unwrap();
        match beurteile(&bilder) {
            Urteil::VerbundenMitLuecken { unvollstaendig } => {
                assert_eq!(unvollstaendig, vec!["a".to_string()])
            }
            anderes => panic!("erwartet war VerbundenMitLuecken, war {anderes:?}"),
        }
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn eine_einseitige_sicht_wird_benannt() {
        let bilder = vec![
            Knotenbild {
                name: "a".into(), peer: "p1".into(), datei: PathBuf::new(),
                zeilen: 2, hoechste_folge: 2, erste_zeit_ms: 0, letzte_zeit_ms: 1,
                gesehen: ["p2".to_string()].into_iter().collect(),
                abgewiesen: 0, empfangen: 0, gesendet: 0, vermittelt: false,
            },
            Knotenbild {
                name: "b".into(), peer: "p2".into(), datei: PathBuf::new(),
                zeilen: 2, hoechste_folge: 2, erste_zeit_ms: 0, letzte_zeit_ms: 1,
                gesehen: ["p3".to_string()].into_iter().collect(),
                abgewiesen: 0, empfangen: 0, gesendet: 0, vermittelt: false,
            },
        ];
        assert_eq!(
            einseitige_sichten(&bilder),
            vec![("a".to_string(), "b".to_string())]
        );
    }

    #[test]
    fn aus_mehreren_laeufen_zaehlt_der_juengste() {
        // Sonst sähe ein Knoten, der im letzten Lauf verbunden war,
        // wegen eines alten Protokolls einsam aus.
        let verz = temp("juengste");
        schreibe(&verz, "a-alt", &[zeile(1, 100, "a", "p1", "start", "")]);
        schreibe(&verz, "a-neu", &[
            zeile(1, 9000, "a", "p1", "start", ""),
            zeile(2, 9100, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\""),
        ]);
        let bilder = sammle(&verz).unwrap();
        assert_eq!(bilder.len(), 1, "derselbe Knoten wurde doppelt gezählt");
        assert_eq!(bilder[0].gesehen.len(), 1, "das alte Protokoll hat gewonnen");
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn maskierte_zeichen_werden_zurueckgelesen() {
        // Der Knoten maskiert Anführungszeichen in Fehlermeldungen. Wer
        // das beim Lesen nicht rückgängig macht, schneidet den Wert ab.
        let z = "{\"art\":\"getrennt\",\"grund\":\"er sagte \\\"nein\\\" und ging\"}";
        assert_eq!(
            text_feld(z, "grund").as_deref(),
            Some("er sagte \"nein\" und ging")
        );
    }

    #[test]
    fn eine_fremde_datei_bringt_die_sammlung_nicht_um() {
        let verz = temp("fremd");
        std::fs::write(verz.join("fremd.jsonl"), "kein json\n").unwrap();
        schreibe(&verz, "a", &[zeile(1, 100, "a", "p1", "start", "")]);
        let bilder = sammle(&verz).unwrap();
        assert_eq!(bilder.len(), 1);
        std::fs::remove_dir_all(&verz).ok();
    }
}
