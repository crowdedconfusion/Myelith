//! Protokolle mehrerer Maschinen gegenüberstellen (Punkt 2.1).
//!
//! Das ist die Stelle, an der der Client seinen Zweck einlöst. Alles
//! davor erzeugt Protokolle; hier wird aus ihnen ein Urteil.
//!
//! ## Der Nachweis braucht zwei Aussagen, nicht eine
//!
//! Ein Cross-Hardware-Determinismus-Nachweis besteht aus:
//!
//! 1. **Die Maschinen sind verschieden.**
//! 2. **Das Ergebnis ist trotzdem bitgleich.**
//!
//! Nur beide zusammen tragen. Zwei gleiche Digests von **derselben**
//! Maschine belegen nichts: sie zeigen, dass ein Programm zweimal
//! dasselbe gerechnet hat, und das ist keine Aussage über Hardware.
//!
//! Deshalb **verweigert** dieses Modul ein positives Urteil, wenn alle
//! Protokolle denselben Hardware-Fingerabdruck tragen ([`Urteil::EineMaschine`]).
//! Das ist ein Akzeptanzkriterium und keine Höflichkeit:
//! Ein Werkzeug, das einen Nachweis vortäuscht, ist schlimmer als keines,
//! weil sein Ergebnis geglaubt wird.
//!
//! ## ⚑ Fund 105 (2026-08-30): Genau das tat es
//!
//! Der Absatz darüber stand seit dem ersten Tag hier, und die Prüfung
//! dazu war gebaut. Sie fragte nur das Falsche. Der Fingerabdruck lief
//! über **alle** erhobenen Felder, und darunter waren drei, die nicht die
//! Maschine beschreiben, sondern den **Bau**: `backends_compiled`,
//! `backends_rechnend`, `backend_selected`.
//!
//! Damit genügte ein zweiter `cargo build`:
//!
//! ```text
//! myl-test --name ref-bau konformitaet          # ohne Feature
//! cargo build --release --features cpu-simd
//! myl-test --name simd-bau konformitaet         # dieselbe CPU
//! myl-test vergleich
//!
//!    ref-bau    aarch64-macos-reference       894d8357ae92b5c1
//!    simd-bau   aarch64-macos-cpu-simd/neon   894d8357ae92b5c1
//!    Urteil: NACHWEIS
//!    Das ist der Cross-Hardware-Determinismus-Nachweis für diese Einstellung.
//! ```
//!
//! Ein Laptop, zwei Übersetzungen, und das Werkzeug bescheinigt eine
//! Aussage über Hardware. Nachgestellt am 2026-08-30, genau so.
//!
//! **Behoben durch eine Trennung, nicht durch eine weitere Prüfung.** Der
//! Fingerabdruck deckt seither nur `hardware::MASCHINENFELDER` ab; die
//! drei Bau-Felder bilden einen eigenen Wert. Damit zerfällt die Frage in
//! die zwei Fragen, die sie immer war:
//!
//! | Maschinen | Rechenpfade | Urteil |
//! |---|---|---|
//! | ≥ 2 | beliebig | [`Urteil::Nachweis`], der Cross-Hardware-Beleg |
//! | 1 | ≥ 2 | [`Urteil::Rechenpfad`], der Backend-Vergleich (TESTCLIENT 2.2) |
//! | 1 | 1 | [`Urteil::EineMaschine`], kein Beleg |
//!
//! Die mittlere Zeile ist kein Trostpreis: „Referenz und SIMD rechnen
//! bitgleich" ist eine Aussage, die das Projekt braucht. Sie ist nur eine
//! **andere** als „zwei Maschinen rechnen bitgleich", und die
//! Verwechslung war der Fehler.
//!
//! ## Was zuerst geprüft wird
//!
//! Vor jedem Digest-Vergleich steht der **Modellstand**. Weichen θ_v oder
//! der Artefakt-Digest ab, sind die Läufe unvergleichbar, und zwar
//! unabhängig davon, wie die Digests ausfallen: Bei verschiedenen
//! Modellen müssen sie verschieden sein. Ein solcher Befund als
//! „Determinismus verletzt" gemeldet wäre genau die Verwechslung, gegen
//! die es den Befehl `artefakte` gibt.
//!
//! ## Warum eigener JSON-Leser
//!
//! Gelesen wird ein Format, das dieses Programm selbst schreibt
//! ([`crate::logging`]): flache Objekte, Zeichenketten und Zahlen, sonst
//! nichts. Ein JSON-Crate wäre eine Abhängigkeit für einen Umfang, den
//! vierzig Zeilen abdecken. Der Leser ist streng: Eine Zeile, die er
//! nicht versteht, überspringt er, statt sie zu raten.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Ein eingelesenes Laufprotokoll.
#[derive(Debug, Clone, Default)]
pub struct Protokoll {
    pub datei: PathBuf,
    pub lauf: String,
    pub befehl: String,
    pub teilnehmer: String,
    pub einstellungen_id: String,
    /// Fingerabdruck der **Maschine**. Er allein trägt den Nachweis.
    pub fingerprint: String,
    /// Fingerabdruck des **Rechenpfads**, also des Baus.
    ///
    /// Leer bei Protokollen von vor dem 2026-08-30; dort steckte er im
    /// Maschinen-Fingerabdruck mit drin, und genau das war Fund 105.
    /// `fingerabdruck_schema` fängt diesen Fall ab.
    pub rechenpfad: String,
    /// Wie der Fingerabdruck gebildet wurde
    /// (`hardware::FINGERABDRUCK_SCHEMA`). Leer bei älteren Protokollen.
    pub schema: String,
    /// Architektur, Betriebssystem und Backend als Kurzform, für die Anzeige.
    pub hardware: String,
    pub theta_v: String,
    pub artefakt_digest: String,
    /// Vergleichswerte in der Reihenfolge des Protokolls.
    pub ergebnisse: Vec<(String, String)>,
    pub abgeschlossen: bool,
    pub erfolgreich: bool,
    /// Trägt das Protokoll einen ausdrücklichen Abbruchvermerk?
    ///
    /// Ergänzt `abgeschlossen`, ersetzt es nicht: Ein Abbruch durch
    /// Strg-C hinterlässt **keinen** Vermerk, weil der Prozess ohne
    /// `Drop` endet. Maßgeblich ist deshalb das Fehlen von
    /// `run_finished`; der Vermerk liefert nur den Grund dazu.
    pub abgebrochen: bool,
    /// Was in den Vergleichswert eingeht (`runs::DIGEST_UMFANG`).
    ///
    /// Leer bei Protokollen von vor Fund 36. Die gab es nur als eigene
    /// Proben, aber leer bleibt trotzdem von `logits+token` verschieden,
    /// und genau das soll es: Sie messen nicht dasselbe.
    pub digest_umfang: String,
    /// Welche Stufen der Konformitätslauf geprüft hat
    /// (`konformitaet::UMFANG_OP` oder `…_VOLL`).
    ///
    /// Leer bei Protokollen ohne Konformitätslauf. Zwei verschiedene
    /// Umfänge sind unvergleichbar, aus demselben Grund wie zwei
    /// verschiedene Modellstände: Es wurde nicht dasselbe gemessen.
    pub konformitaet_umfang: String,
}

impl Protokoll {
    /// Modellstand als ein Wert, die Größe, die vor jedem Digest-Vergleich
    /// übereinstimmen muss.
    fn modellstand(&self) -> (&str, &str, &str, &str) {
        // Der Digest-Umfang gehört hierher und nicht in eine eigene
        // Prüfung: Die Frage ist dieselbe, nämlich ob überhaupt dasselbe
        // gemessen wurde. Ein anderes Modell und ein anderer Umfang sind
        // beide **kein Hardware-Befund**, und beide werden dadurch
        // behoben, dass man sie angleicht und neu misst. Der
        // Konformitäts-Umfang gehört aus derselben Begründung dazu.
        (
            &self.theta_v,
            &self.artefakt_digest,
            &self.digest_umfang,
            &self.konformitaet_umfang,
        )
    }

    /// Bezeichnung für die Ausgabe: Name, sonst Hardware, sonst Datei.
    pub fn bezeichnung(&self) -> String {
        if !self.teilnehmer.is_empty() && self.teilnehmer != crate::logging::OHNE_NAME {
            return self.teilnehmer.clone();
        }
        if !self.hardware.is_empty() {
            return self.hardware.clone();
        }
        self.datei
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

impl Protokoll {
    /// Hat dieser Lauf den Testplan zu Ende gebracht?
    ///
    /// Maßgeblich ist der Abschlusseintrag `run_finished`, **nicht** das
    /// Fehlen eines Abbruchvermerks: Strg-C und ein geschlossenes Fenster
    /// beenden den Prozess, ohne dass noch etwas geschrieben wird. Die
    /// Datei sieht dann tadellos aus, jede Zeile ist vollständig, und nur
    /// die letzte fehlt. Genau darauf wird geprüft.
    ///
    /// `ok=false` zählt ebenfalls als unvollständig für den Nachweis: Der
    /// Lauf hat selbst gemeldet, dass etwas schiefging, und seine
    /// Vergleichswerte sollen dann keinen Determinismusbeleg tragen.
    pub fn vollstaendig(&self) -> bool {
        self.abgeschlossen && self.erfolgreich && !self.abgebrochen
    }

    /// Kurzer Grund, warum dieses Protokoll nicht vollständig ist.
    pub fn mangel(&self) -> Option<&'static str> {
        if self.abgebrochen {
            Some("abgebrochen")
        } else if !self.abgeschlossen {
            Some("ohne Abschluss (Strg-C, Fenster zu, Absturz)")
        } else if !self.erfolgreich {
            Some("mit Fehlern beendet")
        } else {
            None
        }
    }
}

/// Das Urteil über eine Gruppe vergleichbarer Läufe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Urteil {
    /// Digests gleich, Maschinen-Fingerabdrücke verschieden, Modellstand
    /// gleich.
    Nachweis,
    /// Digests gleich, **eine** Maschine, aber zwei Rechenpfade.
    ///
    /// Das ist TESTCLIENT 2.2, der Backend-Vergleich innerhalb einer
    /// Maschine, und er ist ein echtes Ergebnis: Referenz und `cpu-simd`
    /// rechnen bitgleich. Er ist nur **nicht** der
    /// Cross-Hardware-Nachweis, und [`Urteil::ist_nachweis`] bleibt
    /// deshalb `false`.
    ///
    /// ⚑ Bis zum 2026-08-30 fiel genau dieser Fall unter
    /// [`Urteil::Nachweis`], weil der Bau im Fingerabdruck steckte
    /// (Fund 105). Ein zweiter Bau auf demselben Rechner reichte für ein
    /// Urteil, das eine Aussage über Hardware traf.
    Rechenpfad,
    /// Digests gleich, aber alle Läufe stammen von derselben Maschine
    /// **und** demselben Rechenpfad. **Kein Nachweis**: siehe Modul-Doku.
    EineMaschine,
    /// Die Protokolle sind nach verschiedenen Verfahren gebildet, oder
    /// eines nennt sein Verfahren nicht.
    ///
    /// Unvergleichbar, und zwar **bevor** irgendetwas verglichen wird:
    /// Zwei `fingerprint_sha256` aus verschiedenen Client-Fassungen
    /// decken verschiedene Feldmengen ab. Sie unterscheiden sich dann
    /// auch auf derselben Maschine, und ein Urteil daraus wäre Fund 105
    /// über den Umweg zweier Fassungen.
    Fingerabdruckschema,
    /// θ_v oder Artefakt-Digest weichen ab. Unvergleichbar, und
    /// ausdrücklich **kein** Hardware-Befund.
    Modellstand,
    /// Digests weichen bei gleichem Modellstand ab. Der eigentliche Befund.
    Abweichung,
    /// Weniger als zwei Protokolle: es gibt nichts zu vergleichen.
    ZuWenig,
    /// Mindestens ein Protokoll deckt nicht denselben Testplan ab wie die
    /// anderen: abgebrochen, oder mit einer anderen Menge an
    /// Vergleichswerten.
    ///
    /// **Der gefährlichste der fünf Nicht-Nachweise**, weil er vorher
    /// als `Nachweis` durchging: Verglichen wird je Wert nur unter den
    /// Protokollen, die ihn **haben**. Ein Lauf, der nach dem ersten von
    /// sechs Prompts abbrach, stimmte damit in allem überein, was er noch
    /// erreicht hatte, und fehlte im Rest unbemerkt.
    Unvollstaendig,
}

