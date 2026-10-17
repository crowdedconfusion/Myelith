//! **`/mining`**: was dieser Rechner dem Netz hergibt, mit den Pfeiltasten.
//!
//! # ⚑ Dieselbe Bedienung wie `/settings`
//!
//! Hoch und runter waehlt, links und rechts aendert, Enter tippt einen
//! Ordner oder loest den Knopf aus (Auftrag des Projektinhabers,
//! 2026-10-17). **Entschieden wird nichts hier**: Vorschlag, Pruefung,
//! Ablage und Start stehen in `myl_client::mining`, und das Fenster ruft
//! dieselben.

use std::io::{IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};

use myl_client::hardware::Hardware;
use myl_client::mining::{self, Freigabe, Platte, Werkwahl};

use crate::design::Toene;

/// Die Zeile ganz unten.
pub const KUERZEL: &str = "  ↑↓ wählen · ←→ ändern · ⏎ eingeben oder auslösen · Esc schließen";

/// Eine Zeile der Seite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reihe {
    Platte,
    Ordner,
    Volumen,
    Gewichte,
    Kerne,
    Rechenwerk(usize),
    Knopf,
}

impl Reihe {
    /// ⚑ **Der Name der Karte steht im Wert und nicht im Titel**: Die
    /// Titelspalte ist dreissig Zeichen breit, und „NVIDIA GeForce RTX 4090
    /// (CUDA)" passte dort nicht hinein.
    fn titel(self) -> String {
        match self {
            Reihe::Platte => "Platte".into(),
            Reihe::Ordner => "Ordner".into(),
            Reihe::Volumen => "Volumen".into(),
            Reihe::Gewichte => "Eigene Gewichte auch dort".into(),
            Reihe::Kerne => "CPU-Kerne".into(),
            Reihe::Rechenwerk(i) => format!("Rechenwerk {}", i + 1),
            Reihe::Knopf => String::new(),
        }
    }

    /// Der Satz unter der Liste; bei einem gesperrten Rechenwerk sein Grund.
    fn hinweis(self, werke: &[Werkwahl]) -> String {
        if let Reihe::Rechenwerk(i) = self {
            if let Some(g) = werke.get(i).and_then(|w| w.sperrgrund.as_deref()) {
                return g.to_string();
            }
        }
        self.satz().to_string()
    }

    fn satz(self) -> &'static str {
        match self {
            Reihe::Platte => "Wo der Ort liegt; angeschlossene Platten stehen unter ihrem Namen.",
            Reihe::Ordner => "Der Ordner auf der Platte (⏎ zum Ändern). Darin liegen netz/ und, wenn erlaubt, gewichte/.",
            Reihe::Volumen => "Wie viel dort höchstens liegt. Was zugesagt wird, muss frei sein.",
            Reihe::Gewichte => "Ob der Client dort unter gewichte/ auch eigene Modelle sucht.",
            Reihe::Kerne | Reihe::Rechenwerk(_) => {
                "Steht in der Zusage. Gerechnet wird damit, sobald ein Shard am Knoten hängt."
            }
            Reihe::Knopf => {
                "Startet einen Knoten im Probelauf: eigene Probekette, die MYL sind Spielgeld. Die Zusage unterschreibt er mit seinem Kontoschlüssel."
            }
        }
    }

    fn mit_pfeilen(self, werke: &[Werkwahl]) -> bool {
        match self {
            Reihe::Ordner | Reihe::Knopf => false,
            Reihe::Rechenwerk(i) => werke.get(i).is_some_and(|w| w.rechnet),
            _ => true,
        }
    }
}

/// ⚑ **Jedes gefundene Rechenwerk bekommt eine eigene Reihe**, auch eines,
/// das hier nicht rechnet: Es steht gesperrt da und sagt warum.
fn reihen(werke: &[Werkwahl]) -> Vec<Reihe> {
    let mut r = vec![Reihe::Platte, Reihe::Ordner, Reihe::Volumen, Reihe::Gewichte, Reihe::Kerne];
    r.extend((0..werke.len()).map(Reihe::Rechenwerk));
    r.push(Reihe::Knopf);
    r
}

/// Was die Seite weiss.
struct Seite {
    f: Freigabe,
    platten: Vec<Platte>,
    hw: Hardware,
    /// Die Rechenwerke, wie die Seite sie zeigt (`mining::werke`).
    werke: Vec<Werkwahl>,
    laeuft: Option<mining::Zustand>,
    /// Die Zusammenfassung beim Oeffnen und nach dem Start.
    stand: Vec<String>,
    meldung: String,
    geaendert: bool,
}

