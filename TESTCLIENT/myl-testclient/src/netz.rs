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
    /// Fingerabdrücke der gesendeten und angenommenen Nutzlasten,
    /// mit dem Zeitpunkt des Sendens nach der **eigenen** Uhr.
    ///
    /// Der Zeitpunkt dient allein dazu, Fehlalarme zu vermeiden: Eine
    /// Nachricht kann keinen Knoten erreichen, der zu dem Zeitpunkt
    /// noch nicht lief. Siehe [`nachrichtenwege`].
    pub gesendete_digests: BTreeMap<String, i64>,
    /// Fingerabdrücke der empfangenen Nutzlasten.
    pub empfangene_digests: BTreeSet<String>,
    /// Verworfene Nachrichten, nach Grund gezählt.
    pub verworfen: BTreeMap<String, u64>,
    /// Größtes Mesh über alle Topics in der letzten Zustandsaufnahme.
    ///
    /// **Verbunden heißt nicht im Mesh.** Ein Knoten mit Verbindungen
    /// und leerem Mesh bekommt nur Ankündigungen statt Nachrichten.
    /// Ohne diese Zahl sähe das aus wie ein stilles Netz.
    pub mesh_groesse: u64,
    /// Peers unter der Gossip-Schwelle in der letzten Aufnahme.
    /// Ein bewerteter Peer sieht sonst aus wie ein stiller.
    pub schlecht_bewertet: u64,
    /// Verbundene Peers **in derselben Aufnahme** wie [`Self::mesh_groesse`].
    ///
    /// Getrennt von [`Self::gesehen`], und der Unterschied ist wichtig:
    /// `gesehen` sammelt über den ganzen Lauf, das hier ist ein
    /// Momentwert. Sie zu vergleichen wäre falsch, siehe
    /// [`Self::stumm_im_mesh`].
    pub peers_bei_aufnahme: u64,
    /// Was AutoNAT über die eigene Erreichbarkeit gesagt hat.
    pub erreichbar: Option<bool>,
    /// Verbindungen nach Transport: über QUIC, über TCP, über ein Relais.
    ///
    /// **Die Aufschlüsselung beantwortet die offene Frage der
    /// Netzschicht:** Lochstanzen gelingt über UDP zuverlässig, über TCP
    /// oft nicht. Wer wissen will, ob QUIC seinen Platz im Stack
    /// verdient, zählt hier nach, statt es zu glauben.
    pub ueber_quic: u64,
    pub ueber_tcp: u64,
    pub ueber_relais: u64,
    /// Gelungene und gescheiterte Lochstanzversuche (DCUtR).
    ///
    /// **Auf einer Maschine immer null**, dort gibt es nichts zu
    /// durchstoßen. Erst ein Lauf über getrennte Anschlüsse füllt diese
    /// Zahlen, und dann sind sie die interessantesten des Berichts.
    pub lochstanzen_gelungen: u64,
    pub lochstanzen_gescheitert: u64,
    /// Kleinste und größte gemessene Paarlatenz in Mikrosekunden, aus
    /// der letzten Aufnahme mit Messungen.
    pub latenz_min_us: u64,
    pub latenz_max_us: u64,
    /// Die Zustandswurzel je Höhe, wie **dieser** Knoten sie errechnet
    /// hat.
    ///
    /// **Der Kern des Kettenabgleichs.** Zwei Knoten, die aus denselben
    /// Blöcken verschiedene Wurzeln errechnen, haben irgendwo im
    /// Ledger-Pfad etwas Nichtdeterministisches. Das bricht den Konsens
    /// genauso wie ein abweichendes Inferenzergebnis, und es fiele
    /// sonst erst im echten Netz auf.
    pub wurzeln: BTreeMap<u64, String>,
    /// Selbst gebaute Blöcke.
    pub bloecke_erzeugt: u64,
    /// Übernommene Blöcke.
    pub bloecke_uebernommen: u64,
    /// Abgelehnte Blöcke nach Art.
    pub bloecke_abgelehnt: BTreeMap<String, u64>,
    /// Ausgeführte Proben: Kennung → (gelungen, gescheitert).
    pub proben: BTreeMap<String, (u64, u64)>,
    /// Empfangene Nachrichten je Probe. Zusammen mit [`Self::proben`]
    /// die Antwort auf „ist die Funktion **über die Leitung** gegangen".
    pub proben_empfangen: BTreeMap<String, u64>,
    /// Wann dieser Knoten eine Verbindung zu welchem Peer vermerkt hat.
    ///
    /// Zwei Knoten notieren dieselbe Verbindung; der Unterschied ihrer
    /// Zeitstempel ist der Uhrversatz. Siehe [`uhrversatz_ms`].
    pub verbindungszeit: BTreeMap<String, i64>,
    /// Angenommene und verworfene Latenz-Atteste (Sicherheitsaudit A10).
    ///
    /// **Beide Zahlen zusammen sind die Aussage.** Nur angenommene
    /// hieße, dass nie eine Fälschung vorkam; nur verworfene, dass
    /// niemand ein gültiges schickt. Der Prüfpfad ist erst dann belegt,
    /// wenn beides vorkommt oder wenigstens das erste.
    pub atteste_angenommen: u64,
    pub atteste_verworfen: u64,
    /// Bekannte Aussteller im Validatorsatz dieses Knotens.
    /// **Null heißt: Der Knoten kann kein Attest prüfen** und verwirft
    /// alle, meist wegen einer fehlenden Teilnehmerliste.
    pub bekannte_aussteller: u64,
    /// Wie der Lauf endete, falls er ordentlich endete.
    ///
    /// **`None` heißt: abgestürzt, hart abgeschossen, oder läuft noch.**
    /// Das ist die erste Frage, wenn ein Protokoll kürzer ist als die
    /// anderen, und ohne dieses Feld müsste sie geraten werden.
    pub ende: Option<String>,
}