impl Urteil {
    /// Trägt dieses Urteil den Nachweis?
    ///
    /// Nur [`Urteil::Nachweis`]. Alles andere ist entweder ein Befund oder
    /// eine unvollständige Messung, und in beiden Fällen darf ein Skript,
    /// das den Rückgabewert prüft, keinen Erfolg sehen.
    pub fn ist_nachweis(&self) -> bool {
        matches!(self, Urteil::Nachweis)
    }

    pub fn kurz(&self) -> &'static str {
        match self {
            Urteil::Nachweis => "NACHWEIS",
            Urteil::Rechenpfad => "RECHENPFAD-NACHWEIS (eine Maschine, zwei Pfade)",
            Urteil::EineMaschine => "KEIN NACHWEIS (eine Maschine)",
            Urteil::Fingerabdruckschema => "UNVERGLEICHBAR (Fingerabdruck-Verfahren)",
            Urteil::Modellstand => "UNVERGLEICHBAR (Modellstand)",
            Urteil::Abweichung => "ABWEICHUNG",
            Urteil::ZuWenig => "ZU WENIG PROTOKOLLE",
            Urteil::Unvollstaendig => "UNVOLLSTÄNDIG (Lauf nicht zu Ende)",
        }
    }
}

/// Eine Gruppe von Protokollen, die verglichen werden dürfen: gleicher
/// Befehl, gleiche Einstellungs-Kennung.
#[derive(Debug, Clone)]
pub struct Gruppe {
    pub befehl: String,
    pub einstellungen_id: String,
    pub protokolle: Vec<Protokoll>,
    pub urteil: Urteil,
    /// Vergleichswert je Name: Digest → Bezeichnungen der Läufe.
    pub werte: Vec<(String, BTreeMap<String, Vec<String>>)>,
}

/// Das Ergebnis des Einlesens, samt dem, was liegen blieb.
///
/// # Warum das gezählt wird
///
/// Der Koordinator sammelt **beide** Arten von Protokollen an einer
/// Stelle: Determinismusläufe und die Betriebsprotokolle eines
/// Netzlaufs. Diese Auswertung übergeht die Netzprotokolle schon von
/// jeher, aber **still**, und Stille ist hier die falsche Antwort. Wer
/// nicht sagt, dass er drei Dateien liegen lässt, lässt offen, ob sie
/// fehlen oder nicht dazugehören, und genau diese Frage stellt sich
/// sonst jemand um zwei Uhr nachts.
#[derive(Debug, Clone, Default)]
pub struct Einlesung {
    /// Die gelesenen Determinismus-Protokolle.
    pub protokolle: Vec<Protokoll>,
    /// Betriebsprotokolle eines Netzlaufs. Gehören in denselben Ordner
    /// und sind kein Fehler; sie werden mit `netz` ausgewertet.
    pub betriebsprotokolle: usize,
    /// `.jsonl`-Dateien, die weder das eine noch das andere sind.
    /// **Die sind einen Blick wert**: entweder beschädigt oder aus
    /// Versehen hier gelandet.
    pub sonstige: usize,
}

/// Liest alle `.jsonl` eines Verzeichnisses.
pub fn einlesen(dir: &Path) -> Result<Vec<Protokoll>, String> {
    Ok(einlesen_mit_bericht(dir)?.protokolle)
}

/// Wie [`einlesen`], meldet zusätzlich, was übergangen wurde.
pub fn einlesen_mit_bericht(dir: &Path) -> Result<Einlesung, String> {
    let eintraege =
        fs::read_dir(dir).map_err(|e| format!("{} nicht lesbar: {}", dir.display(), e))?;

    let mut pfade: Vec<PathBuf> = eintraege
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    pfade.sort();

    let mut ergebnis = Einlesung::default();
    for p in &pfade {
        match protokoll_lesen(p) {
            Some(prot) => ergebnis.protokolle.push(prot),
            // Die Unterscheidung ist der Punkt: Ein Netzprotokoll gehört
            // hierher und wird anderswo ausgewertet, alles Übrige nicht.
            None if crate::netz::ist_betriebsprotokoll(p) => {
                ergebnis.betriebsprotokolle += 1
            }
            None => ergebnis.sonstige += 1,
        }
    }
    Ok(ergebnis)
}