impl Seite {
    /// Das Hoechste, was die Platte hergibt, in GiB.
    fn hoechstens_gib(&self) -> u64 {
        let frei = Hardware::platte(&self.f.platte).map(|p| p.frei_bytes).unwrap_or(0);
        // ⚑ Dieselbe Zaehlung wie die Pruefung (`mining::pruefen`).
        let schon = mining::belegt(&self.f.pfad());
        (frei.saturating_add(schon) / myl_client::hardware::GIB).max(1)
    }

    fn wert(&self, r: Reihe) -> String {
        match r {
            Reihe::Platte => match self.platten.iter().find(|p| p.wurzel == self.f.platte) {
                Some(p) => format!("{} · {} frei", p.name, mining::groesse_text(p.frei_bytes)),
                None => format!("{} (nicht angeschlossen)", self.f.platte.display()),
            },
            Reihe::Ordner => format!("{}  →  {}", self.f.ordner, self.f.pfad().display()),
            Reihe::Volumen => format!("{} GiB  (höchstens {} GiB)", self.f.volumen_gib, self.hoechstens_gib()),
            Reihe::Gewichte => if self.f.gewichte { "ja" } else { "nein" }.into(),
            Reihe::Kerne => format!("{} von {}", self.f.kerne, self.hw.kerne),
            Reihe::Rechenwerk(i) => {
                let w = &self.werke[i];
                let lage = if !w.rechnet {
                    "gesperrt, rechnet hier nicht"
                } else if self.f.rechenwerke.contains(&w.kennung) {
                    "freigegeben"
                } else {
                    "nicht freigegeben"
                };
                format!("{} · {lage}", w.titel)
            }
            Reihe::Knopf => if self.laeuft.is_some() { "[ Mining beenden ]" } else { "[ Mining starten ]" }.into(),
        }
    }

    /// Links oder rechts.
    fn aendern(&mut self, r: Reihe, rechts: bool) {
        match r {
            Reihe::Platte => {
                if self.platten.is_empty() {
                    return;
                }
                let n = self.platten.len();
                let jetzt = self.platten.iter().position(|p| p.wurzel == self.f.platte);
                let neu = match (jetzt, rechts) {
                    (Some(i), true) => (i + 1) % n,
                    (Some(i), false) => (i + n - 1) % n,
                    (None, _) => 0,
                };
                self.f.platte = self.platten[neu].wurzel.clone();
                self.f.volumen_gib = self.f.volumen_gib.min(self.hoechstens_gib());
            }
            Reihe::Volumen => {
                let s = crate::einstellseite::schritt(self.f.volumen_gib);
                self.f.volumen_gib = if rechts {
                    (self.f.volumen_gib + s).min(self.hoechstens_gib())
                } else {
                    self.f.volumen_gib.saturating_sub(s).max(1)
                };
            }
            Reihe::Gewichte => self.f.gewichte = !self.f.gewichte,
            Reihe::Kerne => {
                let k = self.hw.kerne.max(1) as u32;
                self.f.kerne = if rechts { (self.f.kerne + 1).min(k) } else { self.f.kerne.saturating_sub(1).max(1) };
            }
            Reihe::Rechenwerk(i) => {
                if !self.werke[i].rechnet {
                    return;
                }
                let k = self.werke[i].kennung.clone();
                if let Some(p) = self.f.rechenwerke.iter().position(|x| x == &k) {
                    self.f.rechenwerke.remove(p);
                } else {
                    self.f.rechenwerke.push(k);
                }
            }
            Reihe::Ordner | Reihe::Knopf => return,
        }
        self.geaendert = true;
    }
}

fn jetzt_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn standzeilen(z: &mut Option<mining::Zustand>) -> Vec<String> {
    match z {
        Some(z) => match mining::stand(z) {
            Ok(s) => mining::zusammenfassung(z, &s, jetzt_ms()),
            Err(f) => vec![format!("⛏ Mining läuft, der Knoten antwortet aber nicht: {f}")],
        },
        None => vec!["Mining läuft nicht.".into()],
    }
}