impl Knotenbild {
    /// Verbindungen da, Mesh leer: Der Knoten ist im Netz und bekommt
    /// trotzdem keine Nachrichten.
    ///
    /// **Beide Zahlen stammen aus derselben Aufnahme**, und das ist
    /// nicht kosmetisch. Die erste Fassung verglich [`Self::gesehen`],
    /// eine über den ganzen Lauf gesammelte Menge, gegen die Mesh-Größe
    /// aus der letzten Aufnahme. Beim ersten echten Dreiknotenlauf
    /// meldete sie prompt einen stummen Knoten, der keiner war: Alpha
    /// lief sechs Sekunden länger als die anderen, seine letzte Aufnahme
    /// entstand also, als er allein war. **Ein Momentwert gegen eine
    /// Sammlung ergibt einen Fehlalarm, sobald ein Knoten seine Peers
    /// überlebt**, und das tut in jedem Lauf mindestens einer.
    pub fn stumm_im_mesh(&self) -> bool {
        self.peers_bei_aufnahme > 0 && self.mesh_groesse == 0
    }

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

/// Ob eine Datei ein Betriebsprotokoll eines Knotens ist.
///
/// # Warum das hier steht und wer es sonst noch braucht
///
/// Beide Auswertungen lesen denselben Ordner: Ein Koordinator sammelt
/// Determinismusläufe und Netzprotokolle an einer Stelle, nicht an
/// zweien. Beide überspringen fremde Dateien schon von sich aus, aber
/// **still**, und Stille ist hier die falsche Antwort: Wer nicht sagt,
/// dass er drei Dateien liegen lässt, lässt offen, ob sie fehlen oder
/// nicht dazugehören.
///
/// Geprüft wird die **erste nichtleere Zeile**. Ein Betriebsprotokoll
/// trägt in jeder Zeile `folge`, `knoten` und `peer`; ein
/// Determinismus-Protokoll trägt keines davon. Die ganze Datei zu
/// lesen, um sie wegzuwerfen, machte aus dem Einsammeln eine Wartezeit.
pub fn ist_betriebsprotokoll(pfad: &Path) -> bool {
    let Ok(inhalt) = std::fs::read_to_string(pfad) else {
        return false;
    };
    let Some(erste) = inhalt.lines().find(|z| !z.trim().is_empty()) else {
        return false;
    };
    erste.contains("\"folge\":")
        && erste.contains("\"knoten\":")
        && erste.contains("\"peer\":")
}

/// Das Ergebnis des Einsammelns, samt dem, was liegen blieb.
#[derive(Debug, Clone, Default)]
pub struct Sammlung {
    /// Die gelesenen Betriebsprotokolle.
    pub knoten: Vec<Knotenbild>,
    /// `.jsonl`-Dateien, die keine Betriebsprotokolle sind, in aller
    /// Regel Determinismusläufe. Sie gehören in denselben Ordner und
    /// sind kein Fehler.
    pub fremde: usize,
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
        gesendete_digests: BTreeMap::new(),
        empfangene_digests: BTreeSet::new(),
        verworfen: BTreeMap::new(),
        mesh_groesse: 0,
        schlecht_bewertet: 0,
        peers_bei_aufnahme: 0,
        erreichbar: None,
        ueber_quic: 0,
        ueber_tcp: 0,
        ueber_relais: 0,
        lochstanzen_gelungen: 0,
        lochstanzen_gescheitert: 0,
        latenz_min_us: 0,
        latenz_max_us: 0,
        wurzeln: BTreeMap::new(),
        bloecke_erzeugt: 0,
        bloecke_uebernommen: 0,
        bloecke_abgelehnt: BTreeMap::new(),
        proben: BTreeMap::new(),
        proben_empfangen: BTreeMap::new(),
        verbindungszeit: BTreeMap::new(),
        atteste_angenommen: 0,
        atteste_verworfen: 0,
        bekannte_aussteller: 0,
        ende: None,
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
                        // Die **erste** Verbindung zählt: Bei mehreren
                        // wäre unklar, welche der Gegenüber notiert hat.
                        bild.verbindungszeit
                            .entry(g.clone())
                            .or_insert_with(|| zahl_feld(zeile, "zeit_ms").unwrap_or(0));
                        bild.gesehen.insert(g);
                    }
                }
                if wahr_feld(zeile, "vermittelt") {
                    bild.vermittelt = true;
                    bild.ueber_relais += 1;
                }
                if wahr_feld(zeile, "quic") {
                    bild.ueber_quic += 1;
                } else {
                    bild.ueber_tcp += 1;
                }
            }
            Some("aufnahme") => {
                // Die letzte Aufnahme gewinnt: Sie beschreibt den
                // Zustand am Ende, und der ist der aussagekräftige.
                let mut groesstes = 0i64;
                for stelle in zeile.match_indices("\"mesh_") {
                    let rest = &zeile[stelle.0..];
                    if let Some(doppelpunkt) = rest.find("\":") {
                        let nach = &rest[doppelpunkt + 2..];
                        let ende = nach.find(|c: char| !c.is_ascii_digit()).unwrap_or(nach.len());
                        if let Ok(w) = nach[..ende].parse::<i64>() {
                            groesstes = groesstes.max(w);
                        }
                    }
                }
                bild.mesh_groesse = groesstes.max(0) as u64;
                if let Some(sb) = zahl_feld(zeile, "schlecht_bewertet") {
                    bild.schlecht_bewertet = sb.max(0) as u64;
                }
                if let Some(pz) = zahl_feld(zeile, "peers") {
                    bild.peers_bei_aufnahme = pz.max(0) as u64;
                }
                // Nur Aufnahmen mit Messungen überschreiben die Spanne:
                // Ein ruhiges Fenster am Ende löschte sonst, was vorher
                // gemessen wurde.
                if zahl_feld(zeile, "latenz_messungen").unwrap_or(0) > 0 {
                    if let Some(v) = zahl_feld(zeile, "latenz_min_us") {
                        bild.latenz_min_us = v.max(0) as u64;
                    }
                    if let Some(v) = zahl_feld(zeile, "latenz_max_us") {
                        bild.latenz_max_us = v.max(0) as u64;
                    }
                }
            }
            Some("erreichbarkeit") => {
                bild.erreichbar = Some(wahr_feld(zeile, "erreichbar"));
            }
            Some("block_erzeugt") | Some("block_uebernommen") => {
                if text_feld(zeile, "art").as_deref() == Some("block_erzeugt") {
                    bild.bloecke_erzeugt += 1;
                } else {
                    bild.bloecke_uebernommen += 1;
                }
                if let (Some(h), Some(w)) =
                    (zahl_feld(zeile, "hoehe"), text_feld(zeile, "zustandswurzel"))
                {
                    bild.wurzeln.insert(h.max(0) as u64, w);
                }
            }
            Some("block_abgelehnt") => {
                let art = text_feld(zeile, "art_grund")
                    .or_else(|| {
                        // Das Feld heißt im Protokoll `art`, und `art`
                        // ist zugleich der Eintragstyp. Hier steht der
                        // zweite Treffer.
                        let mut treffer = zeile.match_indices("\"art\":\"");
                        treffer.next();
                        treffer.next().and_then(|(i, _)| {
                            let rest = &zeile[i + 7..];
                            rest.find('"').map(|e| rest[..e].to_string())
                        })
                    })
                    .unwrap_or_else(|| "unbekannt".into());
                *bild.bloecke_abgelehnt.entry(art).or_insert(0) += 1;
            }
            Some("lochstanzen") => {
                if wahr_feld(zeile, "gelungen") {
                    bild.lochstanzen_gelungen += 1;
                } else {
                    bild.lochstanzen_gescheitert += 1;
                }
            }
            Some("abgewiesen") => bild.abgewiesen += 1,
            Some("empfangen") => {
                bild.empfangen += 1;
                if let Some(d) = text_feld(zeile, "digest") {
                    bild.empfangene_digests.insert(d);
                }
                if let Some(k) = text_feld(zeile, "kennung") {
                    *bild.proben_empfangen.entry(k).or_insert(0) += 1;
                }
            }
            Some("attest_angenommen") => bild.atteste_angenommen += 1,
            Some("attest_verworfen") => bild.atteste_verworfen += 1,
            Some("validatorsatz") => {
                if let Some(n) = zahl_feld(zeile, "bekannte_aussteller") {
                    bild.bekannte_aussteller = n.max(0) as u64;
                }
            }
            Some("ende") => {
                bild.ende = text_feld(zeile, "grund");
            }
            Some("probe") => {
                let k = text_feld(zeile, "kennung").unwrap_or_else(|| "unbekannt".into());
                let eintrag = bild.proben.entry(k).or_insert((0, 0));
                if wahr_feld(zeile, "gelungen") {
                    eintrag.0 += 1;
                } else {
                    eintrag.1 += 1;
                }
            }
            Some("verworfen") => {
                let grund = text_feld(zeile, "grund").unwrap_or_else(|| "unbekannt".into());
                *bild.verworfen.entry(grund).or_insert(0) += 1;
            }
            Some("gesendet") if wahr_feld(zeile, "angenommen") => {
                bild.gesendet += 1;
                if let Some(d) = text_feld(zeile, "digest") {
                    bild.gesendete_digests
                        .insert(d, zahl_feld(zeile, "zeit_ms").unwrap_or(0));
                }
            }
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
    Ok(sammle_mit_bericht(verzeichnis)?.knoten)
}