/// Liest ein einzelnes Protokoll. `None`, wenn die Datei keines ist.
pub fn protokoll_lesen(datei: &Path) -> Option<Protokoll> {
    let text = fs::read_to_string(datei).ok()?;
    let mut p = Protokoll {
        datei: datei.to_path_buf(),
        ..Default::default()
    };
    let mut gesehen = false;
    let mut arch = String::new();
    let mut os = String::new();
    let mut backend = String::new();

    for zeile in text.lines() {
        let Some(felder) = felder(zeile) else { continue };
        let hole = |name: &str| -> String {
            felder
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        match hole("kind").as_str() {
            "run_started" => {
                gesehen = true;
                p.lauf = hole("run");
                p.befehl = hole("command");
                p.teilnehmer = hole("teilnehmer");
                p.einstellungen_id = hole("einstellungen_id");
            }
            "hardware" => match hole("key").as_str() {
                "fingerprint_sha256" => p.fingerprint = hole("value"),
                "rechenpfad_sha256" => p.rechenpfad = hole("value"),
                "fingerabdruck_schema" => p.schema = hole("value"),
                "arch" => arch = hole("value"),
                "os" => os = hole("value"),
                "backend_selected" => backend = hole("value"),
                "digest_umfang" => p.digest_umfang = hole("value"),
                "konformitaet_umfang" => p.konformitaet_umfang = hole("value"),
                _ => {}
            },
            "artifact" => match hole("key").as_str() {
                "theta_v" => p.theta_v = hole("value"),
                "artefakt_digest" => p.artefakt_digest = hole("value"),
                // Ältere Protokolle trugen die Einstellungs-Kennung hier
                // statt in `run_started`. Sie sollen lesbar bleiben.
                "einstellungen_id" if p.einstellungen_id.is_empty() => {
                    p.einstellungen_id = hole("value")
                }
                _ => {}
            },
            "result" => p.ergebnisse.push((hole("name"), hole("digest"))),
            "run_finished" => {
                p.abgeschlossen = true;
                p.erfolgreich = hole("ok") == "true";
            }
            "run_aborted" => p.abgebrochen = true,
            _ => {}
        }
    }

    if !gesehen {
        return None;
    }
    p.hardware = [arch, os, backend]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    Some(p)
}

/// Gruppiert nach Befehl und Einstellungs-Kennung und urteilt je Gruppe.
pub fn gruppieren(protokolle: Vec<Protokoll>) -> Vec<Gruppe> {
    let mut nach_schluessel: BTreeMap<(String, String), Vec<Protokoll>> = BTreeMap::new();
    for p in protokolle {
        nach_schluessel
            .entry((p.befehl.clone(), p.einstellungen_id.clone()))
            .or_default()
            .push(p);
    }

    nach_schluessel
        .into_iter()
        .map(|((befehl, einstellungen_id), protokolle)| {
            let werte = werte_sammeln(&protokolle);
            let urteil = urteilen(&protokolle, &werte);
            Gruppe {
                befehl,
                einstellungen_id,
                protokolle,
                urteil,
                werte,
            }
        })
        .collect()
}

/// Sammelt je Vergleichswert, welcher Digest von welchen Läufen kam.
///
/// Die Namen behalten die Reihenfolge des ersten Protokolls: Ein Bericht,
/// dessen Zeilen bei jedem Aufruf anders sortiert sind, ist nicht diffbar.
fn werte_sammeln(protokolle: &[Protokoll]) -> Vec<(String, BTreeMap<String, Vec<String>>)> {
    let mut namen: Vec<String> = Vec::new();
    for p in protokolle {
        for (name, _) in &p.ergebnisse {
            if !namen.contains(name) {
                namen.push(name.clone());
            }
        }
    }

    namen
        .into_iter()
        .map(|name| {
            let mut nach_digest: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for p in protokolle {
                if let Some((_, digest)) = p.ergebnisse.iter().find(|(n, _)| *n == name) {
                    nach_digest
                        .entry(digest.clone())
                        .or_default()
                        .push(p.bezeichnung());
                }
            }
            (name, nach_digest)
        })
        .collect()
}

fn urteilen(
    protokolle: &[Protokoll],
    werte: &[(String, BTreeMap<String, Vec<String>>)],
) -> Urteil {
    if protokolle.len() < 2 {
        return Urteil::ZuWenig;
    }

    // ⚑ **Das Verfahren zuerst, vor allem anderen** (Fund 105,
    // 2026-08-30). Ein `fingerprint_sha256` aus einer älteren
    // Client-Fassung deckt eine andere Feldmenge ab als einer von heute.
    // Beide sind 64 Hexzeichen lang, beide sehen richtig aus, und auf
    // derselben Maschine sind sie verschieden. Wer sie gegeneinander
    // hält, bekommt „zwei Maschinen" gemeldet und hat eine.
    let schemata: std::collections::BTreeSet<&str> =
        protokolle.iter().map(|p| p.schema.as_str()).collect();
    if schemata.len() > 1 || schemata.contains("") {
        return Urteil::Fingerabdruckschema;
    }
    // **Die Marke verspricht beide Werte.** Ein Protokoll, das sie trägt
    // und trotzdem einen davon nicht hat, ist von Hand verändert oder
    // beschädigt. Ohne diese Zeile könnte `maschinen` leer sein, und das
    // Urteil hieße dann „eine Maschine, zwei Pfade", ohne dass auch nur
    // eine Maschine benannt wäre.
    if protokolle
        .iter()
        .any(|p| p.fingerprint.is_empty() || p.rechenpfad.is_empty())
    {
        return Urteil::Fingerabdruckschema;
    }

    // Modellstand zuerst: Bei verschiedenen Modellen sagt ein
    // Digest-Vergleich nichts, weder im Guten noch im Schlechten.
    let erster = protokolle[0].modellstand();
    if protokolle.iter().any(|p| p.modellstand() != erster) {
        return Urteil::Modellstand;
    }

    // Ein Vergleichswert, der von zwei Läufen verschieden gerechnet wurde,
    // ist der Befund, für den es dieses Werkzeug gibt.
    let abweichung = werte
        .iter()
        .any(|(_, nach_digest)| nach_digest.len() > 1);
    if abweichung {
        return Urteil::Abweichung;
    }

    // Gleiche Digests von einer einzigen Maschine auf einem einzigen
    // Rechenpfad sind kein Nachweis, und zwar von keiner Art.
    let maschinen: std::collections::BTreeSet<&str> = protokolle
        .iter()
        .map(|p| p.fingerprint.as_str())
        .filter(|f| !f.is_empty())
        .collect();
    let pfade: std::collections::BTreeSet<&str> = protokolle
        .iter()
        .map(|p| p.rechenpfad.as_str())
        .filter(|f| !f.is_empty())
        .collect();
    if maschinen.len() < 2 && pfade.len() < 2 {
        return Urteil::EineMaschine;
    }

    // **Erst hier, und zwar nach Abweichung.** Ein abgebrochener Lauf,
    // dessen erreichte Werte bereits auseinandergehen, ist ein Befund und
    // bleibt einer; ihn als „unvollständig" abzutun, hieße den wichtigsten
    // Fall wegzuräumen. Umgekehrt darf Übereinstimmung auf einem
    // Bruchteil des Plans nie als Nachweis durchgehen.
    if protokolle.iter().any(|p| !p.vollstaendig()) {
        return Urteil::Unvollstaendig;
    }

    // Gleiche Länge genügt nicht, es muss dieselbe Menge sein: Zwei Läufe
    // mit je sechs Werten, von denen sich zwei unterscheiden, hätten in
    // vier Werten übereingestimmt und in den beiden anderen gar nicht
    // verglichen werden können.
    let namen = |p: &Protokoll| -> std::collections::BTreeSet<String> {
        p.ergebnisse.iter().map(|(n, _)| n.clone()).collect()
    };
    let erste = namen(&protokolle[0]);
    if protokolle.iter().any(|p| namen(p) != erste) {
        return Urteil::Unvollstaendig;
    }

    // **Erst hier trennen sich die beiden Aussagen.** Zwei Maschinen
    // tragen den Cross-Hardware-Nachweis. Eine Maschine mit zwei
    // Rechenpfaden trägt ihn nicht, aber sie trägt etwas anderes, das
    // ebenfalls gebraucht wird: dass Referenz und SIMD bitgleich
    // rechnen. Bis zum 2026-08-30 war beides dasselbe Urteil.
    if maschinen.len() >= 2 {
        Urteil::Nachweis
    } else {
        Urteil::Rechenpfad
    }
}

/// Schreibt den Bericht und liefert das Gesamturteil.
///
/// `true` nur, wenn **jede** Gruppe den Nachweis trägt. Eine Gruppe, die
/// ihn nicht trägt, zieht das Gesamturteil herunter: sonst könnte ein
/// gelungener Determinismuslauf einen fehlgeschlagenen Shard-Lauf decken.
pub fn berichten(dir: &Path, gruppen: &[Gruppe]) -> bool {
    println!("  Protokolle aus {}\n", dir.display());

    if gruppen.is_empty() {
        println!("  Keine Protokolle gefunden.");
        println!("  Erwartet werden `.jsonl`-Dateien, wie sie jeder Testlauf schreibt.");
        return false;
    }

    let mut alles_gut = true;
    for g in gruppen {
        println!(
            "  ── {} · Einstellungen {} · {} Protokolle ──",
            g.befehl,
            g.einstellungen_id,
            g.protokolle.len()
        );
        for p in &g.protokolle {
            println!(
                "     {:<16} {:<28} θ_v {:<8} M {} P {} {}",
                p.bezeichnung(),
                if p.hardware.is_empty() { "" } else { &p.hardware },
                if p.theta_v.is_empty() { "" } else { &p.theta_v },
                // **Beide Fingerabdrücke, getrennt beschriftet.** Das
                // Urteil hängt seit dem 2026-08-30 an zwei Werten, und wer
                // nur einen sieht, kann „Nachweis" nicht von
                // „Rechenpfad-Nachweis" unterscheiden. M wie Maschine,
                // P wie Pfad.
                kurz(&p.fingerprint),
                kurz(&p.rechenpfad),
                // Der Mangel steht in derselben Zeile wie der Lauf, nicht
                // in einer Fußnote: Wer die Tabelle überfliegt, soll nicht
                // erst unten erfahren, dass eine Zeile nichts wert ist.
                match p.mangel() {
                    Some(m) => format!("  ⚠ {} ({} Werte)", m, p.ergebnisse.len()),
                    None => String::new(),
                },
            );
        }
        println!();

        for (name, nach_digest) in &g.werte {
            // **Wie viele Läufe haben diesen Wert überhaupt?** Ein Wert,
            // den nur einer geliefert hat, ist mit niemandem verglichen
            // worden. Ohne diesen Zusatz stand davor ein „=", und das
            // liest sich wie Übereinstimmung, wo gar kein Vergleich
            // stattgefunden hat.
            let beitragende: usize = nach_digest.values().map(|v| v.len()).sum();
            let anteil = if beitragende < g.protokolle.len() {
                format!("   (nur {} von {} Läufen)", beitragende, g.protokolle.len())
            } else {
                String::new()
            };
            if nach_digest.len() == 1 {
                let digest = nach_digest.keys().next().map(String::as_str).unwrap_or("");
                let zeichen = if beitragende < g.protokolle.len() { "·" } else { "=" };
                println!("     {} {:<24} {}{}", zeichen, name, kurz(digest), anteil);
            } else {
                println!("     ≠ {:<24} {} verschiedene Werte:", name, nach_digest.len());
                for (digest, laeufe) in nach_digest {
                    println!("         {}  {}", kurz(digest), laeufe.join(", "));
                }
            }
        }

        println!("\n     Urteil: {}", g.urteil.kurz());
        for zeile in erlaeuterung(&g.urteil).lines() {
            println!("     {}", zeile);
        }
        // Die Einengung steht direkt unter dem Befund und nicht im
        // Bericht allein: Wer auf einer Mietmaschine sitzt, liest den
        // Bildschirm und nicht die Datei.
        if g.urteil == Urteil::Abweichung {
            println!();
            for zeile in abweichungs_hinweis(g).lines() {
                println!("     {}", zeile);
            }
        }
        println!();

        alles_gut &= g.urteil.ist_nachweis();
    }
    alles_gut
}

/// Was das Urteil bedeutet und was als Nächstes zu tun ist.
///
/// Der Text ist der eigentliche Nutzen des Befehls: Ein Urteilswort ohne
/// Folgerung lädt dazu ein, es falsch zu lesen: besonders
/// `EineMaschine`, das wie ein Erfolg aussieht, und `Modellstand`, das
/// wie ein Fehlschlag aussieht.
fn erlaeuterung(u: &Urteil) -> &'static str {
    match u {
        Urteil::Nachweis => {
            "Die MASCHINEN-Fingerabdrücke unterscheiden sich, die Vergleichswerte\n\
             stimmen überein. Das ist der Cross-Hardware-Determinismus-Nachweis für\n\
             diese Einstellung."
        }
        Urteil::Rechenpfad => {
            "Eine Maschine, zwei Rechenpfade, gleiche Vergleichswerte. Referenz und\n\
             SIMD-Bau rechnen bitgleich; das ist der Backend-Vergleich und ein\n\
             eigenständiges Ergebnis.\n\
             \n\
             Es ist NICHT der Cross-Hardware-Nachweis: Alle Läufe stammen von\n\
             derselben CPU. Was hier belegt ist, ist die Gleichheit zweier\n\
             Codepfade, nicht die Gleichheit zweier Maschinen. Dafür fehlt weiterhin\n\
             eine zweite Architektur.\n\
             \n\
             Bis zum 2026-08-30 meldete dieser Fall „NACHWEIS\", weil der Bau in den\n\
             Fingerabdruck einging. Ein zweiter `cargo build` genügte für ein Urteil\n\
             über Hardware."
        }
        Urteil::Fingerabdruckschema => {
            "Die Protokolle bilden ihren Fingerabdruck nach verschiedenen Verfahren,\n\
             oder eines nennt sein Verfahren nicht. Sie sind unvergleichbar, und zwar\n\
             bevor irgendein Wert angesehen wird.\n\
             \n\
             Zwei Fingerabdrücke aus verschiedenen Client-Fassungen decken\n\
             verschiedene Feldmengen ab. Sie unterscheiden sich dann auch auf\n\
             derselben Maschine, und der Vergleich meldete „zwei Maschinen\", wo eine\n\
             steht. Alle Beteiligten auf denselben Client-Stand bringen und neu\n\
             messen."
        }
        Urteil::EineMaschine => {
            "Alle Protokolle tragen denselben Maschinen-Fingerabdruck UND denselben\n\
             Rechenpfad.\n\
             Gleiche Werte belegen hier nichts: Sie zeigen, dass dasselbe Programm auf\n\
             derselben Maschine zweimal dasselbe gerechnet hat. Es fehlt eine zweite\n\
             Architektur, nicht ein weiterer Lauf."
        }
        Urteil::Modellstand => {
            "θ_v, der Artefakt-Digest, der Digest-Umfang oder der\n\
             Konformitäts-Umfang weichen zwischen den Läufen ab. Das ist KEIN\n\
             Hardware-Befund. Hier wurde gegen verschiedene Modelle oder mit\n\
             verschiedenen Messverfahren gerechnet; ein Bitgleichheitstest darüber\n\
             hätte keine Aussage.\n\
             \n\
             Bei abweichendem Modell: `myl-test artefakte` auf allen Maschinen\n\
             gleichziehen, dann erneut messen.\n\
             Bei abweichendem Digest-Umfang: Ein Protokoll stammt aus einer älteren\n\
             Fassung des Clients, die nur die erzeugten Token gehasht hat und damit\n\
             kleine Rechenabweichungen nicht sehen konnte (Fund 36). Alle Beteiligten\n\
             auf denselben Stand bringen und neu messen.\n\
             Bei abweichendem Konformitäts-Umfang: Ein Lauf hat nur die\n\
             Operations-Vektoren geprüft (ohne passendes Artefakt), der andere auch\n\
             Layer und E2E. Beide Seiten mit demselben Artefakt laufen lassen."
        }
        Urteil::Abweichung => {
            "Gleicher Modellstand, gleiche Eingabe, verschiedene Ergebnisse.\n\
             Das ist ein Befund an der Kernthese des Projekts und der wichtigste Fall,\n\
             den dieses Werkzeug finden kann. Protokolle sichern und melden."
        }
        Urteil::ZuWenig => {
            "Für einen Vergleich braucht es mindestens zwei Protokolle mit derselben\n\
             Einstellungs-Kennung. Weichen die Kennungen ab, liefen verschiedene\n\
             Parameter, dann ist die Eingabe zu vereinheitlichen, nicht das Ergebnis."
        }
        Urteil::Unvollstaendig => {
            "Mindestens ein Lauf deckt nicht denselben Testplan ab wie die anderen:\n\
             abgebrochen, mit Fehlern beendet, oder mit einer anderen Menge an\n\
             Vergleichswerten. Oben steht bei den betroffenen Protokollen, woran es liegt.\n\
             \n\
             Verglichen wird je Wert nur unter den Läufen, die ihn haben. Ein Lauf, der\n\
             nach einem von sechs Prompts endete, stimmt deshalb in allem überein, was er\n\
             erreicht hat, und fehlt im Rest, ohne dass es auffiele. Die Übereinstimmung\n\
             wäre echt und die Aussage trotzdem falsch.\n\
             \n\
             Den betroffenen Lauf wiederholen. Er kostet dieselbe Zeit wie beim ersten Mal."
        }
    }
}