/// **Faehrt die Seite.** Gibt den Stand danach zurueck: `Some`, wenn Mining
/// laeuft.
pub fn fahren(t: Toene) -> Option<mining::Zustand> {
    crate::schimmer::vergessen();
    let e = myl_client::Einstellungen::lesen(&myl_client::Einstellungen::vorgabepfad()).unwrap_or_default();
    let orte = match mining::Speicherorte::lesen(&mining::speicherorte_pfad()) {
        Ok(o) => o,
        Err(f) => {
            println!("  {f}");
            println!("  Die Datei liegt unter {}.", mining::speicherorte_pfad().display());
            return mining::zustand();
        }
    };
    let platten = mining::platten();
    // ⚑ **Beim Oeffnen neu gesucht**: Eine Karte, die seit dem Start der
    // Konsole angesteckt wurde, steht sonst erst nach einem Neustart da
    // (Wunsch des Projektinhabers, 2026-10-17). Kostet unter macOS einen
    // Aufruf von `system_profiler`, unter Linux mit NVIDIA einen von
    // `nvidia-smi`.
    myl_client::hardware::rechenwerke_neu_suchen();
    let hw = myl_client::hardware::erheben(&myl_local_agent::ort::datenort());
    let werke = mining::werke(&hw, e.oberflaeche.sprache);
    let f = Freigabe::vorschlag(&e, &orte, &platten, hw.kerne.max(1) as u32, &werke);
    let mut laeuft = mining::zustand();
    let stand = standzeilen(&mut laeuft);
    let mut s = Seite { f, platten, hw, werke, laeuft, stand, meldung: String::new(), geaendert: false };
    let weitere: Vec<String> =
        orte.fuer(mining::Zweck::Netz).filter(|o| o.name != mining::ORTNAME).map(|o| o.name.clone()).collect();

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        for z in &s.stand {
            println!("  {z}");
        }
        return s.laeuft;
    }
    let Ok(_roh) = crate::auswahl::Rohmodus::an() else {
        return s.laeuft;
    };
    let liste = reihen(&s.werke);
    let mut hier = liste.len() - 1;
    zeichnen(&s, &liste, hier, t, &weitere);
    loop {
        let Ok(Event::Key(k)) = event::read() else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        let strg = k.modifiers.contains(KeyModifiers::CONTROL);
        s.meldung.clear();
        let r = liste[hier];
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => break,
            KeyCode::Char('c') if strg => break,
            KeyCode::Up => hier = hier.saturating_sub(1),
            KeyCode::Down => hier = (hier + 1).min(liste.len() - 1),
            KeyCode::Left | KeyCode::Right if s.laeuft.is_some() && r != Reihe::Knopf => {
                s.meldung = "Mining läuft; erst beenden, dann ändern.".into();
            }
            KeyCode::Left | KeyCode::Right => {
                if r.mit_pfeilen(&s.werke) {
                    s.aendern(r, k.code == KeyCode::Right);
                } else if r == Reihe::Ordner {
                    s.meldung = "Der Ordner braucht eine Eingabe: ⏎.".into();
                } else if matches!(r, Reihe::Rechenwerk(_)) {
                    s.meldung = "Dieses Rechenwerk rechnet hier nicht; warum, steht darüber.".into();
                }
            }
            KeyCode::Enter if r == Reihe::Ordner && s.laeuft.is_none() => {
                if let Some(neu) = crate::einstellseite::tippen("Ordner", &s.f.ordner, t) {
                    s.f.ordner = neu;
                    s.geaendert = true;
                }
            }
            KeyCode::Enter if r == Reihe::Knopf => {
                if s.laeuft.is_some() {
                    s.meldung = "beendet den Knoten …".into();
                    zeichnen(&s, &liste, hier, t, &weitere);
                    match mining::beenden() {
                        Ok(_) => {
                            s.laeuft = None;
                            s.meldung = "Mining beendet.".into();
                        }
                        Err(f) => s.meldung = f,
                    }
                } else {
                    s.meldung = "startet den Knoten …".into();
                    zeichnen(&s, &liste, hier, t, &weitere);
                    match mining::starten(&s.f, "Konsole") {
                        Ok(z) => {
                            s.laeuft = Some(z);
                            s.geaendert = false;
                            s.meldung = "Mining läuft. Die Seite darf zu; der Knoten läuft weiter, bis /mining stop oder das Ende der Konsole.".into();
                        }
                        // ⛔️ Ohne Platz keine Freigabe; der Grund steht da.
                        Err(f) => s.meldung = f,
                    }
                }
                s.stand = standzeilen(&mut s.laeuft);
            }
            KeyCode::Enter => s.meldung = "Hier ändern ← und →.".into(),
            _ => continue,
        }
        zeichnen(&s, &liste, hier, t, &weitere);
    }
    drop(_roh);
    // ⚑ **Geaendert und nicht gestartet heisst trotzdem abgelegt**: Wer nur
    //   einen Ort fuer eigene Gewichte eintraegt, startet kein Mining.
    if s.geaendert && s.laeuft.is_none() {
        match mining::pruefen(&s.f, &s.hw).and_then(|_| mining::speichern(&s.f)) {
            Ok(()) => println!("  Abgelegt in {}.", mining::speicherorte_pfad().display()),
            Err(f) => println!("  Nicht abgelegt: {f}"),
        }
    }
    s.laeuft
}