/// Wie [`sammle`], meldet zusätzlich, wie viele fremde Dateien im
/// Ordner lagen. Siehe [`ist_betriebsprotokoll`].
pub fn sammle_mit_bericht(verzeichnis: &Path) -> Result<Sammlung, String> {
    let eintraege = std::fs::read_dir(verzeichnis)
        .map_err(|e| format!("{}: {e}", verzeichnis.display()))?;
    let mut je_knoten: BTreeMap<String, Knotenbild> = BTreeMap::new();
    let mut fremde = 0usize;
    for e in eintraege.flatten() {
        let pfad = e.path();
        if pfad.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        if !ist_betriebsprotokoll(&pfad) {
            fremde += 1;
            continue;
        }
        let bild = match lies_protokoll(&pfad) {
            Ok(b) => b,
            Err(_) => {
                fremde += 1;
                continue;
            }
        };
        let schluessel = if bild.peer.is_empty() { bild.name.clone() } else { bild.peer.clone() };
        match je_knoten.get(&schluessel) {
            Some(vorhanden) if vorhanden.erste_zeit_ms >= bild.erste_zeit_ms => {}
            _ => {
                je_knoten.insert(schluessel, bild);
            }
        }
    }
    Ok(Sammlung { knoten: je_knoten.into_values().collect(), fremde })
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

/// Eine Höhe, auf der die Knoten **nicht** dieselbe Zustandswurzel
/// errechnet haben.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kettenabweichung {
    pub hoehe: u64,
    /// Knotenname zu errechneter Wurzel.
    pub wurzeln: Vec<(String, String)>,
}

/// Vergleicht die Zustandswurzeln aller Knoten Höhe für Höhe.
///
/// Gibt zurück, **wie viele Höhen überhaupt vergleichbar waren** (also
/// von mindestens zwei Knoten belegt) und welche davon abwichen.
///
/// Die erste Zahl gehört dazu: Null vergleichbare Höhen und null
/// Abweichungen sähen sonst aus wie ein bestandener Abgleich, wären
/// aber ein Lauf, in dem nichts zu vergleichen war.
pub fn kettenabgleich(bilder: &[Knotenbild]) -> (u64, Vec<Kettenabweichung>) {
    let mut hoehen: BTreeSet<u64> = BTreeSet::new();
    for b in bilder {
        hoehen.extend(b.wurzeln.keys().copied());
    }
    let mut vergleichbar = 0u64;
    let mut abweichungen = Vec::new();
    for h in hoehen {
        let paare: Vec<(String, String)> = bilder
            .iter()
            .filter_map(|b| b.wurzeln.get(&h).map(|w| (b.name.clone(), w.clone())))
            .collect();
        if paare.len() < 2 {
            continue;
        }
        vergleichbar += 1;
        let erste = &paare[0].1;
        if paare.iter().any(|(_, w)| w != erste) {
            abweichungen.push(Kettenabweichung { hoehe: h, wurzeln: paare });
        }
    }
    (vergleichbar, abweichungen)
}

/// Schätzt den Uhrversatz zwischen zwei Knoten.
///
/// # Wie das ohne gemeinsame Uhr geht
///
/// Wenn A eine Verbindung zu B vermerkt und B dieselbe zu A, ist das
/// **ein physisches Ereignis, zweimal notiert**. Der Unterschied der
/// beiden Zeitstempel ist der Uhrversatz, plus der Laufzeit des
/// Verbindungsaufbaus, und die ist gegenüber einer schief gehenden Uhr
/// klein.
///
/// Damit lässt sich die Annahme prüfen, auf der
/// [`UHRNACHSICHT_MS`] beruht, statt sie zu glauben. `None`, wenn keine
/// gemeinsame Verbindung im Protokoll steht.
pub fn uhrversatz_ms(a: &Knotenbild, b: &Knotenbild) -> Option<i64> {
    let a_sah_b = a.verbindungszeit.get(&b.peer)?;
    let b_sah_a = b.verbindungszeit.get(&a.peer)?;
    Some(a_sah_b - b_sah_a)
}

/// Der Weg einer Nachricht durch das Netz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nachrichtenweg {
    /// Fingerabdruck der Nutzlast.
    pub digest: String,
    /// Wer sie losgeschickt hat.
    pub absender: String,
    /// Wer sie empfangen hat.
    pub empfaenger: Vec<String>,
    /// Wer sie nicht empfangen hat, obwohl er zum Lauf gehört.
    pub ohne_empfang: Vec<String>,
}

impl Nachrichtenweg {
    /// Ob die Nachricht alle anderen Knoten erreicht hat.
    pub fn vollstaendig(&self) -> bool {
        self.ohne_empfang.is_empty()
    }
}

/// Verfolgt jede gesendete Nachricht über die Protokolle hinweg.
///
/// **Das ist die Frage, für die der Fingerabdruck da ist:** „kam an, was
/// losgeschickt wurde". Ohne ihn stünde in einem Protokoll „gesendet,
/// 141 Bytes" und im anderen „empfangen, 141 Bytes", und niemand könnte
/// sagen, ob es dieselbe Nachricht war.
///
/// Ganz ohne gemeinsame Uhr: Verglichen werden Fingerabdrücke, nicht
/// Zeitpunkte.
pub fn nachrichtenwege(bilder: &[Knotenbild]) -> Vec<Nachrichtenweg> {
    let mut wege = Vec::new();
    for absender in bilder {
        for (digest, gesendet_um) in &absender.gesendete_digests {
            let mut empfaenger = Vec::new();
            let mut ohne = Vec::new();
            for anderer in bilder {
                if anderer.peer == absender.peer {
                    continue;
                }
                if anderer.empfangene_digests.contains(digest) {
                    empfaenger.push(anderer.name.clone());
                } else if lief_zur_zeit(anderer, *gesendet_um) {
                    ohne.push(anderer.name.clone());
                }
                // Wer nicht lief, fehlt nicht: Er war nicht da.
            }
            wege.push(Nachrichtenweg {
                digest: digest.clone(),
                absender: absender.name.clone(),
                empfaenger,
                ohne_empfang: ohne,
            });
        }
    }
    wege
}

/// Zeitliche Nachsicht beim Vergleich der Laufzeiten.
///
/// **Fünf Sekunden, und die Zahl hat eine Geschichte.** Die erste
/// Fassung nahm eine Minute, „großzügig gegen Uhrabweichung". Beim
/// ersten Vierknotenlauf über sechzig Sekunden deckte diese Nachsicht
/// **den ganzen Lauf** ab: Jeder Knoten galt zu jedem Zeitpunkt als
/// laufend, und die Prüfung tat nichts.
///
/// Eine Nachsicht muss kleiner sein als das, was sie unterscheiden
/// soll. Fünf Sekunden decken die Abweichung ab, die Rechner mit
/// Zeitabgleich (NTP) untereinander haben, das sind üblicherweise
/// Millisekunden.
///
/// **Die Annahme steht im Bericht**, zusammen mit dem gemessenen
/// Versatz: Wer eine Maschine mit falsch gestellter Uhr im Lauf hat,
/// soll es sehen und nicht raten.
pub const UHRNACHSICHT_MS: i64 = 5_000;

/// Ob ein Knoten zum Zeitpunkt `wann` (nach der Uhr des Absenders) lief.
///
/// # ⚑ Warum hier doch eine Uhr vorkommt
///
/// Dieses Modul urteilt sonst bewusst nicht über Zeitpunkte, weil die
/// Uhren verschiedener Maschinen auseinanderlaufen. Hier geht es nicht
/// um ein Urteil, sondern um das **Vermeiden eines Fehlalarms**: Der
/// erste Vierknotenlauf meldete 51 von 78 Nachrichten als „nicht
/// überall angekommen", und fast alle davon waren an Knoten gerichtet,
/// die zu dem Zeitpunkt noch nicht liefen oder schon beendet waren.
///
/// Ein Bericht, der bei einem gesunden Lauf Alarm schlägt, wird nicht
/// gelesen. Deshalb die Nachsicht aus [`UHRNACHSICHT_MS`]: **großzügig
/// genug, dass Uhrabweichungen nichts auslösen, knapp genug, dass ein
/// wirklich abwesender Knoten erkannt wird.**
pub fn lief_zur_zeit(bild: &Knotenbild, wann: i64) -> bool {
    if wann == 0 || bild.erste_zeit_ms == 0 {
        // Ohne Zeitangabe lieber melden als verschweigen.
        return true;
    }
    wann >= bild.erste_zeit_ms - UHRNACHSICHT_MS
        && wann <= bild.letzte_zeit_ms + UHRNACHSICHT_MS
}