/// Grenzt eine Abweichung ein, aus dem, was schon im Protokoll steht.
///
/// # Wozu
///
/// `Urteil::Abweichung` ist der wichtigste Befund dieses Werkzeugs und
/// zugleich der unbrauchbarste Satz: „die Vergleichswerte gehen
/// auseinander" sagt nicht, **wo**. Auf einer Mietmaschine, die
/// stündlich abgerechnet wird, ist das der Unterschied zwischen einem
/// Befund und einem verlorenen Nachmittag.
///
/// # Warum das ohne einen zweiten Lauf geht
///
/// Der Sammellauf trägt seit dem 2026-08-27 den Konformitätslauf als
/// fünfte Stufe, und dessen Wert steht als eigener Vergleichswert im
/// selben Protokoll. Damit liegt die Einengung bereits vor:
///
/// - Weichen **die Konformitätsvektoren** ab, sitzt der Unterschied
///   unterhalb des Modells, in den Kerneln selbst.
/// - Stimmen sie überein und weicht nur der Modelllauf ab, rechnen die
///   Kernel gleich, und der Unterschied liegt darüber: Artefakt, Laden,
///   Zuschnitt, Abtastung.
///
/// Das ist keine Vermutung, sondern eine Fallunterscheidung über zwei
/// Werte, die beide schon gemessen wurden.
pub fn abweichungs_hinweis(g: &Gruppe) -> String {
    let geteilt = |name: &str| -> bool {
        g.werte
            .iter()
            .any(|(n, nach_digest)| n == name && nach_digest.len() > 1)
    };
    let vorhanden = |name: &str| -> bool { g.werte.iter().any(|(n, _)| n == name) };

    let abweichende: Vec<&str> = g
        .werte
        .iter()
        .filter(|(_, nach_digest)| nach_digest.len() > 1)
        .map(|(n, _)| n.as_str())
        .collect();

    // Zeilenliste statt Zeilenfortsetzung im Literal: Der Text steht
    // links am Rand und traegt die Einrueckung des Quelltexts nicht mit
    // in die Ausgabe.
    let mut zeilen: Vec<String> = vec![format!(
        "Auseinander gehen: {}.",
        if abweichende.is_empty() {
            "nichts".to_string()
        } else {
            abweichende.join(", ")
        }
    )];

    let konf = crate::konformitaet::WERT;
    if !vorhanden(konf) {
        zeilen.extend([
            String::new(),
            "Diese Protokolle tragen keinen Konformitätswert, deshalb lässt sich die".into(),
            "Abweichung hier nicht weiter eingrenzen. Ein Sammellauf (`myl-test` ohne".into(),
            "Unterbefehl) führt die Konformitätsvektoren als fünfte Stufe mit; damit".into(),
            "trennt der nächste Vergleich Kernel von Modell.".into(),
        ]);
        return zeilen.join("\n");
    }

    if geteilt(konf) {
        zeilen.extend([
            String::new(),
            "Die Konformitätsvektoren selbst weichen ab. Der Unterschied sitzt damit".into(),
            "UNTERHALB des Modells, in den Kerneln: eine feste Eingabe, ein fester".into(),
            "erwarteter Wert, und zwei Maschinen rechnen verschieden.".into(),
            String::new(),
            "Nächster Schritt: `myl-test konformitaet` auf beiden Maschinen. Der Lauf".into(),
            "schreibt eine Protokollzeile JE VEKTOR und benennt damit die Operation.".into(),
        ]);
        // Der Umfang entscheidet, wie fein die Einengung ausfällt.
        let nur_op = g
            .protokolle
            .iter()
            .all(|p| p.konformitaet_umfang == crate::konformitaet::UMFANG_OP);
        if nur_op {
            zeilen.extend([
                String::new(),
                "Beide Läufe hatten nur die Operations-Vektoren (kein Artefakt gewählt)."
                    .into(),
                "Mit Artefakt kommen Layer- und E2E-Vektoren dazu, und die grenzen von der"
                    .into(),
                "Operation auf die Schicht ein.".into(),
            ]);
        }
        return zeilen.join("\n");
    }

    zeilen.extend([
        String::new(),
        "Die Konformitätsvektoren stimmen überein. Die Kernel rechnen auf beiden".into(),
        "Maschinen bitgleich; der Unterschied liegt DARÜBER: Artefakt, Laden des".into(),
        "Modells, Zuschnitt der Shards oder Abtastung.".into(),
        String::new(),
        "Nächster Schritt: `myl-test artefakte` auf beiden Maschinen. Weicht schon der".into(),
        "Artefakt-Digest ab, ist es kein Hardware-Befund, sondern ein anderes Modell.".into(),
    ]);
    zeilen.join("\n")
}

fn kurz(digest: &str) -> &str {
    &digest[..16.min(digest.len())]
}

/// Ablage der zugesandten Teilnehmerprotokolle, relativ zur Repository-Wurzel.
pub const ORDNER: &str = "TESTCLIENT/Vergleiche";
/// Ablage der Vergleichsberichte.
pub const BERICHTE: &str = "TESTCLIENT/Vergleiche/Berichte";

/// Wo der Koordinator die zugesandten Protokolle ablegt.
pub fn vergleichsordner(repo: &Path) -> PathBuf {
    repo.join(ORDNER)
}

/// Wo die Berichte hingeschrieben werden.
///
/// Ein **Unterordner** der Eingabe, nicht ihr Geschwister: Läge der
/// Bericht neben den Protokollen, hätte der nächste Aufruf ihn als
/// Eingabe mitgelesen. Er trägt zwar keine `.jsonl`-Endung und wäre
/// deshalb heute unschädlich, aber das ist eine Eigenschaft des
/// Dateinamens, keine des Verfahrens, und darauf soll sich niemand
/// verlassen müssen.
pub fn berichtsordner(repo: &Path) -> PathBuf {
    repo.join(BERICHTE)
}

/// Schreibt den ausführlichen Bericht und liefert seinen Pfad.
///
/// Markdown, weil der Bericht weitergereicht wird: an Mitwirkende, in
/// Tickets, gelegentlich in ein `eval/results/`-Verzeichnis. Er enthält
/// dieselben Angaben wie die Bildschirmausgabe und zusätzlich, was dort
/// keinen Platz hat: vollständige Digests statt Kurzform, Dateinamen,
/// Zeitpunkt des Vergleichs.
pub fn bericht_schreiben(
    ziel: &Path,
    quelle: &Path,
    gruppen: &[Gruppe],
) -> Result<PathBuf, String> {
    fs::create_dir_all(ziel).map_err(|e| format!("{} nicht anlegbar: {}", ziel.display(), e))?;

    let (datum, uhrzeit) = crate::logging::datum_und_uhrzeit();
    let pfad = ziel.join(format!("vergleich_{}_{}.md", datum, uhrzeit));
    fs::write(&pfad, bericht_text(quelle, &datum, &uhrzeit, gruppen))
        .map_err(|e| format!("{} nicht schreibbar: {}", pfad.display(), e))?;
    Ok(pfad)
}