fn zeichnen(s: &Seite, liste: &[Reihe], hier: usize, t: Toene, weitere: &[String]) {
    let breite = (crate::banner::fensterbreite() as usize).clamp(40, 110);
    let mut aus = std::io::stdout();
    let _ = crossterm::queue!(aus, crossterm::cursor::Hide, Print("\x1b[H\x1b[J"));
    let zeile = |aus: &mut std::io::Stdout, text: &str, farbe| {
        let _ = crossterm::queue!(aus, SetForegroundColor(farbe), Print(kuerzen(text, breite)), ResetColor, Print("\r\n"));
    };
    zeile(&mut aus, "  Mining · was dieser Rechner dem Netz hergibt (Probelauf, ohne Testnetz)", t.akzent);
    zeile(&mut aus, "", t.beiwerk);
    for z in &s.stand {
        zeile(&mut aus, &format!("  {z}"), if s.laeuft.is_some() { t.akzent } else { t.beiwerk });
    }
    zeile(&mut aus, "", t.beiwerk);
    for (i, r) in liste.iter().enumerate() {
        let hierhin = i == hier;
        let marke = if hierhin { "▸" } else { " " };
        if *r == Reihe::Knopf {
            zeile(&mut aus, "", t.beiwerk);
        }
        let beweglich = r.mit_pfeilen(&s.werke) && s.laeuft.is_none();
        let (links, rechts) = if beweglich { ("‹ ", " ›") } else { ("  ", "") };
        let titel = format!("  {marke} {:<30}", kuerzen(&r.titel(), 30));
        let wert = format!("{links}{}{rechts}", s.wert(*r));
        let _ = crossterm::queue!(aus, Print("\x1b[2K"));
        if hierhin {
            let _ = crossterm::queue!(aus, SetForegroundColor(t.akzent), SetAttribute(Attribute::Bold), Print(&titel), SetAttribute(Attribute::Reset));
        } else {
            let _ = crossterm::queue!(aus, ResetColor, Print(&titel));
        }
        let platz = breite.saturating_sub(titel.chars().count() + 2);
        let _ = crossterm::queue!(
            aus,
            SetForegroundColor(if hierhin { t.akzent } else { t.beiwerk }),
            Print(kuerzen(&wert, platz)),
            ResetColor,
            Print("\r\n")
        );
    }
    zeile(&mut aus, "", t.beiwerk);
    // ⚑ **Umgebrochen und nicht gekuerzt**: Der Grund, warum eine Karte
    // gesperrt ist, ist mehrere Saetze lang, und abgeschnitten sagte er
    // nichts mehr.
    for z in umbrechen(&liste[hier].hinweis(&s.werke), breite.saturating_sub(4)).iter().take(HINWEISZEILEN) {
        zeile(&mut aus, &format!("  {z}"), t.beiwerk);
    }
    if s.meldung.is_empty() {
        let mut info = format!("  Speicherorte: {}", mining::speicherorte_pfad().display());
        if !weitere.is_empty() {
            info.push_str(&format!(" · weitere Orte für das Netz: {}", weitere.join(", ")));
        }
        zeile(&mut aus, &info, t.beiwerk);
    } else {
        zeile(&mut aus, &format!("  ⚑ {}", s.meldung), t.akzent);
    }
    zeile(&mut aus, "", t.beiwerk);
    zeile(&mut aus, KUERZEL, t.kante);
    let _ = aus.flush();
}

/// Wie viele Zeilen der Satz unter der Liste hoechstens hat.
const HINWEISZEILEN: usize = 6;