/// Schreibt den Bericht auf den Bildschirm und meldet, ob der Lauf
/// gelungen ist.
pub fn run(verzeichnis: &Path) -> bool {
    println!("Netzlauf-Auswertung: {}", verzeichnis.display());
    let sammlung = match sammle_mit_bericht(verzeichnis) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("  Fehler: {e}");
            return false;
        }
    };
    let bilder = sammlung.knoten;
    // Sagen, was liegen bleibt. Determinismusläufe gehören in denselben
    // Ordner; wer nicht erwähnt, dass er sie übergeht, lässt offen, ob
    // sie fehlen oder nicht dazugehören.
    if sammlung.fremde > 0 {
        println!(
            "  {} Betriebsprotokoll(e), {} andere Datei(en) übergangen \
             (Determinismusläufe gehören hierher und werden im Menü unter \
             „Protokolle vergleichen“ ausgewertet).",
            bilder.len(),
            sammlung.fremde
        );
    }
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

    // Stumme Knoten zuerst: verbunden, aber ohne Mesh. Das ist die
    // Lage, die im Protokoll am ehesten mit „das Netz war leer"
    // verwechselt wird, und die Ursache ist eine völlig andere.
    let stumme: Vec<&Knotenbild> = bilder.iter().filter(|b| b.stumm_im_mesh()).collect();
    if !stumme.is_empty() {
        println!();
        println!("  ⚠ Verbunden, aber in keinem Mesh (bekommt nur Ankündigungen):");
        for b in &stumme {
            println!(
                "    {} ({} Peer(s) verbunden, Mesh 0)",
                b.name, b.peers_bei_aufnahme
            );
        }
    }
    let bewertet: Vec<&Knotenbild> = bilder.iter().filter(|b| b.schlecht_bewertet > 0).collect();
    if !bewertet.is_empty() {
        println!();
        println!("  Peers unter der Gossip-Schwelle (bekommen kein Gossip mehr):");
        for b in &bewertet {
            println!("    {}: {}", b.name, b.schlecht_bewertet);
        }
    }
    let unerreichbar: Vec<&Knotenbild> =
        bilder.iter().filter(|b| b.erreichbar == Some(false)).collect();
    if !unerreichbar.is_empty() {
        println!();
        println!("  Von außen nicht erreichbar (AutoNAT):");
        for b in &unerreichbar {
            println!("    {} — braucht ein Relais, siehe --relais", b.name);
        }
    }

    // Verworfene Nachrichten: Sie beantworten die erste Frage
    // jeder Fehlersuche, nämlich ob etwas ankam und weggeworfen wurde.
    let mit_verwuerfen: Vec<&Knotenbild> =
        bilder.iter().filter(|b| !b.verworfen.is_empty()).collect();
    if !mit_verwuerfen.is_empty() {
        println!();
        println!("  Verworfene Nachrichten:");
        for b in &mit_verwuerfen {
            for (grund, anzahl) in &b.verworfen {
                println!("    {}: {anzahl}× {grund}", b.name);
            }
        }
    }

    let wege = nachrichtenwege(&bilder);
    if !wege.is_empty() {
        // **Nur die Fehlwege einzeln.** Ein Lauf über eine Stunde
        // erzeugt hunderte Nachrichten; sie alle aufzulisten macht aus
        // dem Bericht eine Datei, die niemand liest, und darin geht die
        // eine unter, auf die es ankommt.
        let (vollstaendig, unvollstaendig): (Vec<_>, Vec<_>) =
            wege.iter().partition(|w| w.vollstaendig());
        println!();
        println!(
            "  Nachrichtenwege: {} von {} Nachrichten haben alle erreicht, die zu der \
             Zeit liefen.",
            vollstaendig.len(),
            wege.len()
        );
        if unvollstaendig.is_empty() {
            // Ein Beispiel nennen, damit sichtbar ist, dass wirklich
            // Fingerabdrücke verglichen wurden und nicht nur gezählt.
            if let Some(w) = vollstaendig.first() {
                println!(
                    "    Beispiel: {} von {} → alle {} erreicht",
                    w.digest,
                    w.absender,
                    w.empfaenger.len()
                );
            }
        } else {
            println!("    Nicht angekommen, obwohl der Empfänger zu der Zeit lief:");
            for w in unvollstaendig.iter().take(20) {
                println!(
                    "      {} von {} → {} erreicht, {} NICHT: {}",
                    w.digest,
                    w.absender,
                    w.empfaenger.len(),
                    w.ohne_empfang.len(),
                    w.ohne_empfang.join(", ")
                );
            }
            if unvollstaendig.len() > 20 {
                println!("      … und {} weitere", unvollstaendig.len() - 20);
            }
        }
    }

    // Den Uhrversatz messen, bevor irgendetwas ihn voraussetzt.
    let mut versatz: Vec<(String, String, i64)> = Vec::new();
    for (i, a) in bilder.iter().enumerate() {
        for b in bilder.iter().skip(i + 1) {
            if let Some(v) = uhrversatz_ms(a, b) {
                versatz.push((a.name.clone(), b.name.clone(), v));
            }
        }
    }
    if let Some(groesster) = versatz.iter().map(|(_, _, v)| v.abs()).max() {
        println!();
        if groesster > UHRNACHSICHT_MS {
            println!(
                "  ⚠ Uhrversatz zwischen den Maschinen bis {} ms, mehr als die \
                 Nachsicht von {} ms.",
                groesster, UHRNACHSICHT_MS
            );
            println!("    Zeitbezogene Hinweise unten sind mit Vorsicht zu lesen.");
            for (a, b, v) in versatz.iter().filter(|(_, _, v)| v.abs() > UHRNACHSICHT_MS) {
                println!("      {a} gegen {b}: {v} ms");
            }
        } else {
            println!("  Uhrversatz zwischen den Maschinen: höchstens {groesster} ms.");
        }
    }

    // Wer nicht ordentlich endete, als Nächstes: Ein Protokoll ohne
    // Abschlusseintrag heißt abgestürzt, hart abgeschossen oder noch
    // laufend, und das ändert, wie alles Folgende zu lesen ist.
    let ohne_abschluss: Vec<&Knotenbild> = bilder.iter().filter(|b| b.ende.is_none()).collect();
    if !ohne_abschluss.is_empty() {
        println!();
        println!("  ⚠ Ohne Abschlusseintrag beendet:");
        for b in &ohne_abschluss {
            println!("    {} ({} Zeilen)", b.name, b.zeilen);
        }
        println!("    Abgestürzt, hart abgeschossen, oder der Lauf läuft noch.");
        println!("    Ein Lauf, der regulär endet, schreibt einen ende-Eintrag mit Grund.");
    }
    let mit_abbruch: Vec<&Knotenbild> = bilder
        .iter()
        .filter(|b| b.ende.as_deref() == Some("Abbruchsignal"))
        .collect();
    if !mit_abbruch.is_empty() {
        println!();
        println!("  Vorzeitig abgebrochen (Strg-C):");
        for b in &mit_abbruch {
            println!("    {} nach {} s", b.name, b.laufzeit_s());
        }
    }

    // Die Abdeckung als Nächstes: **Welche Funktion wurde überhaupt
    // ausprobiert?** Ein Bericht, der nur nennt, was lief, verschweigt
    // das Wichtigere. Eine Probe, die nie lief, ist kein Erfolg, und
    // ohne diese Tabelle ließe sich das nicht von einer bestandenen
    // unterscheiden.
    println!();
    println!("  Probelauf: welche Funktion wurde ausprobiert");
    println!("  (dies ist eine Trockenübung des Codes, nicht der Beginn der Kette)");
    println!();
    println!(
        "    {:<18} {:>8} {:>8} {:>9}   belegt",
        "Funktion", "gesendet", "gefehlt", "empfangen"
    );
    let mut ungeprueft: Vec<&'static str> = Vec::new();
    for probe in myl_node::Probe::ALLE {
        let k = probe.kennung();
        let (ok, fehl) = bilder.iter().fold((0u64, 0u64), |(a, b), bild| {
            let (x, y) = bild.proben.get(k).copied().unwrap_or((0, 0));
            (a + x, b + y)
        });
        let empf: u64 = bilder
            .iter()
            .map(|b| b.proben_empfangen.get(k).copied().unwrap_or(0))
            .sum();
        // Netz und Nachrichtenfluss ergeben sich aus dem Verhalten:
        // Sie haben keine eigene Nachricht, ihr Beleg steht weiter unten.
        let verhaltensprobe = probe.topic().is_none();
        if ok == 0 && empf == 0 && !verhaltensprobe {
            ungeprueft.push(k);
        }
        println!(
            "    {:<18} {:>8} {:>8} {:>9}   {}",
            k,
            if verhaltensprobe { "—".to_string() } else { ok.to_string() },
            if verhaltensprobe { "—".to_string() } else { fehl.to_string() },
            if verhaltensprobe { "—".to_string() } else { empf.to_string() },
            probe.was_sie_belegt()
        );
    }
    if !ungeprueft.is_empty() {
        println!();
        println!(
            "  ⚠ Nicht ausprobiert: {}. Über diese Funktionen sagt der Lauf nichts.",
            ungeprueft.join(", ")
        );
    }

    // Latenz-Atteste: Sicherheitsaudit A10. Bis zum 2026-08-25 prüfte
    // die Signatur niemand, und ein ungeprüftes Signaturfeld ist
    // gefährlicher als gar keines, weil ein Leser es für einen Schutz
    // hält.
    let attest_gesamt: u64 = bilder
        .iter()
        .map(|b| b.atteste_angenommen + b.atteste_verworfen)
        .sum();
    let ohne_satz: Vec<&Knotenbild> =
        bilder.iter().filter(|b| b.bekannte_aussteller == 0).collect();
    if attest_gesamt > 0 || !ohne_satz.is_empty() {
        println!();
        println!("  Latenz-Atteste (Signaturprüfung, Audit A10):");
        for b in &bilder {
            println!(
                "    {:<14} {:>3} angenommen, {:>3} verworfen, {} bekannte Aussteller",
                b.name, b.atteste_angenommen, b.atteste_verworfen, b.bekannte_aussteller
            );
        }
        if !ohne_satz.is_empty() {
            println!();
            println!("    ⚠ Diese Knoten kennen keinen Aussteller und verwerfen deshalb");
            println!("      JEDES Attest: {}",
                ohne_satz.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join(", "));
            println!("      Fast immer eine fehlende Teilnehmerliste, kein Angriff.");
        }
    }

    // Der Kettenabgleich als Nächstes: Er ist die schwerwiegendste Aussage
    // des Berichts. Alles andere betrifft das Netz, dies betrifft den
    // Zustand.
    let (vergleichbar, abweichungen) = kettenabgleich(&bilder);
    let erzeugt: u64 = bilder.iter().map(|b| b.bloecke_erzeugt).sum();
    if erzeugt > 0 || vergleichbar > 0 {
        println!();
        println!("  Kette:");
        for b in &bilder {
            let hoechste = b.wurzeln.keys().next_back().copied().unwrap_or(0);
            println!(
                "    {:<12} Höhe {:>3}, {} erzeugt, {} übernommen{}",
                b.name,
                hoechste,
                b.bloecke_erzeugt,
                b.bloecke_uebernommen,
                if b.bloecke_abgelehnt.is_empty() {
                    String::new()
                } else {
                    let mut t = String::from(", abgelehnt: ");
                    let liste: Vec<String> = b
                        .bloecke_abgelehnt
                        .iter()
                        .map(|(a, n)| format!("{n}× {a}"))
                        .collect();
                    t.push_str(&liste.join(", "));
                    t
                }
            );
        }
        println!();
        if abweichungen.is_empty() && vergleichbar > 0 {
            println!(
                "  ✓ Zustandswurzeln stimmen auf allen {vergleichbar} vergleichbaren \
                 Höhen überein."
            );
        } else if vergleichbar == 0 {
            println!("  Keine Höhe war von zwei Knoten belegt: nichts zu vergleichen.");
            // Die häufigste Ursache benennen, statt den Leser raten zu
            // lassen. Höhe 0 plus abgelehnte Blöcke heißt fast immer:
            // Der Erzeuger war schneller als die Verbindungen.
            let abgehaengt: Vec<&Knotenbild> = bilder
                .iter()
                .filter(|b| {
                    b.wurzeln.is_empty()
                        && b.bloecke_abgelehnt.get("passt-nicht-an").copied().unwrap_or(0) > 0
                })
                .collect();
            if !abgehaengt.is_empty() {
                println!();
                println!("  Ursache: {} Knoten haben Blöcke bekommen, die nicht an ihre",
                    abgehaengt.len());
                println!("  eigene Kette anschließen, und stehen deshalb auf Höhe 0:");
                for b in &abgehaengt {
                    println!(
                        "    {} ({}× passt-nicht-an)",
                        b.name,
                        b.bloecke_abgelehnt.get("passt-nicht-an").copied().unwrap_or(0)
                    );
                }
                println!();
                println!("  Sie sind später dazugekommen als der erste Block. **Es gibt");
                println!("  keinen Nachholmechanismus**: Jeder folgende Block zeigt auf");
                println!("  einen Vorgänger, den sie nie gesehen haben. Abhilfe für den");
                println!("  nächsten Lauf: alle Knoten starten lassen, bevor der Erzeuger");
                println!("  beginnt. Eine Blocksynchronisierung fehlt und gehört vor ein");
                println!("  echtes Testnetz.");
            }
        } else {
            println!(
                "  ⚠⚠ ZUSTANDSWURZELN WEICHEN AB auf {} von {vergleichbar} Höhen.",
                abweichungen.len()
            );
            println!("     Zwei Maschinen haben aus denselben Blöcken verschiedene");
            println!("     Zustände errechnet. Das bricht den Konsens genauso wie ein");
            println!("     abweichendes Inferenzergebnis.");
            for a in abweichungen.iter().take(5) {
                println!("     Höhe {}:", a.hoehe);
                for (knoten, wurzel) in &a.wurzeln {
                    println!("       {knoten:<12} {wurzel}");
                }
            }
        }
    }

    // Transport und Lochstanzen: die Zahlen, für die es den
    // Mehrmaschinenlauf überhaupt gibt.
    let lochstanzversuche: u64 = bilder
        .iter()
        .map(|b| b.lochstanzen_gelungen + b.lochstanzen_gescheitert)
        .sum();
    println!();
    println!("  Verbindungen nach Transport:");
    for b in &bilder {
        println!(
            "    {:<12} QUIC {:>3}, TCP {:>3}, über Relais {:>3}",
            b.name, b.ueber_quic, b.ueber_tcp, b.ueber_relais
        );
    }
    if lochstanzversuche > 0 {
        let gelungen: u64 = bilder.iter().map(|b| b.lochstanzen_gelungen).sum();
        println!();
        println!(
            "  Lochstanzen (DCUtR): {gelungen} von {lochstanzversuche} gelungen."
        );
        for b in bilder.iter().filter(|b| b.lochstanzen_gescheitert > 0) {
            println!("    {}: {} gescheitert", b.name, b.lochstanzen_gescheitert);
        }
    } else if bilder.iter().any(|b| b.ueber_relais > 0) {
        println!();
        println!("  Kein Lochstanzversuch verzeichnet, obwohl über Relais verbunden.");
        println!("  Auf einer Maschine ist das erwartbar: Über Loopback gibt es");
        println!("  nichts zu durchstoßen. Über getrennte Anschlüsse wäre es ein Befund.");
    }

    // ⚑ Der Transport hängt an der Bootstrap-Adresse, nicht am Können
    // des Knotens. Wer eine TCP-Adresse verteilt, bekommt ein reines
    // TCP-Netz, auch wenn jeder Knoten QUIC spricht. Für das
    // Lochstanzen ist das der Unterschied zwischen „gelingt meistens"
    // und „gelingt selten", und ohne diesen Hinweis fiele es niemandem
    // auf: Das Netz läuft ja.
    let quic_gesamt: u64 = bilder.iter().map(|b| b.ueber_quic).sum();
    let tcp_gesamt: u64 = bilder.iter().map(|b| b.ueber_tcp).sum();
    if quic_gesamt == 0 && tcp_gesamt > 0 {
        println!();
        println!("  ⚠ Keine einzige Verbindung über QUIC, alle über TCP.");
        println!("    Der Transport folgt der Adresse, die verteilt wurde. Wer eine");
        println!("    /tcp/-Adresse als Einladung weitergibt, bekommt ein reines");
        println!("    TCP-Netz. Über UDP gelingt das Lochstanzen durch NAT deutlich");
        println!("    zuverlässiger; für einen Lauf über getrennte Anschlüsse gehört");
        println!("    deshalb die /udp/…/quic-v1-Adresse mit weitergegeben.");
    }

    let mit_latenz: Vec<&Knotenbild> = bilder.iter().filter(|b| b.latenz_max_us > 0).collect();
    if !mit_latenz.is_empty() {
        println!();
        println!("  Paarlatenz, zuletzt gemessene Spanne:");
        for b in &mit_latenz {
            println!(
                "    {:<12} {:>6} bis {:>6} µs{}",
                b.name,
                b.latenz_min_us,
                b.latenz_max_us,
                if b.latenz_max_us > b.latenz_min_us.saturating_mul(10) {
                    "   ⚠ starke Schwankung"
                } else {
                    ""
                }
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
            vermittelt: false, gesendete_digests: BTreeMap::new(),
            empfangene_digests: BTreeSet::new(), verworfen: BTreeMap::new(),
            mesh_groesse: 0, schlecht_bewertet: 0, peers_bei_aufnahme: 0,
            erreichbar: None, ueber_quic: 0, ueber_tcp: 0, ueber_relais: 0,
            lochstanzen_gelungen: 0, lochstanzen_gescheitert: 0,
            latenz_min_us: 0, latenz_max_us: 0, wurzeln: BTreeMap::new(),
            bloecke_erzeugt: 0, bloecke_uebernommen: 0,
            bloecke_abgelehnt: BTreeMap::new(), proben: BTreeMap::new(),
            proben_empfangen: BTreeMap::new(),
            verbindungszeit: BTreeMap::new(), atteste_angenommen: 0,
            atteste_verworfen: 0, bekannte_aussteller: 0, ende: None,
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
                gesendete_digests: BTreeMap::new(),
                empfangene_digests: BTreeSet::new(), verworfen: BTreeMap::new(),
                mesh_groesse: 1, schlecht_bewertet: 0, peers_bei_aufnahme: 1,
                erreichbar: None, ueber_quic: 0, ueber_tcp: 0, ueber_relais: 0,
                lochstanzen_gelungen: 0, lochstanzen_gescheitert: 0,
                latenz_min_us: 0, latenz_max_us: 0, wurzeln: BTreeMap::new(),
                bloecke_erzeugt: 0, bloecke_uebernommen: 0,
                bloecke_abgelehnt: BTreeMap::new(), proben: BTreeMap::new(),
                proben_empfangen: BTreeMap::new(),
                verbindungszeit: BTreeMap::new(), atteste_angenommen: 0,
                atteste_verworfen: 0, bekannte_aussteller: 0, ende: None,
            },
            Knotenbild {
                name: "b".into(), peer: "p2".into(), datei: PathBuf::new(),
                zeilen: 2, hoechste_folge: 2, erste_zeit_ms: 0, letzte_zeit_ms: 1,
                gesehen: ["p3".to_string()].into_iter().collect(),
                abgewiesen: 0, empfangen: 0, gesendet: 0, vermittelt: false,
                gesendete_digests: BTreeMap::new(),
                empfangene_digests: BTreeSet::new(), verworfen: BTreeMap::new(),
                mesh_groesse: 1, schlecht_bewertet: 0, peers_bei_aufnahme: 1,
                erreichbar: None, ueber_quic: 0, ueber_tcp: 0, ueber_relais: 0,
                lochstanzen_gelungen: 0, lochstanzen_gescheitert: 0,
                latenz_min_us: 0, latenz_max_us: 0, wurzeln: BTreeMap::new(),
                bloecke_erzeugt: 0, bloecke_uebernommen: 0,
                bloecke_abgelehnt: BTreeMap::new(), proben: BTreeMap::new(),
                proben_empfangen: BTreeMap::new(),
                verbindungszeit: BTreeMap::new(), atteste_angenommen: 0,
                atteste_verworfen: 0, bekannte_aussteller: 0, ende: None,
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
    fn eine_nachricht_wird_ueber_die_protokolle_verfolgt() {
        // Der Kern der Auswertung: A schickt, B empfängt, und der
        // Fingerabdruck verbindet beide Zeilen. Ohne ihn stünde in
        // beiden Dateien nur eine Bytezahl.
        let verz = temp("wege");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 150, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\""),
            zeile(3, 200, "a", "p1", "gesendet",
                  ",\"digest\":\"abc123\",\"bytes\":141,\"angenommen\":true"),
        ]);
        schreibe(&verz, "b", &[
            zeile(1, 100, "b", "p2", "start", ""),
            zeile(2, 150, "b", "p2", "verbunden", ",\"gegenstelle\":\"p1\""),
            zeile(3, 250, "b", "p2", "empfangen", ",\"digest\":\"abc123\",\"bytes\":141"),
        ]);
        let bilder = sammle(&verz).unwrap();
        let wege = nachrichtenwege(&bilder);
        assert_eq!(wege.len(), 1);
        assert_eq!(wege[0].digest, "abc123");
        assert_eq!(wege[0].absender, "a");
        assert_eq!(wege[0].empfaenger, vec!["b".to_string()]);
        assert!(wege[0].vollstaendig());
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn eine_nicht_angekommene_nachricht_benennt_wen_sie_nicht_erreichte() {
        let verz = temp("fehlweg");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 150, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\""),
            zeile(3, 200, "a", "p1", "gesendet",
                  ",\"digest\":\"deadbeef\",\"bytes\":10,\"angenommen\":true"),
        ]);
        schreibe(&verz, "b", &[
            zeile(1, 100, "b", "p2", "start", ""),
            zeile(2, 150, "b", "p2", "verbunden", ",\"gegenstelle\":\"p1\""),
        ]);
        let bilder = sammle(&verz).unwrap();
        let wege = nachrichtenwege(&bilder);
        assert_eq!(wege.len(), 1);
        assert!(!wege[0].vollstaendig());
        assert_eq!(wege[0].ohne_empfang, vec!["b".to_string()]);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn verworfene_nachrichten_werden_nach_grund_gezaehlt() {
        // Der Unterschied zwischen „nichts kam an" und „es kam an und
        // wurde weggeworfen" ist die erste Frage jeder Fehlersuche.
        let verz = temp("verworfen");
        schreibe(&verz, "b", &[
            zeile(1, 100, "b", "p2", "start", ""),
            zeile(2, 200, "b", "p2", "verworfen", ",\"grund\":\"nutzlastpruefung\""),
            zeile(3, 250, "b", "p2", "verworfen", ",\"grund\":\"nutzlastpruefung\""),
            zeile(4, 300, "b", "p2", "verworfen", ",\"grund\":\"transportregel\""),
        ]);
        let bilder = sammle(&verz).unwrap();
        assert_eq!(bilder[0].verworfen.get("nutzlastpruefung"), Some(&2));
        assert_eq!(bilder[0].verworfen.get("transportregel"), Some(&1));
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn ein_verbundener_knoten_ohne_mesh_faellt_auf() {
        // Die Lage, die im Protokoll am ehesten mit „das Netz war leer"
        // verwechselt wird: Verbindungen da, Mesh leer, nichts kommt an.
        let verz = temp("stumm");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 150, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\""),
            zeile(3, 200, "a", "p1", "aufnahme",
                  ",\"peers\":1,\"schlecht_bewertet\":0,\"mesh_blocks\":0,\"mesh_challenges\":0"),
        ]);
        let bilder = sammle(&verz).unwrap();
        assert!(bilder[0].stumm_im_mesh(), "leeres Mesh wurde nicht erkannt");
        assert_eq!(bilder[0].peers_bei_aufnahme, 1);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn ein_knoten_mit_mesh_gilt_nicht_als_stumm() {
        let verz = temp("mesh");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 150, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\""),
            zeile(3, 200, "a", "p1", "aufnahme",
                  ",\"peers\":1,\"schlecht_bewertet\":2,\"mesh_blocks\":3,\"mesh_challenges\":0"),
        ]);
        let bilder = sammle(&verz).unwrap();
        assert!(!bilder[0].stumm_im_mesh());
        assert_eq!(bilder[0].mesh_groesse, 3, "das größte Mesh zählt");
        assert_eq!(bilder[0].schlecht_bewertet, 2);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn wer_seine_peers_ueberlebt_gilt_nicht_als_stumm() {
        // Der Fehlalarm, den der erste echte Dreiknotenlauf erzeugt hat:
        // Alpha lief länger als die anderen, seine letzte Aufnahme
        // entstand allein. Über den Lauf gesehen hatte er zwei Peers,
        // in dem Moment keinen. Beide Zahlen müssen aus derselben
        // Aufnahme stammen.
        let verz = temp("ueberlebt");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 150, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\""),
            zeile(3, 200, "a", "p1", "aufnahme",
                  ",\"peers\":1,\"schlecht_bewertet\":0,\"mesh_blocks\":1"),
            zeile(4, 300, "a", "p1", "getrennt", ",\"gegenstelle\":\"p2\""),
            zeile(5, 400, "a", "p1", "aufnahme",
                  ",\"peers\":0,\"schlecht_bewertet\":0,\"mesh_blocks\":0"),
        ]);
        let bilder = sammle(&verz).unwrap();
        assert!(!bilder[0].gesehen.is_empty(), "der Lauf kannte einen Peer");
        assert_eq!(bilder[0].peers_bei_aufnahme, 0, "am Ende war er allein");
        assert!(
            !bilder[0].stumm_im_mesh(),
            "allein am Ende ist kein stummer Knoten, sondern ein Lauf, der endet"
        );
        std::fs::remove_dir_all(&verz).ok();
    }

    /// **Wer noch nicht lief, hat nichts verpasst.**
    ///
    /// Der erste Vierknotenlauf meldete 51 von 78 Nachrichten als nicht
    /// angekommen, fast alle an Knoten, die es zu dem Zeitpunkt noch
    /// nicht gab. Ein Bericht, der bei einem gesunden Lauf Alarm
    /// schlägt, wird nicht gelesen.
    #[test]
    fn ein_spaeter_gestarteter_knoten_gilt_nicht_als_verfehlt() {
        let verz = temp("nachzuegler-fehlalarm");
        // A sendet bei t=1000.
        schreibe(&verz, "a", &[
            zeile(1, 1_000, "a", "p1", "start", ""),
            zeile(2, 1_000, "a", "p1", "gesendet",
                  ",\"digest\":\"aaa111\",\"bytes\":10,\"angenommen\":true"),
            zeile(3, 900_000, "a", "p1", "ende", ",\"grund\":\"Laufzeit abgelaufen\""),
        ]);
        // B startet erst weit danach.
        schreibe(&verz, "b", &[
            zeile(1, 800_000, "b", "p2", "start", ""),
            zeile(2, 900_000, "b", "p2", "ende", ",\"grund\":\"Laufzeit abgelaufen\""),
        ]);
        let bilder = sammle(&verz).unwrap();
        let wege = nachrichtenwege(&bilder);
        assert_eq!(wege.len(), 1);
        assert!(
            wege[0].vollstaendig(),
            "der später gestartete Knoten wurde als Verfehlung gezählt: {:?}",
            wege[0].ohne_empfang
        );
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn wer_lief_und_nichts_bekam_wird_gemeldet() {
        // Die Gegenrichtung: Nachsicht darf nicht alles verschlucken.
        let verz = temp("echte-verfehlung");
        schreibe(&verz, "a", &[
            zeile(1, 1_000, "a", "p1", "start", ""),
            zeile(2, 2_000, "a", "p1", "gesendet",
                  ",\"digest\":\"bbb222\",\"bytes\":10,\"angenommen\":true"),
            zeile(3, 9_000, "a", "p1", "ende", ",\"grund\":\"Laufzeit abgelaufen\""),
        ]);
        schreibe(&verz, "b", &[
            zeile(1, 1_000, "b", "p2", "start", ""),
            zeile(2, 9_000, "b", "p2", "ende", ",\"grund\":\"Laufzeit abgelaufen\""),
        ]);
        let bilder = sammle(&verz).unwrap();
        let wege = nachrichtenwege(&bilder);
        assert_eq!(wege.len(), 1);
        assert!(!wege[0].vollstaendig(), "die echte Verfehlung wurde verschluckt");
        assert_eq!(wege[0].ohne_empfang, vec!["b".to_string()]);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn attestpruefungen_werden_gezaehlt() {
        // A10: Erst wenn beide Zahlen dastehen, ist der Prüfpfad belegt.
        let verz = temp("attest");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 110, "a", "p1", "validatorsatz",
                  ",\"bekannte_aussteller\":3,\"atteste_pruefbar\":true"),
            zeile(3, 200, "a", "p1", "attest_angenommen", ",\"bytes\":176"),
            zeile(4, 210, "a", "p1", "attest_angenommen", ",\"bytes\":176"),
            zeile(5, 220, "a", "p1", "attest_verworfen",
                  ",\"bytes\":176,\"grund\":\"nutzlastpruefung\""),
        ]);
        let b = &sammle(&verz).unwrap()[0];
        assert_eq!(b.atteste_angenommen, 2);
        assert_eq!(b.atteste_verworfen, 1);
        assert_eq!(b.bekannte_aussteller, 3);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn ein_knoten_ohne_validatorsatz_faellt_auf() {
        // Er verwirft jedes Attest, und das ist fast immer eine
        // fehlende Teilnehmerliste, kein Angriff.
        let verz = temp("kein-satz");
        schreibe(&verz, "a", &[zeile(1, 100, "a", "p1", "start", "")]);
        assert_eq!(sammle(&verz).unwrap()[0].bekannte_aussteller, 0);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn ein_abschluss_wird_mit_grund_erkannt() {
        let verz = temp("ende");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 200, "a", "p1", "ende", ",\"grund\":\"Abbruchsignal\",\"hoehe\":3"),
        ]);
        assert_eq!(sammle(&verz).unwrap()[0].ende.as_deref(), Some("Abbruchsignal"));
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn ein_protokoll_ohne_abschluss_faellt_auf() {
        // Abgestürzt, hart abgeschossen, oder läuft noch. Ohne diese
        // Unterscheidung müsste sie geraten werden.
        let verz = temp("kein-ende");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 200, "a", "p1", "aufnahme", ",\"peers\":1"),
        ]);
        assert_eq!(sammle(&verz).unwrap()[0].ende, None);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn proben_werden_nach_kennung_gezaehlt() {
        let verz = temp("proben");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 150, "a", "p1", "probe", ",\"kennung\":\"poi-buendel\",\"gelungen\":true"),
            zeile(3, 160, "a", "p1", "probe", ",\"kennung\":\"poi-buendel\",\"gelungen\":true"),
            zeile(4, 170, "a", "p1", "probe", ",\"kennung\":\"challenge\",\"gelungen\":false"),
            zeile(5, 180, "a", "p1", "empfangen",
                  ",\"topic\":\"Challenges\",\"kennung\":\"challenge\",\"digest\":\"aa\",\"bytes\":10"),
        ]);
        let b = &sammle(&verz).unwrap()[0];
        assert_eq!(b.proben.get("poi-buendel"), Some(&(2, 0)));
        assert_eq!(b.proben.get("challenge"), Some(&(0, 1)));
        assert_eq!(b.proben_empfangen.get("challenge"), Some(&1));
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn eine_nie_gelaufene_probe_bleibt_leer() {
        // Der Fall, den die Abdeckungstabelle sichtbar machen soll:
        // Eine Probe, die nie lief, ist kein Erfolg.
        let verz = temp("proben-leer");
        schreibe(&verz, "a", &[zeile(1, 100, "a", "p1", "start", "")]);
        let b = &sammle(&verz).unwrap()[0];
        assert!(b.proben.is_empty());
        assert_eq!(b.proben.get("poi-buendel"), None);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn uebereinstimmende_zustandswurzeln_ergeben_keinen_befund() {
        let verz = temp("kette-gut");
        for (name, peer) in [("a", "p1"), ("b", "p2")] {
            schreibe(&verz, name, &[
                zeile(1, 100, name, peer, "start", ""),
                zeile(2, 150, name, peer, "verbunden", ",\"gegenstelle\":\"px\""),
                zeile(3, 200, name, peer, "block_uebernommen",
                      ",\"hoehe\":1,\"txs\":2,\"zustandswurzel\":\"aaaa1111\""),
                zeile(4, 300, name, peer, "block_uebernommen",
                      ",\"hoehe\":2,\"txs\":1,\"zustandswurzel\":\"bbbb2222\""),
            ]);
        }
        let bilder = sammle(&verz).unwrap();
        let (vergleichbar, abweichungen) = kettenabgleich(&bilder);
        assert_eq!(vergleichbar, 2, "beide Höhen waren vergleichbar");
        assert!(abweichungen.is_empty());
        assert_eq!(bilder[0].bloecke_uebernommen, 2);
        std::fs::remove_dir_all(&verz).ok();
    }

    /// **Der Befund, für den es den Kettenabgleich gibt.**
    #[test]
    fn eine_abweichende_zustandswurzel_wird_benannt() {
        let verz = temp("kette-schlecht");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 150, "a", "p1", "verbunden", ",\"gegenstelle\":\"p2\""),
            zeile(3, 200, "a", "p1", "block_erzeugt",
                  ",\"hoehe\":1,\"txs\":2,\"zustandswurzel\":\"aaaa1111\""),
        ]);
        schreibe(&verz, "b", &[
            zeile(1, 100, "b", "p2", "start", ""),
            zeile(2, 150, "b", "p2", "verbunden", ",\"gegenstelle\":\"p1\""),
            zeile(3, 200, "b", "p2", "block_uebernommen",
                  ",\"hoehe\":1,\"txs\":2,\"zustandswurzel\":\"ffff9999\""),
        ]);
        let bilder = sammle(&verz).unwrap();
        let (vergleichbar, abweichungen) = kettenabgleich(&bilder);
        assert_eq!(vergleichbar, 1);
        assert_eq!(abweichungen.len(), 1, "die Abweichung wurde nicht erkannt");
        assert_eq!(abweichungen[0].hoehe, 1);
        assert_eq!(abweichungen[0].wurzeln.len(), 2);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn eine_hoehe_die_nur_einer_kennt_zaehlt_nicht_als_geprueft() {
        // Sonst sähe ein Lauf, in dem nur der Erzeuger baute und niemand
        // übernahm, aus wie ein bestandener Abgleich.
        let verz = temp("kette-allein");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 200, "a", "p1", "block_erzeugt",
                  ",\"hoehe\":1,\"txs\":0,\"zustandswurzel\":\"aaaa1111\""),
        ]);
        schreibe(&verz, "b", &[zeile(1, 100, "b", "p2", "start", "")]);
        let bilder = sammle(&verz).unwrap();
        let (vergleichbar, abweichungen) = kettenabgleich(&bilder);
        assert_eq!(vergleichbar, 0, "eine einseitige Höhe ist nicht vergleichbar");
        assert!(abweichungen.is_empty());
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn transport_und_lochstanzen_werden_gezaehlt() {
        // Die Zahlen, für die es den Mehrmaschinenlauf gibt: Lochstanzen
        // gelingt über UDP zuverlässig, über TCP oft nicht. Wer wissen
        // will, ob QUIC seinen Platz verdient, zählt nach.
        let verz = temp("transport");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 150, "a", "p1", "verbunden",
                  ",\"gegenstelle\":\"p2\",\"vermittelt\":true,\"quic\":false"),
            zeile(3, 200, "a", "p1", "verbunden",
                  ",\"gegenstelle\":\"p3\",\"vermittelt\":false,\"quic\":true"),
            zeile(4, 250, "a", "p1", "lochstanzen",
                  ",\"gegenstelle\":\"p2\",\"gelungen\":true,\"grund\":\"direkt\""),
            zeile(5, 260, "a", "p1", "lochstanzen",
                  ",\"gegenstelle\":\"p3\",\"gelungen\":false,\"grund\":\"Attempts exceeded\""),
        ]);
        let b = &sammle(&verz).unwrap()[0];
        assert_eq!(b.ueber_relais, 1);
        assert_eq!(b.ueber_quic, 1);
        assert_eq!(b.ueber_tcp, 1);
        assert_eq!(b.lochstanzen_gelungen, 1);
        assert_eq!(b.lochstanzen_gescheitert, 1);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn ein_reines_tcp_netz_ist_erkennbar() {
        // Der Fall, der beim ersten Dreiknotenlauf auftrat und beinahe
        // durchgegangen wäre: Alles lief über TCP, weil die Einladung
        // eine TCP-Adresse war. Das Netz läuft, und die Messung, für die
        // QUIC im Stack ist, findet nicht statt.
        let verz = temp("nurtcp");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 150, "a", "p1", "verbunden",
                  ",\"gegenstelle\":\"p2\",\"vermittelt\":false,\"quic\":false"),
        ]);
        let b = &sammle(&verz).unwrap()[0];
        assert_eq!(b.ueber_quic, 0);
        assert_eq!(b.ueber_tcp, 1);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn eine_ruhige_aufnahme_loescht_die_latenzspanne_nicht() {
        // Am Ende eines Laufs steht oft eine Aufnahme ohne Messungen.
        // Würde sie die Spanne überschreiben, ginge genau die Auskunft
        // verloren, wegen der man sie liest.
        let verz = temp("latenz");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 200, "a", "p1", "aufnahme",
                  ",\"peers\":1,\"latenz_messungen\":4,\"latenz_min_us\":900,\"latenz_max_us\":48000"),
            zeile(3, 300, "a", "p1", "aufnahme", ",\"peers\":0,\"latenz_messungen\":0"),
        ]);
        let b = &sammle(&verz).unwrap()[0];
        assert_eq!(b.latenz_min_us, 900, "die Spanne wurde überschrieben");
        assert_eq!(b.latenz_max_us, 48000);
        std::fs::remove_dir_all(&verz).ok();
    }

    #[test]
    fn eine_fehlende_erreichbarkeit_wird_gemerkt() {
        let verz = temp("erreichbar");
        schreibe(&verz, "a", &[
            zeile(1, 100, "a", "p1", "start", ""),
            zeile(2, 200, "a", "p1", "erreichbarkeit",
                  ",\"addr\":\"/ip4/1.2.3.4/tcp/4150\",\"erreichbar\":false,\"grund\":\"timeout\""),
        ]);
        let bilder = sammle(&verz).unwrap();
        assert_eq!(bilder[0].erreichbar, Some(false));
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