/// Setzt den Berichtstext zusammen. Getrennt vom Schreiben, damit er ohne
/// Dateisystem prüfbar ist.
fn bericht_text(quelle: &Path, datum: &str, uhrzeit: &str, gruppen: &[Gruppe]) -> String {
    use std::fmt::Write as _;
    let mut t = String::new();

    let _ = writeln!(t, "# Vergleichsbericht {} {}", datum, uhrzeit);
    let _ = writeln!(t);
    let _ = writeln!(t, "**Quelle:** `{}`  ", quelle.display());
    let _ = writeln!(
        t,
        "**Protokolle:** {}  ",
        gruppen.iter().map(|g| g.protokolle.len()).sum::<usize>()
    );
    let _ = writeln!(t, "**Gruppen:** {}", gruppen.len());
    let _ = writeln!(t);

    if gruppen.is_empty() {
        let _ = writeln!(
            t,
            "Keine Protokolle gefunden. Erwartet werden `.jsonl`-Dateien, \
             wie sie jeder Testlauf schreibt."
        );
        return t;
    }

    let alles_gut = gruppen.iter().all(|g| g.urteil.ist_nachweis());
    let _ = writeln!(
        t,
        "**Gesamturteil:** {}",
        if alles_gut {
            "NACHWEIS über alle Gruppen"
        } else {
            "kein durchgehender Nachweis: siehe die Urteile je Gruppe"
        }
    );

    for g in gruppen {
        let _ = writeln!(t);
        let _ = writeln!(
            t,
            "## {} · Einstellungen `{}`",
            g.befehl, g.einstellungen_id
        );
        let _ = writeln!(t);
        let _ = writeln!(t, "### Beteiligte Läufe");
        let _ = writeln!(t);
        // Die Spalte „Lauf" steht **vor** den Digests. Ein Bericht wird
        // später gelesen als er entsteht, oft von jemandem, der beim Lauf
        // nicht dabei war; dass eine Zeile aus einem abgebrochenen
        // Durchgang stammt, gehört dann nicht ans Ende.
        let _ = writeln!(
            t,
            "| Teilnehmer | Lauf | Werte | Hardware | θ_v | Artefakt-Digest | Maschine | Rechenpfad | Datei |"
        );
        let _ = writeln!(t, "|---|---|---|---|---|---|---|---|---|");
        for p in &g.protokolle {
            let _ = writeln!(
                t,
                "| {} | {} | {} | {} | {} | `{}` | `{}` | `{}` | `{}` |",
                p.bezeichnung(),
                match p.mangel() {
                    Some(m) => format!("**{}**", m),
                    None => "vollständig".to_string(),
                },
                p.ergebnisse.len(),
                leer_als_strich(&p.hardware),
                leer_als_strich(&p.theta_v),
                kurz(&p.artefakt_digest),
                kurz(&p.fingerprint),
                kurz(&p.rechenpfad),
                p.datei
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            );
        }

        let _ = writeln!(t);
        let _ = writeln!(t, "### Vergleichswerte");
        let _ = writeln!(t);
        for (name, nach_digest) in &g.werte {
            let beitragende: usize = nach_digest.values().map(|v| v.len()).sum();
            if nach_digest.len() == 1 {
                let digest = nach_digest.keys().next().map(String::as_str).unwrap_or("");
                if beitragende < g.protokolle.len() {
                    let _ = writeln!(
                        t,
                        "- **{}**: NICHT VERGLICHEN, nur {} von {} Läufen haben \
                         diesen Wert: `{}`",
                        name, beitragende, g.protokolle.len(), digest
                    );
                } else {
                    let _ = writeln!(t, "- **{}**: übereinstimmend: `{}`", name, digest);
                }
            } else {
                let _ = writeln!(t, "- **{}**. ABWEICHUNG, {} verschiedene Werte:", name, nach_digest.len());
                for (digest, laeufe) in nach_digest {
                    let _ = writeln!(t, "  - `{}`: {}", digest, laeufe.join(", "));
                }
            }
        }

        let _ = writeln!(t);
        let _ = writeln!(t, "### Urteil: {}", g.urteil.kurz());
        let _ = writeln!(t);
        for zeile in erlaeuterung(&g.urteil).lines() {
            let _ = writeln!(t, "{}", zeile);
        }
        if g.urteil == Urteil::Abweichung {
            let _ = writeln!(t);
            let _ = writeln!(t, "#### Wo die Abweichung sitzt");
            let _ = writeln!(t);
            for zeile in abweichungs_hinweis(g).lines() {
                let _ = writeln!(t, "{}", zeile);
            }
        }
    }

    let _ = writeln!(t);
    let _ = writeln!(t, "---");
    let _ = writeln!(t);
    let _ = writeln!(
        t,
        "Dieser Bericht hält den Stand des Quellordners zum genannten \
         Zeitpunkt fest. Ein **bestätigter** Cross-Hardware-Nachweis gehört \
         nach `INTEGER_LLM/eval/results/` (Punkt 2.3), der \
         Berichtsordner wird nicht versioniert."
    );
    t
}

fn leer_als_strich(s: &str) -> &str {
    if s.is_empty() {
        ""
    } else {
        s
    }
}

/// `myl-test vergleich`: einlesen, gruppieren, berichten, festhalten.
pub fn run(dir: &Path, berichte: Option<&Path>) -> bool {
    let eingelesen = match einlesen_mit_bericht(dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("  {}", e);
            return false;
        }
    };
    if eingelesen.betriebsprotokolle > 0 {
        println!(
            "  {} Betriebsprotokoll(e) eines Netzlaufs übergangen. Sie gehören \
             hierher und werden im Entwickler-Menü unter „Netzlauf auswerten“ \
             betrachtet.",
            eingelesen.betriebsprotokolle
        );
    }
    if eingelesen.sonstige > 0 {
        println!(
            "  ⚠ {} .jsonl-Datei(en) sind weder Testlauf noch Netzprotokoll. \
             Entweder beschädigt oder aus Versehen hier gelandet.",
            eingelesen.sonstige
        );
    }
    let protokolle = eingelesen.protokolle;

    let gruppen = gruppieren(protokolle);
    let ok = berichten(dir, &gruppen);

    if let Some(ziel) = berichte {
        match bericht_schreiben(ziel, dir, &gruppen) {
            Ok(pfad) => println!("  Bericht: {}", pfad.display()),
            // Ein fehlgeschlagener Bericht darf das Urteil nicht kippen:
            // Es steht bereits auf dem Bildschirm und ist damit gefällt.
            Err(e) => eprintln!("  WARNUNG: Bericht nicht geschrieben: {}", e),
        }
    }
    ok
}

/// Zerlegt eine JSONL-Zeile in ihre Felder.
///
/// Gegenstück zu `logging::json_escape`. `None` bei allem, was nicht dem
/// selbst geschriebenen Format entspricht: lieber eine Zeile auslassen
/// als sie falsch deuten.
fn felder(zeile: &str) -> Option<Vec<(String, String)>> {
    let z: Vec<char> = zeile.trim().chars().collect();
    if z.first() != Some(&'{') || z.last() != Some(&'}') {
        return None;
    }

    let mut i = 1;
    let mut out = Vec::new();
    loop {
        while i < z.len() && (z[i] == ',' || z[i].is_whitespace()) {
            i += 1;
        }
        if i >= z.len() || z[i] == '}' {
            return Some(out);
        }

        let schluessel = lies_string(&z, &mut i)?;
        while i < z.len() && z[i].is_whitespace() {
            i += 1;
        }
        if z.get(i) != Some(&':') {
            return None;
        }
        i += 1;
        while i < z.len() && z[i].is_whitespace() {
            i += 1;
        }

        let wert = if z.get(i) == Some(&'"') {
            lies_string(&z, &mut i)?
        } else {
            // Zahl oder Wahrheitswert: bis zum nächsten Trenner.
            let start = i;
            while i < z.len() && z[i] != ',' && z[i] != '}' {
                i += 1;
            }
            z[start..i].iter().collect::<String>().trim().to_string()
        };
        out.push((schluessel, wert));
    }
}