/// Bricht an Wortgrenzen um; ein Wort, das allein zu lang ist, steht in
/// seiner eigenen Zeile und wird dort gekuerzt.
fn umbrechen(text: &str, breite: usize) -> Vec<String> {
    let breite = breite.max(10);
    let mut aus: Vec<String> = Vec::new();
    let mut zeile = String::new();
    for wort in text.split_whitespace() {
        if !zeile.is_empty() && zeile.chars().count() + 1 + wort.chars().count() > breite {
            aus.push(std::mem::take(&mut zeile));
        }
        if !zeile.is_empty() {
            zeile.push(' ');
        }
        zeile.push_str(wort);
    }
    if !zeile.is_empty() {
        aus.push(zeile);
    }
    aus.into_iter().map(|z| kuerzen(&z, breite)).collect()
}

fn kuerzen(text: &str, breite: usize) -> String {
    let n = text.chars().count();
    if n <= breite {
        return text.to_string();
    }
    text.chars().take(breite.saturating_sub(1)).chain(['…']).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Jede Reihe ist erreichbar und der Knopf steht zuletzt**, damit
    /// Enter auf der Vorauswahl startet oder beendet.
    #[test]
    fn der_knopf_steht_zuletzt_und_jedes_rechenwerk_hat_eine_reihe() {
        let hw = myl_client::hardware::erheben(&std::env::temp_dir());
        let werke = mining::werke(&hw, myl_client::einstellungen::Sprache::De);
        let r = reihen(&werke);
        assert_eq!(r.last(), Some(&Reihe::Knopf));
        assert_eq!(r.iter().filter(|x| matches!(x, Reihe::Rechenwerk(_))).count(), hw.rechenwerke.len());
        for x in &r {
            assert!(!x.hinweis(&werke).is_empty());
        }
    }

    /// ⚑ **Ein gesperrtes Rechenwerk laesst sich nicht schalten** und sagt
    /// unter der Liste, warum; ein freies schaltet hin und her.
    #[test]
    fn ein_gesperrtes_rechenwerk_schaltet_nicht_und_nennt_den_grund() {
        let werke = vec![
            Werkwahl { kennung: "frei".into(), titel: "Apple M5 Pro (Metal)".into(), rechnet: true, sperrgrund: None },
            Werkwahl { kennung: "zu".into(), titel: "Intel UHD".into(), rechnet: false, sperrgrund: Some("Kein Rechenweg.".into()) },
        ];
        let f = Freigabe { platte: "/p".into(), ordner: "m".into(), volumen_gib: 1, gewichte: false, kerne: 1, rechenwerke: vec![] };
        let hw = myl_client::hardware::erheben(&std::env::temp_dir());
        let mut s = Seite { f, platten: vec![], hw, werke, laeuft: None, stand: vec![], meldung: String::new(), geaendert: false };
        assert!(!Reihe::Rechenwerk(1).mit_pfeilen(&s.werke));
        s.aendern(Reihe::Rechenwerk(1), true);
        assert!(s.f.rechenwerke.is_empty() && !s.geaendert, "die gesperrte Karte wurde geschaltet");
        assert_eq!(Reihe::Rechenwerk(1).hinweis(&s.werke), "Kein Rechenweg.");
        assert!(s.wert(Reihe::Rechenwerk(1)).contains("gesperrt"));
        s.aendern(Reihe::Rechenwerk(0), true);
        assert_eq!(s.f.rechenwerke, ["frei"]);
        assert!(s.wert(Reihe::Rechenwerk(0)).starts_with("Apple M5 Pro (Metal) · freigegeben"));
        s.aendern(Reihe::Rechenwerk(0), false);
        assert!(s.f.rechenwerke.is_empty());
    }

    #[test]
    fn der_hinweis_wird_umgebrochen_und_nicht_abgeschnitten() {
        let z = umbrechen("eins zwei drei vier fünf sechs sieben acht", 12);
        assert_eq!(z, ["eins zwei", "drei vier", "fünf sechs", "sieben acht"]);
    }

    /// ⛔️ **Kein Start an der Pruefung vorbei**: Die Seite startet nur ueber
    /// `mining::starten`, und das prueft zuerst.
    #[test]
    fn gestartet_wird_nur_ueber_die_kiste() {
        let quelle = include_str!("miningseite.rs");
        let rumpf = quelle.split("#[cfg(test)]").next().unwrap();
        assert_eq!(rumpf.matches("mining::starten(").count(), 1);
        assert!(!rumpf.contains("Command::new"), "die Seite startet selbst einen Prozess");
    }
}