fn lies_string(z: &[char], i: &mut usize) -> Option<String> {
    if z.get(*i) != Some(&'"') {
        return None;
    }
    *i += 1;
    let mut out = String::new();
    while *i < z.len() {
        match z[*i] {
            '"' => {
                *i += 1;
                return Some(out);
            }
            '\\' => {
                *i += 1;
                match z.get(*i)? {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    'u' => {
                        let hex: String = z.get(*i + 1..*i + 5)?.iter().collect();
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                        *i += 4;
                    }
                    _ => return None,
                }
                *i += 1;
            }
            c => {
                out.push(c);
                *i += 1;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{Event, LogZiel, RunLog};

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("myl-testclient-vergleich-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// Schreibt ein Protokoll, wie ein echter Lauf es hinterlässt.
    fn protokoll_schreiben(
        dir: &Path,
        teilnehmer: &str,
        einstellungen: &str,
        arch: &str,
        fingerprint: &str,
        theta_v: &str,
        digest: &str,
    ) {
        let mut log = RunLog::mit_ziel(
            LogZiel::neu(dir, "determinismus", teilnehmer, einstellungen, arch),
            false,
        );
        log.event(Event::Hardware {
            key: "arch".into(),
            value: arch.into(),
        });
        log.event(Event::Hardware {
            key: "fingerprint_sha256".into(),
            value: fingerprint.into(),
        });
        // Die Proben stellen Protokolle des heutigen Clients nach: Ohne
        // die Schema-Marke fielen sie alle unter
        // `Urteil::Fingerabdruckschema`, und zwar zu Recht.
        log.event(Event::Hardware {
            key: "fingerabdruck_schema".into(),
            value: crate::hardware::FINGERABDRUCK_SCHEMA.into(),
        });
        log.event(Event::Hardware {
            key: "rechenpfad_sha256".into(),
            value: "pfad-referenz".into(),
        });
        log.event(Event::Artifact {
            key: "theta_v".into(),
            value: theta_v.into(),
        });
        log.event(Event::Artifact {
            key: "artefakt_digest".into(),
            value: "c42bb8a8d85bba5a".into(),
        });
        log.result("determinismus", digest, "bitgleich");
        log.finish(true);
    }

    /// Schreibt ein Protokoll mit mehreren Vergleichswerten und, je nach
    /// `abschluss`, mit oder ohne Abschlusseintrag.
    ///
    /// `abschluss = false` bildet den Strg-C-Fall nach: Die Datei bleibt
    /// vollständig bis zur letzten geschriebenen Zeile, es fehlt nur der
    /// Abschluss. `RunLog` wird dafür mit `std::mem::forget` stehen
    /// gelassen, weil `Drop` sonst einen Abbruchvermerk schriebe, den ein
    /// echtes SIGINT nie hinterlässt.
    fn lauf_mit_werten(
        dir: &Path,
        teilnehmer: &str,
        arch: &str,
        fingerprint: &str,
        werte: &[(&str, &str)],
        abschluss: bool,
    ) {
        let mut log = RunLog::mit_ziel(
            LogZiel::neu(dir, "determinismus", teilnehmer, "abcd1234", arch),
            false,
        );
        log.event(Event::Hardware {
            key: "arch".into(),
            value: arch.into(),
        });
        log.event(Event::Hardware {
            key: "fingerprint_sha256".into(),
            value: fingerprint.into(),
        });
        // Die Proben stellen Protokolle des heutigen Clients nach: Ohne
        // die Schema-Marke fielen sie alle unter
        // `Urteil::Fingerabdruckschema`, und zwar zu Recht.
        log.event(Event::Hardware {
            key: "fingerabdruck_schema".into(),
            value: crate::hardware::FINGERABDRUCK_SCHEMA.into(),
        });
        log.event(Event::Hardware {
            key: "rechenpfad_sha256".into(),
            value: "pfad-referenz".into(),
        });
        log.event(Event::Artifact {
            key: "theta_v".into(),
            value: "0.17.0".into(),
        });
        log.event(Event::Artifact {
            key: "artefakt_digest".into(),
            value: "c42bb8a8d85bba5a".into(),
        });
        for (name, digest) in werte {
            log.result(name, digest, "bitgleich");
        }
        if abschluss {
            log.finish(true);
        } else {
            std::mem::forget(log);
        }
    }

    fn urteil_ueber(dir: &Path) -> Urteil {
        let gruppen = gruppieren(einlesen(dir).expect("lesbar"));
        assert_eq!(gruppen.len(), 1, "eine Gruppe erwartet");
        gruppen[0].urteil.clone()
    }

    /// Der Regelfall, für den es das Werkzeug gibt: zwei Architekturen,
    /// gleicher Digest.
    #[test]
    fn zwei_architekturen_gleicher_digest_ist_der_nachweis() {
        let dir = tempdir("nachweis");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-x");
        assert_eq!(urteil_ueber(&dir), Urteil::Nachweis);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Das Akzeptanzkriterium: Zwei Läufe von derselben
    /// Maschine dürfen **kein** positives Urteil ergeben, auch wenn die
    /// Digests übereinstimmen.
    #[test]
    fn eine_maschine_ergibt_keinen_nachweis() {
        let dir = tempdir("eine-maschine");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        let urteil = urteil_ueber(&dir);
        assert_eq!(urteil, Urteil::EineMaschine);
        assert!(!urteil.ist_nachweis(), "darf nicht als Nachweis gelten");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Verschiedene Digests bei gleichem Modellstand sind der eigentliche
    /// Befund und dürfen nicht als Modellstandsfrage abgetan werden.
    #[test]
    fn verschiedene_digests_sind_ein_befund() {
        let dir = tempdir("abweichung");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-y");
        assert_eq!(urteil_ueber(&dir), Urteil::Abweichung);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein abweichender Modellstand muss **vor** dem Digest-Vergleich
    /// erkannt werden: sonst meldet der Client einen Hardware-Befund,
    /// wo zwei verschiedene Modelle verglichen wurden.
    #[test]
    fn abweichender_modellstand_schlaegt_den_digestvergleich() {
        let dir = tempdir("modellstand");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.16.0", "digest-y");
        assert_eq!(urteil_ueber(&dir), Urteil::Modellstand);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ein_einzelnes_protokoll_ist_kein_vergleich() {
        let dir = tempdir("einzeln");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        assert_eq!(urteil_ueber(&dir), Urteil::ZuWenig);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Verschiedene Einstellungen sind verschiedene Gruppen: sie dürfen
    /// nicht gegeneinander verglichen werden.
    #[test]
    fn verschiedene_einstellungen_bleiben_getrennt() {
        let dir = tempdir("gruppen");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "99998888", "x86-64", "fp-b", "0.17.0", "digest-y");
        let gruppen = gruppieren(einlesen(&dir).expect("lesbar"));
        assert_eq!(gruppen.len(), 2);
        assert!(gruppen.iter().all(|g| g.urteil == Urteil::ZuWenig));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Bericht darf nur bei einem echten Nachweis Erfolg melden.
    #[test]
    fn bericht_meldet_nur_beim_nachweis_erfolg() {
        let dir = tempdir("bericht");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        assert!(!run(&dir, None), "eine Maschine darf keinen Erfolg melden");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein leeres Verzeichnis ist kein Nachweis, aber auch kein Absturz.
    #[test]
    fn leeres_verzeichnis_meldet_sauber() {
        let dir = tempdir("leer");
        fs::create_dir_all(&dir).unwrap();
        assert!(!run(&dir, None));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fehlendes_verzeichnis_meldet_sauber() {
        assert!(!run(Path::new("/nicht/vorhanden/myl"), None));
    }

    /// Der Leser muss genau das zurückgeben, was der Schreiber maskiert
    /// hat: sonst weichen Digests scheinbar ab, weil ein Wert falsch
    /// gelesen wurde.
    #[test]
    fn maskierte_zeichen_kommen_unveraendert_zurueck() {
        let original = "Zeile\nmit \"Anführung\" und \\Backslash\tTab";
        let zeile = format!(
            "{{\"t_ms\":7,\"kind\":\"note\",\"text\":\"{}\"}}",
            crate::logging::json_escape(original)
        );
        let f = felder(&zeile).expect("lesbar");
        let text = f.iter().find(|(k, _)| k == "text").expect("text").1.clone();
        assert_eq!(text, original);
    }

    #[test]
    fn zahlen_und_wahrheitswerte_werden_gelesen() {
        let f = felder(r#"{"t_ms":42,"kind":"run_finished","ok":"true"}"#).expect("lesbar");
        assert_eq!(f[0], ("t_ms".to_string(), "42".to_string()));
        assert_eq!(f[2], ("ok".to_string(), "true".to_string()));
    }

    /// Was nicht dem eigenen Format entspricht, wird ausgelassen statt
    /// geraten.
    #[test]
    fn unverstaendliche_zeilen_werden_ausgelassen() {
        assert!(felder("kein json").is_none());
        assert!(felder("{unquoted: 1}").is_none());
        assert!(felder(r#"{"offen":"#).is_none());
        assert!(felder("{}").expect("leeres Objekt").is_empty());
    }

    /// Eine Datei ohne `run_started` ist kein Protokoll.
    #[test]
    fn fremde_dateien_werden_nicht_als_protokoll_gelesen() {
        let dir = tempdir("fremd");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("fremd.jsonl"), "{\"kind\":\"etwas\"}\n").unwrap();
        assert!(einlesen(&dir).expect("lesbar").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    /// **Beide Arten dürfen in denselben Ordner**, und jede Auswertung
    /// sagt, was sie liegen lässt.
    ///
    /// Das ist der Punkt, an dem ein Koordinator sonst rät: Drei Dateien
    /// weniger als erwartet, und niemand sagt, ob sie fehlen oder nicht
    /// dazugehören.
    #[test]
    fn netzprotokolle_werden_erkannt_und_gezaehlt_statt_still_uebergangen() {
        let dir = tempdir("gemischt");
        fs::create_dir_all(&dir).unwrap();
        // Ein Betriebsprotokoll eines Knotens.
        fs::write(
            dir.join("alpha-1.jsonl"),
            "{\"folge\":1,\"zeit_ms\":1,\"knoten\":\"alpha\",\"peer\":\"p1\",\"art\":\"start\"}\n",
        )
        .unwrap();
        // Etwas, das keines von beidem ist.
        fs::write(dir.join("muell.jsonl"), "{\"kind\":\"etwas\"}\n").unwrap();

        let e = einlesen_mit_bericht(&dir).expect("lesbar");
        assert!(e.protokolle.is_empty(), "kein Determinismuslauf lag da");
        assert_eq!(e.betriebsprotokolle, 1, "das Netzprotokoll wurde nicht erkannt");
        assert_eq!(e.sonstige, 1, "die Müll-Datei wurde nicht getrennt gezählt");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Bericht muss das Urteil und die vollständigen Digests tragen:
    /// die Bildschirmausgabe kürzt beides, und der Bericht wird
    /// weitergereicht.
    #[test]
    fn bericht_traegt_urteil_und_volle_digests() {
        let dir = tempdir("bericht-inhalt");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-x");
        let gruppen = gruppieren(einlesen(&dir).expect("lesbar"));

        let text = bericht_text(&dir, "2026-08-21", "143022", &gruppen);
        assert!(text.contains("# Vergleichsbericht 2026-08-21 143022"), "{text}");
        assert!(text.contains("NACHWEIS"), "{text}");
        assert!(text.contains("anna") && text.contains("björn"), "{text}");
        assert!(text.contains("digest-x"), "voller Digest fehlt");
        assert!(text.contains("0.17.0"), "θ_v fehlt");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein Bericht ohne Protokolle darf nicht so aussehen, als sei nichts
    /// zu beanstanden gewesen.
    #[test]
    fn leerer_bericht_sagt_es_deutlich() {
        let text = bericht_text(Path::new("/leer"), "2026-08-21", "143022", &[]);
        assert!(text.contains("Keine Protokolle gefunden"), "{text}");
        assert!(!text.contains("NACHWEIS"), "leerer Bericht darf nichts belegen");
    }

    /// Der Bericht landet in seinem eigenen Ordner und wird beim nächsten
    /// Vergleich **nicht** als Eingabe mitgelesen.
    #[test]
    fn bericht_wird_nicht_zur_eingabe() {
        let dir = tempdir("bericht-ablage");
        let berichte = dir.join("Berichte");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-x");

        assert!(run(&dir, Some(&berichte)));
        let geschrieben: Vec<_> = fs::read_dir(&berichte)
            .expect("Berichtsordner")
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(geschrieben.len(), 1, "genau ein Bericht erwartet");

        // Zweiter Lauf über denselben Ordner: Die Zahl der eingelesenen
        // Protokolle darf sich nicht verändert haben.
        assert_eq!(einlesen(&dir).expect("lesbar").len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ein unbeschreibbarer Berichtsordner darf das Urteil nicht kippen:
    /// es steht bereits auf dem Bildschirm und ist damit gefällt.
    #[test]
    fn fehlender_berichtsordner_kippt_das_urteil_nicht() {
        let dir = tempdir("bericht-fehler");
        protokoll_schreiben(&dir, "anna", "abcd1234", "aarch64", "fp-a", "0.17.0", "digest-x");
        protokoll_schreiben(&dir, "björn", "abcd1234", "x86-64", "fp-b", "0.17.0", "digest-x");
        assert!(run(&dir, Some(Path::new("/proc/kein-schreibzugriff/myl"))));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Ohne Namen muss die Bezeichnung auf die Hardware zurückfallen:
    /// ein Bericht mit lauter „ohne-name" wäre nicht auswertbar.
    #[test]
    fn bezeichnung_faellt_auf_hardware_zurueck() {
        let p = Protokoll {
            teilnehmer: crate::logging::OHNE_NAME.to_string(),
            hardware: "aarch64-macos-reference".to_string(),
            schema: crate::hardware::FINGERABDRUCK_SCHEMA.into(),
            rechenpfad: "pfad-referenz".into(),
            ..Default::default()
        };
        assert_eq!(p.bezeichnung(), "aarch64-macos-reference");
    }

    /// **Fund 35 als Test (2026-08-22).** Ein Partner bricht nach dem
    /// ersten von sechs Prompts ab und schickt sein Protokoll. Vorher
    /// urteilte der Vergleich darüber `NACHWEIS`: Verglichen wurde je
    /// Wert nur unter den Läufen, die ihn hatten, und in `prompt_0`
    /// stimmten beide überein. Die übrigen fünf Werte kamen von einer
    /// einzigen Maschine und wurden nie verglichen.
    #[test]
    fn ein_abgebrochener_lauf_traegt_keinen_nachweis() {
        let dir = tempdir("abbruch");
        let voll: Vec<(&str, &str)> = vec![
            ("prompt_0", "d0"),
            ("prompt_1", "d1"),
            ("prompt_2", "d2"),
            ("prompt_3", "d3"),
            ("prompt_4", "d4"),
            ("prompt_5", "d5"),
        ];
        lauf_mit_werten(&dir, "anna", "aarch64", "fp-a", &voll, true);
        lauf_mit_werten(&dir, "björn", "x86-64", "fp-b", &voll[..1], false);

        assert_eq!(urteil_ueber(&dir), Urteil::Unvollstaendig);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Abbruch muss auch dann auffallen, wenn er alle Werte erreicht
    /// hat: Ein Lauf ohne Abschlusseintrag kann mitten im Schreiben
    /// geendet haben, und was danach gekommen wäre, weiß niemand.
    #[test]
    fn fehlender_abschluss_genuegt_fuer_unvollstaendig() {
        let dir = tempdir("kein-abschluss");
        let werte: Vec<(&str, &str)> = vec![("prompt_0", "d0"), ("prompt_1", "d1")];
        lauf_mit_werten(&dir, "anna", "aarch64", "fp-a", &werte, true);
        lauf_mit_werten(&dir, "björn", "x86-64", "fp-b", &werte, false);

        assert_eq!(urteil_ueber(&dir), Urteil::Unvollstaendig);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Gegenprobe: Zwei vollständige Läufe über dieselben sechs Werte
    /// ergeben weiterhin den Nachweis. Ohne diesen Test wäre die neue
    /// Prüfung nicht von einer Sperre gegen alles zu unterscheiden.
    #[test]
    fn zwei_vollstaendige_laeufe_ergeben_weiterhin_den_nachweis() {
        let dir = tempdir("vollstaendig");
        let werte: Vec<(&str, &str)> = vec![
            ("prompt_0", "d0"),
            ("prompt_1", "d1"),
            ("prompt_2", "d2"),
        ];
        lauf_mit_werten(&dir, "anna", "aarch64", "fp-a", &werte, true);
        lauf_mit_werten(&dir, "björn", "x86-64", "fp-b", &werte, true);

        assert_eq!(urteil_ueber(&dir), Urteil::Nachweis);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Eine echte Abweichung wiegt schwerer als die Unvollständigkeit.
    /// Sonst verschwände der wichtigste Befund des Werkzeugs hinter einem
    /// Verfahrenshinweis, sobald ein Lauf nebenbei abgebrochen ist.
    #[test]
    fn abweichung_geht_der_unvollstaendigkeit_vor() {
        let dir = tempdir("abweichung-vor-abbruch");
        lauf_mit_werten(
            &dir,
            "anna",
            "aarch64",
            "fp-a",
            &[("prompt_0", "d0"), ("prompt_1", "d1")],
            true,
        );
        lauf_mit_werten(&dir, "björn", "x86-64", "fp-b", &[("prompt_0", "ANDERS")], false);

        assert_eq!(urteil_ueber(&dir), Urteil::Abweichung);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Verschiedene Mengen an Vergleichswerten fallen auch dann auf, wenn
    /// beide Läufe sauber abgeschlossen haben: Dann hat ein Testplan
    /// unterwegs etwas übersprungen, und die Überschneidung allein trägt
    /// keine Aussage.
    #[test]
    fn verschiedene_wertemengen_sind_unvollstaendig() {
        let dir = tempdir("wertemengen");
        lauf_mit_werten(
            &dir,
            "anna",
            "aarch64",
            "fp-a",
            &[("prompt_0", "d0"), ("prompt_1", "d1")],
            true,
        );
        lauf_mit_werten(&dir, "björn", "x86-64", "fp-b", &[("prompt_0", "d0")], true);

        assert_eq!(urteil_ueber(&dir), Urteil::Unvollstaendig);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Der Mangel muss im Bericht stehen, nicht nur im Urteil: Der
    /// Bericht ist das, was aufbewahrt und weitergereicht wird.
    #[test]
    fn der_bericht_nennt_den_mangel() {
        let dir = tempdir("bericht-mangel");
        let werte: Vec<(&str, &str)> = vec![("prompt_0", "d0")];
        lauf_mit_werten(&dir, "anna", "aarch64", "fp-a", &werte, true);
        lauf_mit_werten(&dir, "björn", "x86-64", "fp-b", &werte, false);

        let gruppen = gruppieren(einlesen(&dir).expect("lesbar"));
        let text = bericht_text(&dir, "2026-08-22", "12:00", &gruppen);
        assert!(
            text.contains("ohne Abschluss"),
            "Bericht verschweigt den Mangel:\n{text}"
        );
        assert!(
            text.contains("vollständig"),
            "Bericht kennzeichnet den heilen Lauf nicht:\n{text}"
        );
        let _ = fs::remove_dir_all(&dir);
    }


    /// **Fund 36 als Test (2026-08-22).** Zwei Protokolle, deren
    /// Vergleichswerte verschiedene Dinge abdecken, dürfen nicht
    /// gegeneinander geurteilt werden: Die Werte müssten abweichen, und
    /// das sähe wie ein Hardware-Befund aus.
    #[test]
    fn verschiedener_digest_umfang_ist_unvergleichbar() {
        let a = Protokoll {
            theta_v: "0.17.0".into(),
            artefakt_digest: "c42b".into(),
            digest_umfang: "logits+token".into(),
            fingerprint: "fp-a".into(),
            abgeschlossen: true,
            erfolgreich: true,
            ergebnisse: vec![("determinismus".into(), "d0".into())],
            schema: crate::hardware::FINGERABDRUCK_SCHEMA.into(),
            rechenpfad: "pfad-referenz".into(),
            ..Default::default()
        };
        let b = Protokoll {
            // Wie ein Protokoll aus der Fassung vor Fund 36: Feld fehlt.
            digest_umfang: String::new(),
            fingerprint: "fp-b".into(),
            ..a.clone()
        };
        let protokolle = vec![a, b];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::Modellstand);
    }

    /// Gegenprobe: Gleicher Umfang, sonst alles wie oben, ergibt den
    /// Nachweis. Ohne sie wäre nicht unterscheidbar, ob die Prüfung den
    /// Umfang beurteilt oder einfach alles ablehnt.
    #[test]
    fn gleicher_digest_umfang_stoert_den_nachweis_nicht() {
        let a = Protokoll {
            theta_v: "0.17.0".into(),
            artefakt_digest: "c42b".into(),
            digest_umfang: "logits+token".into(),
            fingerprint: "fp-a".into(),
            abgeschlossen: true,
            erfolgreich: true,
            ergebnisse: vec![("determinismus".into(), "d0".into())],
            schema: crate::hardware::FINGERABDRUCK_SCHEMA.into(),
            rechenpfad: "pfad-referenz".into(),
            ..Default::default()
        };
        let b = Protokoll {
            fingerprint: "fp-b".into(),
            ..a.clone()
        };
        let protokolle = vec![a, b];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::Nachweis);
    }

    /// Zwei Konformitätsläufe mit verschiedenen Umfängen haben nicht
    /// dasselbe gemessen: der eine nur die Operations-Vektoren, der andere
    /// auch Layer und E2E. Sie dürfen nicht gegeneinander geurteilt werden.
    #[test]
    fn verschiedener_konformitaets_umfang_ist_unvergleichbar() {
        let a = Protokoll {
            theta_v: "0.17.0".into(),
            artefakt_digest: "c42b".into(),
            digest_umfang: "logits+token".into(),
            konformitaet_umfang: "op+layer+e2e".into(),
            fingerprint: "fp-a".into(),
            abgeschlossen: true,
            erfolgreich: true,
            ergebnisse: vec![("konformitaet".into(), "k0".into())],
            schema: crate::hardware::FINGERABDRUCK_SCHEMA.into(),
            rechenpfad: "pfad-referenz".into(),
            ..Default::default()
        };
        let b = Protokoll {
            konformitaet_umfang: "op".into(),
            fingerprint: "fp-b".into(),
            ..a.clone()
        };
        let protokolle = vec![a, b];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::Modellstand);
    }

    /// Gegenprobe: Gleicher Konformitäts-Umfang, sonst alles wie oben,
    /// ergibt den Nachweis.
    #[test]
    fn gleicher_konformitaets_umfang_stoert_den_nachweis_nicht() {
        let a = Protokoll {
            theta_v: "0.17.0".into(),
            artefakt_digest: "c42b".into(),
            digest_umfang: "logits+token".into(),
            konformitaet_umfang: "op+layer+e2e".into(),
            fingerprint: "fp-a".into(),
            abgeschlossen: true,
            erfolgreich: true,
            ergebnisse: vec![("konformitaet".into(), "k0".into())],
            schema: crate::hardware::FINGERABDRUCK_SCHEMA.into(),
            rechenpfad: "pfad-referenz".into(),
            ..Default::default()
        };
        let b = Protokoll {
            fingerprint: "fp-b".into(),
            ..a.clone()
        };
        let protokolle = vec![a, b];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::Nachweis);
    }

    /// Der Leser muss den `konformitaet_umfang` aus der Hardware-Zeile
    /// ziehen: ohne ihn fiele der Vergleich auf „beide leer" zurück und
    /// sähe Läufe als vergleichbar, die es nicht sind.
    #[test]
    fn konformitaets_umfang_wird_eingelesen() {
        let dir = tempdir("konf-umfang-lesen");
        let mut log = RunLog::mit_ziel(
            LogZiel::neu(&dir, "konformitaet", "anna", "abcd1234", "aarch64"),
            false,
        );
        log.event(Event::Hardware {
            key: "konformitaet_umfang".into(),
            value: "op".into(),
        });
        log.result("konformitaet", "k0", "6/6");
        let datei = dir.join(format!("{}.jsonl", log.dateiname()));
        log.finish(true);

        let p = protokoll_lesen(&datei).expect("Protokoll");
        assert_eq!(p.konformitaet_umfang, "op");
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Fund 105: Maschine und Rechenpfad sind zwei Fragen ──────────

    /// Ein Grundgerüst für die vier Proben darunter: vollständiger Lauf,
    /// ein Vergleichswert, heutiges Schema.
    fn lauf(maschine: &str, pfad: &str) -> Protokoll {
        Protokoll {
            theta_v: "0.17.0".into(),
            artefakt_digest: "c42b".into(),
            digest_umfang: "logits+token".into(),
            fingerprint: maschine.into(),
            rechenpfad: pfad.into(),
            schema: crate::hardware::FINGERABDRUCK_SCHEMA.into(),
            abgeschlossen: true,
            erfolgreich: true,
            ergebnisse: vec![("determinismus".into(), "d0".into())],
            ..Default::default()
        }
    }

    /// ⚑ **Fund 105 in einem Satz.** Zwei Bauten auf einer Maschine sind
    /// kein Cross-Hardware-Nachweis.
    ///
    /// Bis zum 2026-08-30 kam hier `Urteil::Nachweis` heraus, weil der
    /// Bau in den Fingerabdruck einging. Nachgestellt wurde es mit dem
    /// echten Client: ein MacBook, `cargo build` mit und ohne
    /// `--features cpu-simd`, gleicher Konformitätswert, Urteil
    /// „NACHWEIS".
    #[test]
    fn zwei_bauten_einer_maschine_sind_kein_cross_hardware_nachweis() {
        let protokolle = vec![lauf("gleiche-cpu", "pfad-referenz"), lauf("gleiche-cpu", "pfad-simd")];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::Rechenpfad);
    }

    /// Und der Rechenpfad-Nachweis darf den Rückgabewert nicht auf Erfolg
    /// setzen: `myl-test vergleich` meldet mit seinem Exit-Code genau eine
    /// Aussage, nämlich den Cross-Hardware-Nachweis.
    #[test]
    fn der_rechenpfad_nachweis_ist_kein_nachweis() {
        assert!(!Urteil::Rechenpfad.ist_nachweis());
        assert!(Urteil::Nachweis.ist_nachweis());
    }

    /// Gegenprobe: Zwei Maschinen bleiben der Nachweis, auch wenn sie
    /// zusätzlich verschiedene Rechenpfade fahren. Ohne sie wäre nicht
    /// unterscheidbar, ob die neue Prüfung die Maschinen beurteilt oder
    /// jeden Pfadunterschied abwertet.
    #[test]
    fn zwei_maschinen_bleiben_der_nachweis_auch_mit_zwei_pfaden() {
        let protokolle = vec![lauf("cpu-a", "pfad-referenz"), lauf("cpu-b", "pfad-simd")];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::Nachweis);
    }

    /// Eine Maschine, ein Pfad, zwei Läufe: unverändert kein Nachweis.
    #[test]
    fn eine_maschine_ein_pfad_bleibt_kein_nachweis() {
        let protokolle = vec![lauf("gleiche-cpu", "pfad-referenz"), lauf("gleiche-cpu", "pfad-referenz")];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::EineMaschine);
    }

    /// Ein Protokoll ohne Schema-Marke stammt aus einer Fassung, deren
    /// Fingerabdruck eine andere Feldmenge abdeckte. Es ist unvergleichbar,
    /// und **das Schweigen darüber wäre Fund 105 über zwei Fassungen**.
    #[test]
    fn ein_protokoll_ohne_schema_ist_unvergleichbar() {
        let mut alt = lauf("cpu-b", "pfad-referenz");
        alt.schema = String::new();
        let protokolle = vec![lauf("cpu-a", "pfad-referenz"), alt];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::Fingerabdruckschema);
    }

    /// Die Marke verspricht beide Fingerabdrücke. Fehlt einer trotzdem,
    /// ist das Protokoll beschädigt, und ein Urteil darüber wäre
    /// erfunden.
    #[test]
    fn marke_ohne_fingerabdruck_ist_unvergleichbar() {
        let mut beschaedigt = lauf("cpu-b", "pfad-referenz");
        beschaedigt.fingerprint = String::new();
        let protokolle = vec![lauf("cpu-a", "pfad-referenz"), beschaedigt];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::Fingerabdruckschema);
    }

    /// Und ebenso, wenn beide eine Marke tragen, aber verschiedene.
    #[test]
    fn verschiedene_schemata_sind_unvergleichbar() {
        let mut kuenftig = lauf("cpu-b", "pfad-referenz");
        kuenftig.schema = "maschine/2".into();
        let protokolle = vec![lauf("cpu-a", "pfad-referenz"), kuenftig];
        let werte = werte_sammeln(&protokolle);
        assert_eq!(urteilen(&protokolle, &werte), Urteil::Fingerabdruckschema);
    }

    // ── Punkt 2: die Abweichung eingrenzen ──────────────────────────

    /// Wie [`lauf`], aber mit frei gesetzten Vergleichswerten.
    fn lauf_mit(maschine: &str, werte: &[(&str, &str)], umfang: &str) -> Protokoll {
        Protokoll {
            konformitaet_umfang: umfang.into(),
            ergebnisse: werte
                .iter()
                .map(|(n, d)| (n.to_string(), d.to_string()))
                .collect(),
            ..lauf(maschine, "pfad-referenz")
        }
    }

    fn hinweis(protokolle: Vec<Protokoll>) -> String {
        let gruppen = gruppieren(protokolle);
        assert_eq!(gruppen.len(), 1, "die Probe braucht genau eine Gruppe");
        assert_eq!(gruppen[0].urteil, Urteil::Abweichung);
        abweichungs_hinweis(&gruppen[0])
    }

    /// Weichen schon die Konformitätsvektoren ab, sitzt der Unterschied
    /// in den Kerneln. Das ist die Einengung, die einen bezahlten
    /// Nachmittag rettet.
    #[test]
    fn abweichende_konformitaet_zeigt_unter_das_modell() {
        let t = hinweis(vec![
            lauf_mit("cpu-a", &[("konformitaet", "k0"), ("determinismus", "d0")], "op"),
            lauf_mit("cpu-b", &[("konformitaet", "k1"), ("determinismus", "d1")], "op"),
        ]);
        assert!(t.contains("UNTERHALB"), "{t}");
        assert!(t.contains("myl-test konformitaet"), "{t}");
        // Beide liefen ohne Artefakt: der Hinweis sagt, was fehlt.
        assert!(t.contains("Layer- und E2E-Vektoren"), "{t}");
    }

    /// Stimmen die Vektoren überein und weicht nur der Modelllauf ab,
    /// rechnen die Kernel gleich. Dann ist es kein Kernel-Befund, und der
    /// Hinweis muss in die andere Richtung zeigen.
    #[test]
    fn gleiche_konformitaet_zeigt_ueber_das_modell() {
        let t = hinweis(vec![
            lauf_mit("cpu-a", &[("konformitaet", "k0"), ("determinismus", "d0")], "op"),
            lauf_mit("cpu-b", &[("konformitaet", "k0"), ("determinismus", "d1")], "op"),
        ]);
        assert!(t.contains("DARÜBER"), "{t}");
        assert!(t.contains("myl-test artefakte"), "{t}");
        assert!(!t.contains("UNTERHALB"), "{t}");
    }

    /// Ohne Konformitätswert lässt sich nichts eingrenzen, und **genau
    /// das gehört gesagt**. Ein Hinweis, der so täte, als wüsste er es,
    /// wäre schlimmer als keiner.
    #[test]
    fn ohne_konformitaetswert_sagt_der_hinweis_das() {
        let t = hinweis(vec![
            lauf_mit("cpu-a", &[("determinismus", "d0")], ""),
            lauf_mit("cpu-b", &[("determinismus", "d1")], ""),
        ]);
        assert!(t.contains("keinen Konformitätswert"), "{t}");
        assert!(t.contains("fünfte Stufe"), "{t}");
    }

    /// Der Hinweis nennt die abweichenden Werte beim Namen. Ohne das
    /// müsste man sie aus der Tabelle darüber zusammensuchen.
    #[test]
    fn der_hinweis_nennt_die_abweichenden_werte() {
        let t = hinweis(vec![
            lauf_mit("cpu-a", &[("konformitaet", "k0"), ("determinismus", "d0")], "op"),
            lauf_mit("cpu-b", &[("konformitaet", "k0"), ("determinismus", "d1")], "op"),
        ]);
        assert!(t.contains("Auseinander gehen: determinismus"), "{t}");
    }
}
