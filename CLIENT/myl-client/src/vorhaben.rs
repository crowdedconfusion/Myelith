//! **Vorhaben: ein Ziel, das der Agent ueber viele Runden verfolgt.**
//!
//! # ⚑ Was hier neu ist
//!
//! Ein Agentenlauf (`crate::lauf`) faehrt einen Auftrag bis zum Ende und
//! vergisst ihn dann. Ein Vorhaben traegt ein Ziel **ueber Runden
//! hinweg**: Jede Runde ist ein gewoehnlicher Lauf, und zwischen den
//! Runden bleibt, was der Agent festgehalten hat (Notizen), was er getan
//! hat (Tagebuch) und wann er weitermachen will (Selbstwecken). Ein
//! fertiges Vorhaben kann ein naechstes anstossen (Kette).
//!
//! # ⚑ Festlegungen des Projektinhabers (2026-09-26)
//!
//! - **Kein eigener Prozess, keine Zeitsteuerung des Betriebssystems.**
//!   Der Loop laeuft im Fenster oder in der Konsole. Wird eines davon
//!   geschlossen, haelt er an und macht beim naechsten Oeffnen **genau
//!   dort** weiter: Eine unterbrochene Runde setzt mit dem fort, was sie
//!   schon erledigt hatte (`Rundenstand`), eine Wartezeit mit ihrem Rest,
//!   und die Hoechstdauer zaehlt nur offene Zeit.
//! - **Ausloeser sind Selbstwecken und Kette**, auch zusammen.
//! - **Aufsicht wie beim normalen Agenten**: Im `manual mode` legt die
//!   Ruestung jede Handlung mit Wirkung nach aussen vor; wer auf `auto`
//!   stellt, bekommt vorher eine Sicherheitsmeldung.
//! - **Der Pruefdurchgang ist eine Einstellung** (`loop.pruefen`).
//!
//! # ⛔️ Grenzen stehen im Code
//!
//! Runden, Dauer, Schritte je Runde und Stillstand sind Zahlen aus
//! `Loopeinstellung`, und ein Vorhaben endet an ihnen, gleich was das
//! Modell meint. Das Modell kann sich weder mehr Runden geben noch seine
//! Wartezeit ueber `WECKEN_BIS` hinausschieben.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use myl_local_agent::ausfuehrung::{Werkzeugausfuehrung, Werkzeugfehler};
use myl_local_agent::tuerklient::{Modellweg, Nachricht};
use myl_local_agent::werkzeug::Werkzeug;
use serde::{Deserialize, Serialize};

use crate::einstellungen::{Loopeinstellung, Sprache};

/// Kuerzeste und laengste Wartezeit, die das Modell sich geben darf.
pub const WECKEN_AB_MINUTEN: u64 = 1;
pub const WECKEN_BIS_MINUTEN: u64 = 7 * 24 * 60;

/// Obergrenzen des Notizblocks.
///
/// ⚑ **Klein, weil er in jeder Runde im Kontext steht.** Ein Notizblock,
/// der waechst, verdraengt irgendwann das Ziel aus dem Kontext eines
/// kleinen Modells.
pub const NOTIZEN_HOECHSTENS: usize = 24;
pub const NOTIZ_ZEICHEN: usize = 600;

/// Wie viele Tagebucheintraege eine Runde zu sehen bekommt.
pub const TAGEBUCH_RUECKBLICK: usize = 3;

/// Wo ein Vorhaben gerade steht.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "art", rename_all = "snake_case")]
pub enum Zustand {
    /// Faellig, sobald der Loop laeuft.
    Bereit,
    /// Wartet, bis `Vorhaben::naechste` erreicht ist.
    Schlaeft,
    /// Ziel erreicht.
    Fertig,
    /// Angehalten, mit Grund; weiter nur, wenn ein Mensch es fortsetzt.
    Angehalten { grund: String },
}

/// Ein Vorhaben, so wie es auf der Platte liegt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vorhaben {
    pub kennung: String,
    pub ziel: String,
    pub angelegt: u64,
    pub zustand: Zustand,
    /// Abgeschlossene Runden.
    pub runden: u32,
    /// Sekunden, in denen der Loop offen war und dieses Vorhaben aktiv.
    pub laufzeit: u64,
    /// Unix-Sekunden, ab denen die naechste Runde faellig ist.
    pub naechste: u64,
    /// ⚑ **Die Restwartezeit beim Anhalten.** Beim Schliessen wird aus
    ///   „faellig um 14:00" ein „faellig in 90 Minuten", beim Oeffnen
    ///   wieder ein Zeitpunkt. So wartet ein Vorhaben nach dem Oeffnen
    ///   genau den Rest, nicht null und nicht von vorn.
    #[serde(default)]
    pub rest: Option<u64>,
    pub notizen: BTreeMap<String, String>,
    /// Ziele, die nach dem Fertigwerden als neue Vorhaben beginnen.
    pub anschluss: Vec<String>,
    pub ergebnis: Option<String>,
    pub ohne_fortschritt: u32,
    #[serde(default)]
    pub vorgaenger: Option<String>,
    /// ⚑ **Der Abnahmebefehl**, vom Menschen beim Anlegen genannt (Punkt 4.7,
    /// Befund des Loop-Szenarios). Nach jeder Runde laeuft er im
    /// Arbeitsordner; **endet er mit 0, ist das Vorhaben fertig**, sonst
    /// nie, gleich was Modell und Pruefung sagen.
    #[serde(default)]
    pub abnahme: Option<String>,
}

impl Vorhaben {
    pub fn neu(kennung: String, ziel: &str, jetzt: u64) -> Self {
        Self {
            kennung,
            ziel: ziel.trim().to_string(),
            angelegt: jetzt,
            zustand: Zustand::Bereit,
            runden: 0,
            laufzeit: 0,
            naechste: jetzt,
            rest: None,
            notizen: BTreeMap::new(),
            anschluss: Vec::new(),
            ergebnis: None,
            ohne_fortschritt: 0,
            vorgaenger: None,
            abnahme: None,
        }
    }

    /// Laeuft es noch, im weitesten Sinn (bereit oder schlafend)?
    pub fn aktiv(&self) -> bool {
        matches!(self.zustand, Zustand::Bereit | Zustand::Schlaeft)
    }

    /// Ist eine Runde faellig?
    pub fn faellig(&self, jetzt: u64) -> bool {
        self.aktiv() && self.rest.is_none() && self.naechste <= jetzt
    }
}

/// Was von einer unterbrochenen Runde schon erledigt war.
///
/// ⚑ **Nach jedem Werkzeugergebnis geschrieben.** Stirbt der Prozess
/// mitten in der Runde (Fenster zu), liegt hier, was schon getan ist,
/// und die fortgesetzte Runde bekommt es vorgelegt, statt es zu
/// wiederholen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rundenstand {
    pub erledigt: Vec<Erledigt>,
    /// Notizen, wie sie beim letzten Werkzeugschritt standen.
    pub notizen: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Erledigt {
    pub werkzeug: String,
    pub argumente: String,
    pub ergebnis: String,
}

/// Die Ablage aller Vorhaben: ein Ordner je Vorhaben.
///
/// `<ordner>/<kennung>/vorhaben.json`, `tagebuch.md` und, waehrend einer
/// Runde, `runde.json`.
#[derive(Debug, Clone)]
pub struct Ablage {
    pub ordner: PathBuf,
}

impl Ablage {
    pub fn neu(ordner: impl Into<PathBuf>) -> Self {
        Self { ordner: ordner.into() }
    }

    /// Neben den Einstellungen: `…/myelith/vorhaben`.
    pub fn vorgabe() -> Self {
        let e = crate::einstellungen::Einstellungen::vorgabepfad();
        let basis = e.parent().map(Path::to_path_buf).unwrap_or_default();
        Self::neu(basis.join("vorhaben"))
    }

    fn pfad(&self, kennung: &str) -> PathBuf {
        self.ordner.join(kennung)
    }

    /// Legt ein neues Vorhaben an. Die Kennung ist Zeit plus Zaehler, damit
    /// die Reihenfolge im Ordner der Reihenfolge des Anlegens entspricht.
    pub fn anlegen(&self, ziel: &str, jetzt: u64) -> Result<Vorhaben, String> {
        if ziel.trim().is_empty() {
            return Err("ein Vorhaben braucht ein Ziel".into());
        }
        std::fs::create_dir_all(&self.ordner).map_err(|e| e.to_string())?;
        let mut n = 0u32;
        let kennung = loop {
            let k = format!("v{jetzt}-{n}");
            if !self.pfad(&k).exists() {
                break k;
            }
            n += 1;
        };
        let v = Vorhaben::neu(kennung, ziel, jetzt);
        self.speichern(&v)?;
        Ok(v)
    }

    pub fn speichern(&self, v: &Vorhaben) -> Result<(), String> {
        let d = self.pfad(&v.kennung);
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
        schreiben_sicher(&d.join("vorhaben.json"), &text)
    }

    pub fn laden(&self, kennung: &str) -> Result<Vorhaben, String> {
        let p = self.pfad(kennung).join("vorhaben.json");
        let t = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))
    }

    /// Alle Vorhaben, aelteste zuerst. Kaputte Eintraege werden
    /// uebergangen und nicht geloescht: Sie gehoeren dem Menschen.
    pub fn alle(&self) -> Vec<Vorhaben> {
        let Ok(es) = std::fs::read_dir(&self.ordner) else { return Vec::new() };
        let mut namen: Vec<String> =
            es.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        namen.sort();
        namen.iter().filter_map(|k| self.laden(k).ok()).collect()
    }

    pub fn tagebuch_anhaengen(&self, kennung: &str, runde: u32, text: &str) -> Result<(), String> {
        use std::io::Write;
        let p = self.pfad(kennung).join("tagebuch.md");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .map_err(|e| e.to_string())?;
        writeln!(f, "## Runde {runde}\n\n{}\n", text.trim()).map_err(|e| e.to_string())
    }

    /// Die letzten `n` Tagebucheintraege, aelteste zuerst.
    pub fn tagebuch_letzte(&self, kennung: &str, n: usize) -> Vec<String> {
        let Ok(t) = std::fs::read_to_string(self.pfad(kennung).join("tagebuch.md")) else {
            return Vec::new();
        };
        let eintraege: Vec<String> = t
            .split("## Runde ")
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("Runde {}", s.trim()))
            .collect();
        let ab = eintraege.len().saturating_sub(n);
        eintraege[ab..].to_vec()
    }

    /// ⚑ **War der Loop aktiv, als geschlossen wurde?** Die Marke steht,
    /// solange er laeuft; Schliessen laesst sie stehen, ein bewusstes
    /// Pausieren nimmt sie weg. Beim Oeffnen faehrt der Loop genau dann
    /// von selbst weiter, wenn sie steht (Festlegung des
    /// Projektinhabers: exakt dort weiter, wo gestoppt).
    pub fn loop_war_aktiv(&self) -> bool {
        self.ordner.join(".aktiv").exists()
    }

    pub fn loop_aktiv_setzen(&self, aktiv: bool) -> Result<(), String> {
        let p = self.ordner.join(".aktiv");
        if aktiv {
            std::fs::create_dir_all(&self.ordner).map_err(|e| e.to_string())?;
            std::fs::write(p, "").map_err(|e| e.to_string())
        } else {
            match std::fs::remove_file(p) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        }
    }

    pub fn rundenstand(&self, kennung: &str) -> Option<Rundenstand> {
        let t = std::fs::read_to_string(self.pfad(kennung).join("runde.json")).ok()?;
        serde_json::from_str(&t).ok()
    }

    pub fn rundenstand_schreiben(&self, kennung: &str, r: &Rundenstand) -> Result<(), String> {
        let text = serde_json::to_string_pretty(r).map_err(|e| e.to_string())?;
        schreiben_sicher(&self.pfad(kennung).join("runde.json"), &text)
    }

    pub fn rundenstand_loeschen(&self, kennung: &str) {
        let _ = std::fs::remove_file(self.pfad(kennung).join("runde.json"));
    }

    // ── Die Warteschlange ──

    /// **Alle Vorhaben in ihrer geplanten Reihenfolge.**
    ///
    /// ⚑ **Es laeuft immer nur das vorderste aktive**, alle dahinter
    /// warten (Festlegung des Projektinhabers, 2026-09-26: „(läuft)" und
    /// „(queued)"). Die Reihenfolge steht in `reihe.json` und laesst sich
    /// umstellen; was dort fehlt (neu angelegt, von Hand kopiert), steht
    /// hinten, aelteste zuerst. Eine Kennung in der Datei, zu der es kein
    /// Vorhaben mehr gibt, faellt still heraus.
    ///
    /// ⚠️ Eine Datei und keine Nummer im Vorhaben: Umstellen aendert dann
    /// eine Stelle statt jedes Vorhabens, und ein halb gespeichertes
    /// Umstellen gibt es nicht.
    pub fn reihe(&self) -> Vec<Vorhaben> {
        let mut rest = self.alle();
        let mut aus = Vec::with_capacity(rest.len());
        for k in self.reihe_lesen() {
            if let Some(i) = rest.iter().position(|v| v.kennung == k) {
                aus.push(rest.remove(i));
            }
        }
        aus.extend(rest);
        aus
    }

    fn reihe_lesen(&self) -> Vec<String> {
        std::fs::read_to_string(self.ordner.join("reihe.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Stellt die Reihenfolge um. Kennungen, die es nicht gibt, werden
    /// uebergangen; fehlende rutschen nach hinten (siehe [`Ablage::reihe`]).
    pub fn reihe_setzen(&self, kennungen: &[String]) -> Result<(), String> {
        let da: std::collections::BTreeSet<String> = self.alle().into_iter().map(|v| v.kennung).collect();
        let mut neu: Vec<&String> = Vec::new();
        for k in kennungen {
            if da.contains(k) && !neu.contains(&k) {
                neu.push(k);
            }
        }
        std::fs::create_dir_all(&self.ordner).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(&neu).map_err(|e| e.to_string())?;
        schreiben_sicher(&self.ordner.join("reihe.json"), &text)
    }

    /// ⚑ **Eine Kette reiht sich direkt hinter ihrem Vorgaenger ein**,
    /// nicht hinten: Sie ist die Fortsetzung dessen, was gerade lief, und
    /// soll nicht hinter allem warten, was inzwischen angelegt wurde.
    pub fn einreihen_nach(&self, neu: &str, nach: &str) -> Result<(), String> {
        let mut k: Vec<String> = self.reihe().into_iter().map(|v| v.kennung).filter(|x| x != neu).collect();
        let stelle = k.iter().position(|x| x == nach).map(|i| i + 1).unwrap_or(k.len());
        k.insert(stelle, neu.to_string());
        self.reihe_setzen(&k)
    }

    /// Das Vorhaben, das als naechstes laeuft: das vorderste aktive.
    pub fn vorn(&self) -> Option<Vorhaben> {
        self.reihe().into_iter().find(Vorhaben::aktiv)
    }

    /// Wo jedes Vorhaben in der Schlange steht, in ihrer Reihenfolge.
    pub fn stellungen(&self) -> Vec<(Vorhaben, Stellung)> {
        let mut vorn_vergeben = false;
        self.reihe()
            .into_iter()
            .map(|v| {
                let s = match &v.zustand {
                    Zustand::Fertig => Stellung::Fertig,
                    Zustand::Angehalten { .. } => Stellung::Angehalten,
                    _ if !vorn_vergeben => {
                        vorn_vergeben = true;
                        Stellung::Vorn
                    }
                    _ => Stellung::Wartend,
                };
                (v, s)
            })
            .collect()
    }

    /// **Ein Mensch setzt fort**: bereit, sofort faellig, Stillstand von vorn.
    pub fn weitermachen(&self, kennung: &str) -> Result<Vorhaben, String> {
        let mut v = self.laden(kennung)?;
        if v.zustand == Zustand::Fertig {
            return Err(format!("{kennung} ist fertig"));
        }
        v.zustand = Zustand::Bereit;
        v.ohne_fortschritt = 0;
        v.naechste = jetzt();
        v.rest = None;
        self.speichern(&v)?;
        Ok(v)
    }

    /// **Setzt oder loescht den Abnahmebefehl** (Punkt 4.7). Ein leerer
    /// Befehl loescht ihn.
    pub fn abnahme_setzen(&self, kennung: &str, befehl: &str) -> Result<Vorhaben, String> {
        let mut v = self.laden(kennung)?;
        let b = befehl.trim();
        v.abnahme = (!b.is_empty()).then(|| b.to_string());
        self.speichern(&v)?;
        Ok(v)
    }

    /// **Ein Mensch haelt an.** Das Vorhaben bleibt, mit Notizen und
    /// Tagebuch; `weitermachen` holt es zurueck.
    pub fn stoppen(&self, kennung: &str, sprache: Sprache) -> Result<Vorhaben, String> {
        let mut v = self.laden(kennung)?;
        let grund = if sprache == Sprache::De { "vom Menschen angehalten" } else { "paused by a human" };
        v.zustand = Zustand::Angehalten { grund: grund.into() };
        self.speichern(&v)?;
        Ok(v)
    }

    /// **Entfernt ein Vorhaben samt Tagebuch.**
    ///
    /// ⛔️ Nur auf ausdruecklichen Wunsch; der Aufrufer fragt vorher. Der
    /// Ordner muss ein Vorhaben sein, sonst wird nichts geloescht: Eine
    /// Kennung wie `..` duerfte sonst anderswo loeschen.
    pub fn entfernen(&self, kennung: &str) -> Result<(), String> {
        if kennung.is_empty() || !kennung.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(format!("keine Kennung: {kennung}"));
        }
        let d = self.pfad(kennung);
        if !d.join("vorhaben.json").is_file() {
            return Err(format!("kein Vorhaben {kennung}"));
        }
        std::fs::remove_dir_all(&d).map_err(|e| e.to_string())?;
        let rest: Vec<String> = self.reihe().into_iter().map(|v| v.kennung).collect();
        self.reihe_setzen(&rest)
    }
}

impl Ablage {
    /// **Faehrt gerade jemand den Loop?** Die Sperre steht, und ihr Prozess
    /// lebt. Fuer die Anzeige „(läuft)" in einem Prozess, der selbst nicht
    /// faehrt.
    pub fn laeuft_gerade(&self) -> bool {
        std::fs::read_to_string(self.ordner.join(".laeufer"))
            .ok()
            .and_then(|t| t.split_whitespace().next().and_then(|p| p.parse::<u32>().ok()))
            .is_some_and(lebt)
    }

    /// **Die Liste fuer Konsole und `myl`**, eine Zeile je Vorhaben, in
    /// der Reihenfolge der Schlange.
    pub fn liste(&self, sprache: Sprache) -> Vec<String> {
        let an = self.laeuft_gerade();
        let runden = if sprache == Sprache::De { "Runden" } else { "rounds" };
        self.stellungen()
            .into_iter()
            .enumerate()
            .map(|(i, (v, st))| {
                format!("{:>2}. {}  ({})  {} {runden}  {}", i + 1, v.kennung, st.wort(&v, an, sprache), v.runden, v.ziel)
            })
            .collect()
    }
}

/// **Wie ein Vorhaben nach einer Runde steht**, in einem Satz: weiter,
/// naechste Runde in, fertig, angehalten. ⚑ Ein Text fuer `myl`, Konsole
/// und Fenster.
pub fn zustandswort(v: &Vorhaben, sprache: Sprache) -> String {
    let de = sprache == Sprache::De;
    match &v.zustand {
        Zustand::Bereit => (if de { "weiter" } else { "continues" }).into(),
        Zustand::Schlaeft => {
            let min = v.naechste.saturating_sub(jetzt()).div_ceil(60);
            if de { format!("nächste Runde in {min} min") } else { format!("next round in {min} min") }
        }
        Zustand::Fertig if de => format!("fertig: {}", v.ergebnis.as_deref().unwrap_or("")),
        Zustand::Fertig => format!("done: {}", v.ergebnis.as_deref().unwrap_or("")),
        Zustand::Angehalten { grund } if de => format!("angehalten: {grund}"),
        Zustand::Angehalten { grund } => format!("stopped: {grund}"),
    }
}

/// Wo ein Vorhaben in der Schlange steht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stellung {
    /// Das vorderste aktive: Es laeuft, sobald der Loop an ist.
    Vorn,
    /// Aktiv, aber hinter einem anderen.
    Wartend,
    Fertig,
    Angehalten,
}

impl Stellung {
    /// ⚑ **Ein Wort fuer Fenster, Konsole und `myl`**, damit alle drei
    /// dasselbe sagen. „queued" auch im Deutschen, so hat es der
    /// Projektinhaber gesetzt.
    pub fn wort(self, v: &Vorhaben, loop_an: bool, sprache: Sprache) -> String {
        let de = sprache == Sprache::De;
        match self {
            Stellung::Vorn if loop_an && v.zustand == Zustand::Schlaeft => {
                let min = v.naechste.saturating_sub(jetzt()).div_ceil(60);
                if de { format!("läuft, nächste Runde in {min} min") } else { format!("running, next round in {min} min") }
            }
            Stellung::Vorn if loop_an => (if de { "läuft" } else { "running" }).into(),
            Stellung::Vorn if v.rest.is_some() => {
                let min = v.rest.unwrap_or(0).div_ceil(60);
                if de { format!("pausiert, wartet noch {min} min") } else { format!("paused, {min} min left to wait") }
            }
            Stellung::Vorn if v.runden > 0 => (if de { "pausiert" } else { "paused" }).into(),
            Stellung::Vorn => (if de { "als Nächstes" } else { "next" }).into(),
            Stellung::Wartend => "queued".into(),
            Stellung::Fertig => (if de { "fertig" } else { "done" }).into(),
            Stellung::Angehalten => match &v.zustand {
                Zustand::Angehalten { grund } if de => format!("angehalten: {grund}"),
                Zustand::Angehalten { grund } => format!("stopped: {grund}"),
                _ => String::new(),
            },
        }
    }
}

/// Schreibt erst in eine Nebendatei und benennt dann um.
///
/// ⚑ **Weil der Prozess jederzeit enden darf** (Fenster zu). Ein halb
/// geschriebenes `vorhaben.json` waere ein Vorhaben, das beim naechsten
/// Oeffnen nicht mehr lesbar ist; das Umbenennen ist atomar.
fn schreiben_sicher(ziel: &Path, text: &str) -> Result<(), String> {
    let neben = ziel.with_extension("json.neu");
    std::fs::write(&neben, text).map_err(|e| e.to_string())?;
    std::fs::rename(&neben, ziel).map_err(|e| e.to_string())
}

// ── Die Werkzeuge einer Runde ───────────────────────────────────────

/// Was das Modell in einer Runde ueber das Vorhaben entschieden hat.
#[derive(Debug, Clone, Default)]
pub struct Rundenwahl {
    pub notizen: BTreeMap<String, String>,
    pub notizen_geaendert: bool,
    pub wecken_minuten: Option<u64>,
    pub fertig: Option<String>,
    pub anschluss: Vec<String>,
}

type Geteilt = Arc<Mutex<Rundenwahl>>;

fn text_arg(a: &serde_json::Value, name: &str) -> Result<String, Werkzeugfehler> {
    a.get(name)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .ok_or_else(|| Werkzeugfehler { grund: format!("argument `{name}` (text) is missing") })
}

/// Werkzeuge, die nur lesen: Ein zweiter gleicher Aufruf ohne Aenderung
/// dazwischen liefert nichts Neues.
const LESEND: [&str; 16] = [
    "list_directory", "read_file", "search_files", "search_skill", "learn_skill", "read_history",
    "list_history", "search_history", "verzeichnis", "datei_lesen", "suchen", "skill_suchen",
    "skill_lernen", "verlauf_lesen", "verlauf_liste", "verlauf_suchen",
];

/// Werkzeuge, die weder lesen noch die Welt aendern: die des Loops selbst.
const OHNE_WIRKUNG: [&str; 4] = ["note_set", "wake_in", "finish_goal", "chain_goal"];

/// **Die Wiederholungsbremse**: Ein lesender Aufruf, der in dieser Runde
/// genauso schon lief, und seither hat sich nichts geaendert, laeuft
/// nicht noch einmal.
///
/// 📌 **Fund 488 (2026-09-26):** Das 8B drehte sich im Loop-Szenario in
/// einer Runde viermal durch dieselben vier Aufrufe (Skill suchen, Skill
/// lernen, suchen, lesen), bis die Schrittgrenze kam. Die Erkennung
/// „steckengeblieben" der Schleife sieht nur **aufeinanderfolgende**
/// Wiederholungen; ein Kreis aus vier fiel durch.
///
/// ⚑ **„Nichts geaendert" ist genau gemeint:** Jeder nicht lesende Aufruf
/// (schreiben, aendern, Befehl, Notiz) zaehlt als moegliche Aenderung. Ein
/// `read_file` nach einem `edit_file` laeuft also wieder, ein zweites
/// `run_command` ohnehin.
struct Wiederholungsbremse {
    inner: Box<dyn Werkzeugausfuehrung>,
    gesehen: Arc<Mutex<std::collections::HashMap<(String, String), u64>>>,
    welt: Arc<std::sync::atomic::AtomicU64>,
    de: bool,
}

impl Werkzeugausfuehrung for Wiederholungsbremse {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        use std::sync::atomic::Ordering;
        let name = self.inner.name().to_string();
        // 📌 **Nachtrag zu Fund 488 (Lauf 3 des Szenarios):** Ein
        //    `note_set` zwischen zwei gleichen Suchen hob die Bremse auf, denn
        //    es zaehlte als Aenderung. Eine Notiz, ein Wecken und die reinen
        //    Rechenwerkzeuge der verankerten Kiste aendern keine Datei; sie
        //    zaehlen weder als Aenderung noch werden sie gebremst.
        if OHNE_WIRKUNG.contains(&name.as_str()) || crate::verankert::Verankert::ALLE.iter().any(|v| v.name() == name) {
            return self.inner.ausfuehren(a);
        }
        if !LESEND.contains(&name.as_str()) {
            let r = self.inner.ausfuehren(a);
            self.welt.fetch_add(1, Ordering::SeqCst);
            return r;
        }
        let stand = self.welt.load(Ordering::SeqCst);
        let schluessel = (name, a.to_string());
        {
            let mut g = self.gesehen.lock().expect("Wiederholungsbremse");
            if g.get(&schluessel) == Some(&stand) {
                return Ok(if self.de {
                    "(Nicht noch einmal ausgeführt: Genau dieser Aufruf lief in dieser Runde schon, und seither \
                     hat sich nichts geändert. Das Ergebnis steht weiter oben. Mach mit dem nächsten Schritt \
                     weiter: einer Änderung, einem Befehl oder deiner Schlussantwort.)"
                        .into()
                } else {
                    "(Not run again: exactly this call already ran in this round, and nothing has changed since. \
                     Its result is above. Continue with the next step: a change, a command or your final answer.)"
                        .into()
                });
            }
            g.insert(schluessel, stand);
        }
        self.inner.ausfuehren(a)
    }
}

/// Werkzeuge, die eine Datei schreiben und einen `pfad` tragen.
const SCHREIBEND: [&str; 4] = ["write_file", "edit_file", "datei_schreiben", "datei_aendern"];

/// **Der Pendelwaechter** (Punkt 4.8): merkt sich je Datei die Staende
/// dieser Runde und sagt dazu, wenn eine Aenderung nichts aenderte oder
/// eine Datei auf einen frueheren Stand zurueckfiel.
///
/// 📌 **Lauf 4 des Loop-Szenarios (2026-09-26):** Das 8B aenderte dieselbe
/// Zeile fuenfzehnmal zwischen `zeile["sensor"]` und `zeile["sens"]` hin
/// und her und ersetzte sie fuenfmal durch sich selbst. Die
/// Wiederholungsbremse sieht das nicht, denn jede dieser Aenderungen ist
/// ein anderer Aufruf mit Wirkung. **Ein Kreis ueber zwei Zustaende ist
/// nur am Zustand zu erkennen.**
///
/// ⚑ Er verhindert nichts, er sagt es an: Ein Zurueck kann auch richtig
/// sein, etwa nach einem missglueckten Versuch.
struct Pendelwaechter {
    inner: Box<dyn Werkzeugausfuehrung>,
    wurzel: PathBuf,
    staende: Arc<Mutex<std::collections::HashMap<String, Vec<u64>>>>,
    de: bool,
}

fn inhalt_stand(p: &Path) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let t = std::fs::read(p).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    Some(h.finish())
}

impl Werkzeugausfuehrung for Pendelwaechter {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let Some(pfad) = a.get("pfad").and_then(|x| x.as_str()).map(str::to_string) else {
            return self.inner.ausfuehren(a);
        };
        let datei = self.wurzel.join(&pfad);
        let vorher = inhalt_stand(&datei);
        let r = self.inner.ausfuehren(a);
        let Ok(text) = r else { return r };
        let Some(nachher) = inhalt_stand(&datei) else { return Ok(text) };
        let mut g = self.staende.lock().expect("Pendelwaechter");
        let liste = g.entry(pfad.clone()).or_default();
        if liste.is_empty() {
            if let Some(v) = vorher {
                liste.push(v);
            }
        }
        let hinweis = if vorher == Some(nachher) {
            Some(if self.de {
                format!("\n[Hinweis: {pfad} ist danach genau wie vorher; diese Änderung hat nichts geändert.]")
            } else {
                format!("\n[Note: {pfad} is exactly as before; this change changed nothing.]")
            })
        } else if liste.contains(&nachher) {
            Some(if self.de {
                format!(
                    "\n[Hinweis: {pfad} hat jetzt wieder einen Stand, den die Datei in dieser Runde schon hatte. \
                     Du drehst dich im Kreis; prüfe die Ursache mit einem anderen Ansatz, statt dieselben Stände zu wechseln.]"
                )
            } else {
                format!(
                    "\n[Note: {pfad} is now back in a state it already had in this round. You are going in circles; \
                     find the cause with a different approach instead of switching between the same states.]"
                )
            })
        } else {
            None
        };
        liste.push(nachher);
        Ok(match hinweis {
            Some(h) => text + &h,
            None => text,
        })
    }
}

/// Liefert fuer einen schon erledigten Aufruf das gespeicherte Ergebnis.
struct Doppelsperre {
    inner: Box<dyn Werkzeugausfuehrung>,
    erledigt: Arc<std::collections::HashMap<(String, String), String>>,
    de: bool,
}

impl Werkzeugausfuehrung for Doppelsperre {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let schluessel = (self.inner.name().to_string(), a.to_string());
        match self.erledigt.get(&schluessel) {
            Some(ergebnis) if self.de => Ok(format!(
                "(Bereits vor der Unterbrechung erledigt, nicht noch einmal ausgeführt. Ergebnis von damals:) {ergebnis}"
            )),
            Some(ergebnis) => Ok(format!("(Already done before the interruption, not run again. Result then:) {ergebnis}")),
            None => self.inner.ausfuehren(a),
        }
    }
}

struct Notiz(Geteilt);
struct Wecken(Geteilt);
struct Fertig(Geteilt);
struct Anschluss(Geteilt);

impl Werkzeugausfuehrung for Notiz {
    fn name(&self) -> &str {
        "note_set"
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let schluessel = text_arg(a, "key")?;
        let wert = a.get("value").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        if schluessel.is_empty() {
            return Err(Werkzeugfehler { grund: "the key must not be empty".into() });
        }
        let mut w = self.0.lock().expect("Rundenwahl");
        if wert.is_empty() {
            w.notizen.remove(&schluessel);
            w.notizen_geaendert = true;
            return Ok(format!("note `{schluessel}` removed"));
        }
        if !w.notizen.contains_key(&schluessel) && w.notizen.len() >= NOTIZEN_HOECHSTENS {
            return Err(Werkzeugfehler {
                grund: format!("the notepad is full ({NOTIZEN_HOECHSTENS} notes); remove one with an empty value first"),
            });
        }
        let gekuerzt: String = wert.chars().take(NOTIZ_ZEICHEN).collect();
        let hinweis = if gekuerzt.len() < wert.len() { " (shortened)" } else { "" };
        if w.notizen.get(&schluessel) != Some(&gekuerzt) {
            w.notizen.insert(schluessel.clone(), gekuerzt);
            w.notizen_geaendert = true;
        }
        Ok(format!("note `{schluessel}` saved{hinweis}"))
    }
}

impl Werkzeugausfuehrung for Wecken {
    fn name(&self) -> &str {
        "wake_in"
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let m = a
            .get("minutes")
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f.max(0.0) as u64)))
            .ok_or_else(|| Werkzeugfehler { grund: "argument `minutes` (number) is missing".into() })?;
        let m = m.clamp(WECKEN_AB_MINUTEN, WECKEN_BIS_MINUTEN);
        self.0.lock().expect("Rundenwahl").wecken_minuten = Some(m);
        Ok(format!("the next round starts in {m} minutes"))
    }
}

impl Werkzeugausfuehrung for Fertig {
    fn name(&self) -> &str {
        "finish_goal"
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let r = text_arg(a, "result")?;
        self.0.lock().expect("Rundenwahl").fertig = Some(r);
        Ok("marked as finished; answer with a short summary".into())
    }
}

impl Werkzeugausfuehrung for Anschluss {
    fn name(&self) -> &str {
        "chain_goal"
    }
    fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
        let z = text_arg(a, "goal")?;
        if z.is_empty() {
            return Err(Werkzeugfehler { grund: "the goal must not be empty".into() });
        }
        let mut w = self.0.lock().expect("Rundenwahl");
        if w.anschluss.len() >= 3 {
            return Err(Werkzeugfehler { grund: "at most three follow-up goals per task".into() });
        }
        w.anschluss.push(z);
        Ok("the follow-up goal starts once this task is finished".into())
    }
}

/// Die vier Werkzeuge einer Runde, mit dem geteilten Stand.
pub fn werkzeuge(wahl: &Geteilt) -> Zusatzwerkzeuge {
    let text = |b: &str| serde_json::json!({"type": "string", "description": b});
    vec![
        (
            Werkzeug {
                name: "note_set".into(),
                beschreibung: "Save a note for the next rounds of this long-running task (empty value removes it).".into(),
                parameter: serde_json::json!({"type": "object", "properties": {
                    "key": text("short name of the note"), "value": text("what the next round must know")},
                    "required": ["key", "value"]}),
            },
            Box::new(Notiz(wahl.clone())),
        ),
        (
            Werkzeug {
                name: "wake_in".into(),
                beschreibung: "Start the next round of this task only after the given number of minutes.".into(),
                parameter: serde_json::json!({"type": "object", "properties": {
                    "minutes": {"type": "integer", "description": "minutes until the next round"}},
                    "required": ["minutes"]}),
            },
            Box::new(Wecken(wahl.clone())),
        ),
        (
            Werkzeug {
                name: "finish_goal".into(),
                beschreibung: "Mark the whole task as finished, with its result.".into(),
                parameter: serde_json::json!({"type": "object", "properties": {
                    "result": text("the result of the task")}, "required": ["result"]}),
            },
            Box::new(Fertig(wahl.clone())),
        ),
        (
            Werkzeug {
                name: "chain_goal".into(),
                beschreibung: "Start a new task with this goal once the current task is finished.".into(),
                parameter: serde_json::json!({"type": "object", "properties": {
                    "goal": text("the goal of the follow-up task")}, "required": ["goal"]}),
            },
            Box::new(Anschluss(wahl.clone())),
        ),
    ]
}

// ── Der Auftrag einer Runde ─────────────────────────────────────────

/// Der Auftrag, mit dem eine Runde beginnt.
pub fn auftrag(
    v: &Vorhaben,
    rueckblick: &[String],
    stand: Option<&Rundenstand>,
    grenzen: &Loopeinstellung,
    sprache: Sprache,
) -> String {
    let de = sprache == Sprache::De;
    let mut s = String::new();
    let runde = v.runden + 1;
    s.push_str(&if de {
        format!("Du arbeitest an einem Langzeitvorhaben, Runde {runde} von höchstens {}.\n\nZIEL:\n{}\n\n", grenzen.runden, v.ziel)
    } else {
        format!("You are working on a long-running task, round {runde} of at most {}.\n\nGOAL:\n{}\n\n", grenzen.runden, v.ziel)
    });
    let notizen = stand.map(|r| &r.notizen).unwrap_or(&v.notizen);
    s.push_str(if de { "NOTIZEN (dein Gedächtnis zwischen den Runden):\n" } else { "NOTES (your memory between rounds):\n" });
    if notizen.is_empty() {
        s.push_str(if de { "- noch keine\n" } else { "- none yet\n" });
    }
    for (k, w) in notizen {
        s.push_str(&format!("- {k}: {w}\n"));
    }
    if !rueckblick.is_empty() {
        s.push_str(if de { "\nLETZTE RUNDEN:\n" } else { "\nRECENT ROUNDS:\n" });
        for r in rueckblick {
            s.push_str(&format!("{}\n", r.trim()));
        }
    }
    if let Some(b) = v.abnahme.as_deref() {
        s.push_str(&if de {
            format!("\nABNAHME: Nach jeder Runde wird `{b}` im Arbeitsordner ausgeführt. Das Ziel gilt genau dann als erreicht, wenn dieser Befehl mit 0 endet; seine letzte Ausgabe steht in den Notizen unter „abnahme“.\n")
        } else {
            format!("\nACCEPTANCE: After every round `{b}` runs in the working directory. The goal counts as reached exactly when this command exits with 0; its last output is in the notes under \"abnahme\".\n")
        });
    }
    if let Some(r) = stand.filter(|r| !r.erledigt.is_empty()) {
        s.push_str(if de {
            "\nDIESE RUNDE WURDE UNTERBROCHEN. Bereits erledigt, nicht wiederholen:\n"
        } else {
            "\nTHIS ROUND WAS INTERRUPTED. Already done, do not repeat:\n"
        });
        for (i, e) in r.erledigt.iter().enumerate() {
            let erg: String = e.ergebnis.chars().take(200).collect();
            s.push_str(&format!("{}. {}({}) -> {}\n", i + 1, e.werkzeug, e.argumente, erg));
        }
    }
    s.push_str(if de {
        "\nEin Werkzeug wirkt nur, wenn du es aufrufst. Eine Zeile wie „Ausgeführt: …“ in deiner Antwort führt \
         nichts aus; was wirklich lief, hält das System mit „[System]“ fest.\n"
    } else {
        "\nA tool only has an effect when you call it. A line like \"Executed: ...\" in your answer does \
         nothing; what really ran is recorded by the system with \"[System]\".\n"
    });
    s.push_str(if de {
        "\nTu in dieser Runde den nächsten sinnvollen Schritt auf das Ziel hin. \
         Halte mit note_set fest, was die nächste Runde wissen muss. \
         Ist das Ziel erreicht, rufe finish_goal mit dem Ergebnis. \
         Soll die nächste Runde erst später kommen, rufe wake_in mit Minuten. \
         Soll danach ein weiteres Vorhaben beginnen, rufe chain_goal. \
         Schließe mit einem Satz, was du in dieser Runde getan hast."
    } else {
        "\nIn this round, take the next sensible step towards the goal. \
         Use note_set to record what the next round must know. \
         If the goal is reached, call finish_goal with the result. \
         If the next round should come later, call wake_in with minutes. \
         If another task should start afterwards, call chain_goal. \
         End with one sentence on what you did in this round."
    });
    s
}

// ── Der Pruefdurchgang ──────────────────────────────────────────────

/// Was der Pruefdurchgang ueber eine Runde sagt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pruefung {
    pub fortschritt: bool,
    pub erreicht: bool,
    pub grund: String,
}

/// Die Frage an das Modell, ohne Werkzeuge.
/// Wie viele Werkzeugergebnisse der Pruefdurchgang als Belege sieht, und
/// wie lang jedes hoechstens ist.
pub const PRUEF_BELEGE: usize = 8;
const PRUEF_BELEG_ZEICHEN: usize = 400;

/// **Der Auftrag des Pruefdurchgangs**, mit den Belegen der Runde.
///
/// 📌 **Fund 487 (2026-09-26): Die Pruefung glaubte der Behauptung.** Im
/// Loop-Szenario endete die erste Runde des 8B an der Schrittgrenze, ohne
/// eine Datei geaendert zu haben; der „Bericht" war die Liste der Aufrufe.
/// Die Pruefung sah nur Ziel und Bericht und antwortete „erreicht: ja,
/// das Skript laeuft ohne Fehler". Das Skript brach weiter ab. **Jetzt
/// sieht sie die Werkzeugergebnisse**, die letzten `PRUEF_BELEGE`, und die
/// Regel, dass eine Behauptung ohne Beleg nicht zaehlt.
pub fn pruefauftrag(v: &Vorhaben, wahl: &Rundenwahl, bericht: &str, belege: &[Erledigt], sprache: Sprache) -> Vec<Nachricht> {
    let notizen: Vec<String> = wahl.notizen.iter().map(|(k, w)| format!("- {k}: {w}")).collect();
    let behauptet = wahl.fertig.as_deref().unwrap_or("-");
    let ab = belege.len().saturating_sub(PRUEF_BELEGE);
    let belegt: Vec<String> = belege[ab..]
        .iter()
        .map(|e| {
            let erg: String = e.ergebnis.chars().take(PRUEF_BELEG_ZEICHEN).collect();
            format!("- {}({}) -> {}", e.werkzeug, e.argumente.chars().take(120).collect::<String>(), erg.replace('\n', " "))
        })
        .collect();
    let keine = if sprache == Sprache::De { "(keine: in dieser Runde lief kein Werkzeug)" } else { "(none: no tool ran in this round)" };
    let belegt = if belegt.is_empty() { keine.to_string() } else { belegt.join("\n") };
    let text = if sprache == Sprache::De {
        format!(
            "Prüfe eine Runde eines Langzeitvorhabens.\n\nZIEL:\n{}\n\nNOTIZEN:\n{}\n\nBERICHT DER RUNDE:\n{}\n\n\
             BELEGE (die tatsächlichen Werkzeugergebnisse dieser Runde, die letzten zuerst abgeschnitten):\n{belegt}\n\n\
             DER AGENT MELDET ALS ERGEBNIS: {behauptet}\n\n\
             ERREICHT heißt JA nur, wenn die BELEGE zeigen, dass das Ziel erfüllt ist. Eine Behauptung im Bericht \
             ohne passenden Beleg zählt nicht; im Zweifel NEIN.\n\n\
             Antworte in genau drei Zeilen:\nFORTSCHRITT: JA oder NEIN\nERREICHT: JA oder NEIN\nGRUND: ein Satz",
            v.ziel, notizen.join("\n"), bericht
        )
    } else {
        format!(
            "Review one round of a long-running task.\n\nGOAL:\n{}\n\nNOTES:\n{}\n\nREPORT OF THE ROUND:\n{}\n\n\
             EVIDENCE (the actual tool results of this round):\n{belegt}\n\n\
             THE AGENT REPORTS AS RESULT: {behauptet}\n\n\
             REACHED is YES only if the EVIDENCE shows that the goal is met. A claim in the report without \
             matching evidence does not count; if in doubt, NO.\n\n\
             Answer in exactly three lines:\nPROGRESS: YES or NO\nREACHED: YES or NO\nREASON: one sentence",
            v.ziel, notizen.join("\n"), bericht
        )
    };
    vec![Nachricht::nutzer(text)]
}

/// Liest die Antwort des Pruefdurchgangs.
///
/// ⚠️ **Im Zweifel nein.** Eine Antwort, die sich nicht lesen laesst,
/// zaehlt als „kein Fortschritt, nicht erreicht": Ein Vorhaben endet
/// lieber an der Stillstandsgrenze als an einer falsch gelesenen
/// Zustimmung.
pub fn pruefung_lesen(text: &str) -> Pruefung {
    let mut p = Pruefung { fortschritt: false, erreicht: false, grund: String::new() };
    for zeile in text.lines() {
        let z = zeile.trim();
        let (k, w) = match z.split_once(':') {
            Some((k, w)) => (k.trim().to_uppercase(), w.trim()),
            None => continue,
        };
        let ja = {
            let w = w.to_uppercase();
            w.starts_with("JA") || w.starts_with("YES")
        };
        match k.as_str() {
            "FORTSCHRITT" | "PROGRESS" => p.fortschritt = ja,
            "ERREICHT" | "REACHED" => p.erreicht = ja,
            "GRUND" | "REASON" => p.grund = w.to_string(),
            _ => {}
        }
    }
    p
}

// ── Nach der Runde ──────────────────────────────────────────────────

/// Wie die Agentenschleife der Runde endete, soweit es hier zaehlt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rundenende {
    /// Das Modell hat geantwortet.
    Normal,
    /// Schrittgrenze oder steckengeblieben: Die Runde zaehlt, aber als
    /// eine ohne eigenen Abschluss.
    Begrenzt,
    /// Notaus oder ein Fehler der Tuer: Das Vorhaben haelt an.
    Abbruch(String),
    /// Fenster oder Konsole wurden geschlossen: Die Runde zaehlt nicht,
    /// das Vorhaben bleibt, wie es war, und der Rundenstand bleibt liegen.
    Unterbrochen,
}

static SCHLIESSEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Fenster oder Konsole schliessen: die laufende Runde sofort beenden,
/// ohne das Vorhaben anzuhalten.
///
/// ⚑ **Dieselbe Bremse wie der Notaus, ein anderes Ergebnis.** Die
/// Erzeugung haelt am Notausschalter an, also wird er gezogen; an
/// `SCHLIESSEN` erkennt `runde` danach, dass es kein Notaus war.
pub fn schliessen_anfordern() {
    SCHLIESSEN.store(true, std::sync::atomic::Ordering::SeqCst);
    crate::notaus::ausloesen_still();
}

/// Wurde das Schliessen angefordert?
pub fn schliessen_angefordert() -> bool {
    SCHLIESSEN.load(std::sync::atomic::Ordering::SeqCst)
}

/// Vor jedem neuen Start des Loops im selben Prozess: Die Konsole kann
/// pausieren und spaeter weitermachen, ohne neu zu starten.
pub fn schliessen_zuruecksetzen() {
    SCHLIESSEN.store(false, std::sync::atomic::Ordering::SeqCst);
    crate::notaus::zuruecksetzen();
}

/// Rechnet eine abgeschlossene Runde in das Vorhaben ein und liefert die
/// Vorhaben, die als Kette neu beginnen.
///
/// ⚑ **Eine reine Rechnung**, ohne Platte und ohne Uhr, damit sich jede
/// Grenze pruefen laesst.
pub fn abschliessen(
    v: &mut Vorhaben,
    wahl: &Rundenwahl,
    pruefung: Option<&Pruefung>,
    ende: &Rundenende,
    grenzen: &Loopeinstellung,
    jetzt: u64,
) -> Vec<String> {
    if let Rundenende::Abbruch(grund) = ende {
        v.notizen = wahl.notizen.clone();
        v.zustand = Zustand::Angehalten { grund: grund.clone() };
        return Vec::new();
    }
    if *ende == Rundenende::Unterbrochen {
        return Vec::new();
    }
    v.runden += 1;
    v.notizen = wahl.notizen.clone();
    for z in &wahl.anschluss {
        if !v.anschluss.contains(z) {
            v.anschluss.push(z.clone());
        }
    }

    // ⚑ **Mit Pruefdurchgang entscheidet die Pruefung ueber „fertig",
    //   ohne ihn das Wort des Agenten.** Der Agent kann es vorschlagen;
    //   widerspricht die Pruefung, geht es weiter, und ihr Grund steht in
    //   den Notizen.
    //   📌 Gemessen am 2026-09-26 mit dem 0,6B-Modell: Die Datei stand
    //   nach Runde 1, die Pruefung sagte dreimal „erreicht", aber das
    //   Modell rief nie `finish_goal`, und zwei Runden liefen umsonst bis
    //   zur Hoechstzahl. Kleine Modelle vergessen das Abmelden.
    // ⛔️ **Eine Runde an der Schrittgrenze ist nie „fertig"** (Fund 487):
    //    Das Modell hat dann nichts abgeschlossen, und ein Urteil darueber
    //    waere eines ueber eine Runde, die mitten im Satz aufhoerte.
    //    Der Fortschritt zaehlt trotzdem: Eine Runde, die an der Grenze
    //    endet, kann eine Zeile repariert haben.
    let begrenzt = *ende == Rundenende::Begrenzt;
    if begrenzt && (wahl.fertig.is_some() || pruefung.is_some_and(|p| p.erreicht)) {
        v.notizen.insert(
            "runde".into(),
            "Die letzte Runde endete an der Schrittgrenze; fertig erst nach einer Runde mit Abschluss.".into(),
        );
    }
    let wahl_fertig = if begrenzt { None } else { wahl.fertig.clone() };
    let fertig = match pruefung {
        Some(_) if begrenzt => None,
        Some(p) if p.erreicht => Some(wahl_fertig.clone().unwrap_or_else(|| p.grund.clone())),
        Some(p) => {
            if wahl_fertig.is_some() {
                v.notizen.insert("pruefung".into(), format!("noch nicht erreicht: {}", p.grund));
            }
            None
        }
        None => wahl_fertig,
    };
    if let Some(ergebnis) = fertig {
        v.zustand = Zustand::Fertig;
        v.ergebnis = Some(ergebnis);
        return v.anschluss.clone();
    }

    let fortschritt = match pruefung {
        Some(p) => p.fortschritt,
        None => wahl.notizen_geaendert,
    };
    v.ohne_fortschritt = if fortschritt { 0 } else { v.ohne_fortschritt + 1 };

    if v.ohne_fortschritt >= grenzen.stillstand {
        v.zustand = Zustand::Angehalten {
            grund: format!("{} Runden ohne Fortschritt", v.ohne_fortschritt),
        };
    } else if v.runden >= grenzen.runden {
        v.zustand = Zustand::Angehalten { grund: format!("Höchstzahl von {} Runden erreicht", grenzen.runden) };
    } else if v.laufzeit >= u64::from(grenzen.stunden) * 3600 {
        v.zustand = Zustand::Angehalten { grund: format!("Höchstdauer von {} Stunden erreicht", grenzen.stunden) };
    } else if let Some(m) = wahl.wecken_minuten {
        v.naechste = jetzt + m * 60;
        v.zustand = Zustand::Schlaeft;
    } else {
        v.naechste = jetzt;
        v.zustand = Zustand::Bereit;
    }
    Vec::new()
}

/// Die Marke, mit der das System seine Zeilen im Tagebuch kennzeichnet.
pub const SYSTEMMARKE: &str = "[System]";

/// **Markiert Zeilen, in denen das Modell Aufrufe nur behauptet** (Fund 491).
///
/// ⚑ Eine Zeile im Bericht des Modells, die wie eine Zeile des Systems
/// aussieht („Ausgeführt:“, „Tatsächlich ausgeführt“, die Systemmarke),
/// bekommt „(behauptet, nicht ausgeführt)“ davor. Was wirklich lief, haelt
/// das System selbst fest.
pub fn behauptungen_markieren(text: &str, sprache: Sprache) -> String {
    let marke = if sprache == Sprache::De { "(behauptet, nicht ausgeführt) " } else { "(claimed, not executed) " };
    text.lines()
        .map(|z| {
            let t = z.trim_start();
            let verdaechtig = ["Ausgeführt:", "Ausgefuehrt:", "Executed:", "Tatsächlich ausgeführt", "Actually executed", SYSTEMMARKE]
                .iter()
                .any(|m| t.starts_with(m));
            if verdaechtig { format!("{marke}{t}") } else { z.to_string() }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Was der Abnahmebefehl nach einer Runde ergab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abnahme {
    pub befehl: String,
    pub bestanden: bool,
    /// Rueckgabewert, `None` bei Zeitueberschreitung oder Startfehler.
    pub code: Option<i32>,
    /// Das Ende der Ausgabe (stdout und stderr), gekuerzt.
    pub ausgabe: String,
}

/// Wie lange ein Abnahmebefehl hoechstens laufen darf.
pub const ABNAHME_SEKUNDEN: u64 = 300;
const ABNAHME_ZEICHEN: usize = 1200;

/// **Faehrt den Abnahmebefehl** mit `sh -c` im Arbeitsordner.
///
/// ⚑ **Ein Befehl des Menschen, nicht des Modells**: Er steht im Vorhaben,
/// seit es angelegt wurde, und das Modell kann ihn nicht aendern. Deshalb
/// laeuft er unabhaengig von der Werkzeugkiste.
pub fn abnahme_fahren(befehl: &str, ordner: Option<&Path>) -> Abnahme {
    let fehlschlag = |ausgabe: String| Abnahme { befehl: befehl.to_string(), bestanden: false, code: None, ausgabe };
    let Some(ordner) = ordner else {
        return fehlschlag("kein Arbeitsordner eingehaengt".into());
    };
    let mut kind = match std::process::Command::new("sh")
        .arg("-c")
        .arg(befehl)
        .current_dir(ordner)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(k) => k,
        Err(e) => return fehlschlag(format!("liess sich nicht starten: {e}")),
    };
    // Die Ausgabe in Faeden lesen, sonst blockiert ein voller Puffer das Kind.
    let lesen = |q: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut t = Vec::new();
            if let Some(mut q) = q {
                let _ = q.read_to_end(&mut t);
            }
            t
        })
    };
    let aus = lesen(kind.stdout.take().map(|x| Box::new(x) as Box<dyn std::io::Read + Send>));
    let fehler = lesen(kind.stderr.take().map(|x| Box::new(x) as Box<dyn std::io::Read + Send>));
    let bis = std::time::Instant::now() + std::time::Duration::from_secs(ABNAHME_SEKUNDEN);
    let status = loop {
        match kind.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if std::time::Instant::now() < bis => std::thread::sleep(std::time::Duration::from_millis(50)),
            _ => {
                let _ = kind.kill();
                let _ = kind.wait();
                break None;
            }
        }
    };
    let mut text = String::from_utf8_lossy(&aus.join().unwrap_or_default()).into_owned();
    text.push_str(&String::from_utf8_lossy(&fehler.join().unwrap_or_default()));
    let n = text.chars().count();
    let text: String = text.chars().skip(n.saturating_sub(ABNAHME_ZEICHEN)).collect();
    match status {
        Some(st) => Abnahme { befehl: befehl.to_string(), bestanden: st.success(), code: st.code(), ausgabe: text.trim().to_string() },
        None => fehlschlag(format!("nach {ABNAHME_SEKUNDEN} s abgebrochen. {}", text.trim())),
    }
}

/// **Rechnet eine Runde samt Abnahme ein.**
///
/// ⚑ **Mit Abnahme entscheidet der Befehl ueber „fertig“, sonst niemand.**
/// Endet er mit 0, ist das Vorhaben fertig, auch an der Schrittgrenze und
/// auch, wenn das Modell sich nicht abgemeldet hat. Endet er anders, ist
/// es nicht fertig, und seine Ausgabe steht als Notiz `abnahme` da: Die
/// naechste Runde sieht den echten Fehler statt einer Vermutung.
///
/// 📌 **Warum (Loop-Szenario, 2026-09-26):** Der Pruefdurchgang des 8B
/// bestaetigte zweimal falsche Arbeit als erreicht, auch mit Belegen. **Ein
/// Modell, das sich selbst abnimmt, nimmt sich nicht ab.**
pub fn abschliessen_mit_abnahme(
    v: &mut Vorhaben,
    wahl: &Rundenwahl,
    pruefung: Option<&Pruefung>,
    ende: &Rundenende,
    grenzen: &Loopeinstellung,
    jetzt: u64,
    abnahme: Option<&Abnahme>,
) -> Vec<String> {
    let Some(a) = abnahme.filter(|_| matches!(ende, Rundenende::Normal | Rundenende::Begrenzt)) else {
        return abschliessen(v, wahl, pruefung, ende, grenzen, jetzt);
    };
    let mut w = wahl.clone();
    if a.bestanden {
        w.fertig = Some(w.fertig.clone().unwrap_or_else(|| format!("Abnahme bestanden: {}", a.befehl)));
        let p = Pruefung { fortschritt: true, erreicht: true, grund: format!("`{}` endete mit 0", a.befehl) };
        let aus = abschliessen(v, &w, Some(&p), &Rundenende::Normal, grenzen, jetzt);
        v.notizen.remove("abnahme");
        v.notizen.remove("runde");
        v.notizen.remove("pruefung");
        return aus;
    }
    w.fertig = None;
    let p = pruefung.map(|p| Pruefung { erreicht: false, ..p.clone() });
    let aus = abschliessen(v, &w, p.as_ref(), ende, grenzen, jetzt);
    let code = a.code.map(|c| c.to_string()).unwrap_or_else(|| "ohne Rueckgabewert".into());
    let ausgabe: String = a.ausgabe.chars().rev().take(NOTIZ_ZEICHEN - 80).collect::<Vec<_>>().into_iter().rev().collect();
    v.notizen.insert("abnahme".into(), format!("`{}` endete mit {code}: {ausgabe}", a.befehl));
    aus
}

// ── Anhalten und Fortsetzen ─────────────────────────────────────────

/// Beim Schliessen: aus jedem Weckzeitpunkt wird die Restwartezeit.
pub fn anhalten(v: &mut Vorhaben, jetzt: u64) {
    if v.aktiv() && v.rest.is_none() {
        v.rest = Some(v.naechste.saturating_sub(jetzt));
    }
}

/// Beim Oeffnen: aus der Restwartezeit wird wieder ein Zeitpunkt.
pub fn fortsetzen(v: &mut Vorhaben, jetzt: u64) {
    if let Some(r) = v.rest.take() {
        v.naechste = jetzt + r;
    }
}

/// Unix-Sekunden jetzt.
pub fn jetzt() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Eine Runde fahren ───────────────────────────────────────────────

/// Was eine Runde zurueckmeldet.
#[derive(Debug, Clone)]
pub struct Runde {
    /// Die Runde wurde durch Schliessen unterbrochen und zaehlt nicht.
    pub unterbrochen: bool,
    pub bericht: String,
    pub pruefung: Option<Pruefung>,
    pub neue: Vec<Vorhaben>,
    /// Was der Abnahmebefehl ergab, falls das Vorhaben einen hat.
    pub abnahme: Option<Abnahme>,
}

/// Die Ruestung einer Runde baut der Aufrufer (Fenster, Konsole, `myl`):
/// Er kennt Einstellungen, Einhaengung und Nachfrage. Hier kommen nur die
/// vier Loop-Werkzeuge dazu.
///
/// ⚑ Das zweite Argument ist die **Saat der Web-Recherche**: das Ziel des
/// Tasks, also was der Mensch geschrieben hat (`Agenteneinstellung::netzsaat`).
pub type Ruester<'a> = dyn Fn(Zusatzwerkzeuge, &str) -> Result<crate::ruestung::Ruestung, String> + 'a;

/// Werkzeuge, die eine Runde zur Ruestung hinzufuegt.
pub type Zusatzwerkzeuge = Vec<(Werkzeug, Box<dyn Werkzeugausfuehrung>)>;

/// Faehrt eine Runde von `v` und schreibt alles zurueck in die Ablage.
#[allow(clippy::too_many_arguments)]
pub fn runde(
    ablage: &Ablage,
    v: &mut Vorhaben,
    modell: &dyn Modellweg,
    ruesten: &Ruester<'_>,
    grenzen: &Loopeinstellung,
    sprache: Sprache,
    max_tokens: u32,
    melder: Option<&dyn Fn(myl_local_agent::schleife::Meldung<'_>)>,
) -> Result<Runde, String> {
    let anfang = std::time::Instant::now();
    let stand = ablage.rundenstand(&v.kennung);
    let rueckblick = ablage.tagebuch_letzte(&v.kennung, TAGEBUCH_RUECKBLICK);
    let text = auftrag(v, &rueckblick, stand.as_ref(), grenzen, sprache);

    let start_notizen = stand.as_ref().map(|r| r.notizen.clone()).unwrap_or_else(|| v.notizen.clone());
    let wahl: Geteilt = Arc::new(Mutex::new(Rundenwahl { notizen: start_notizen, ..Default::default() }));
    let mut ruestung = ruesten(werkzeuge(&wahl), &v.ziel)?;
    // ⛔️ **Was vor einer Unterbrechung lief, laeuft nicht noch einmal.**
    //    Der Hinweis im Auftrag reicht nicht: Im Probelauf am 2026-09-26
    //    wiederholte das 0,6B-Modell drei erledigte Aufrufe trotzdem. Bei
    //    einer Datei ist das harmlos, bei einem Befehl oder einer
    //    Web-Anfrage waere es eine doppelte Wirkung. Die Sperre liefert
    //    fuer denselben Aufruf das gespeicherte Ergebnis.
    if let Some(r) = stand.as_ref().filter(|r| !r.erledigt.is_empty()) {
        let erledigt: std::collections::HashMap<(String, String), String> = r
            .erledigt
            .iter()
            .map(|e| ((e.werkzeug.clone(), e.argumente.clone()), e.ergebnis.clone()))
            .collect();
        let erledigt = Arc::new(erledigt);
        let de = sprache == Sprache::De;
        ruestung.kasten.umhuellen(|inner| Box::new(Doppelsperre { inner, erledigt: Arc::clone(&erledigt), de }));
    }
    // ⚑ Der Pendelwaechter um jedes schreibende Dateiwerkzeug (Punkt 4.8).
    if let Some(wurzel) = ruestung.einhaengung.as_ref().map(|e| e.wurzel().to_path_buf()) {
        let staende = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let de = sprache == Sprache::De;
        ruestung.kasten.umhuellen(|inner| {
            if SCHREIBEND.contains(&inner.name()) {
                Box::new(Pendelwaechter { inner, wurzel: wurzel.clone(), staende: Arc::clone(&staende), de })
            } else {
                inner
            }
        });
    }
    // ⚑ Die Wiederholungsbremse gilt in jeder Runde (Fund 488).
    {
        let gesehen = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let welt = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let de = sprache == Sprache::De;
        ruestung.kasten.umhuellen(|inner| {
            Box::new(Wiederholungsbremse { inner, gesehen: Arc::clone(&gesehen), welt: Arc::clone(&welt), de })
        });
    }

    // ⚑ Nach jedem Werkzeugergebnis den Rundenstand schreiben.
    let festhalten = Mutex::new((stand.clone().unwrap_or_default(), None::<(String, String)>));
    let kennung = v.kennung.clone();
    let mitschreiben = |m: myl_local_agent::schleife::Meldung<'_>| {
        use myl_local_agent::schleife::Meldung;
        let mut f = festhalten.lock().expect("Rundenstand");
        match m {
            Meldung::Aufruf { name, argumente } => f.1 = Some((name.to_string(), argumente.to_string())),
            Meldung::Ergebnis { name, text } => {
                let argumente = f.1.take().filter(|(n, _)| n == name).map(|(_, a)| a).unwrap_or_default();
                f.0.erledigt.push(Erledigt { werkzeug: name.to_string(), argumente, ergebnis: text.to_string() });
                f.0.notizen = wahl.lock().expect("Rundenwahl").notizen.clone();
                let _ = ablage.rundenstand_schreiben(&kennung, &f.0);
            }
            _ => {}
        }
        if let Some(weiter) = melder {
            weiter(m);
        }
    };
    let ausgang =
        crate::lauf::fahren_beobachtet(modell, &ruestung, grenzen.schritte as usize, true, max_tokens, &text, Some(&mitschreiben));

    let ende = if schliessen_angefordert() {
        Rundenende::Unterbrochen
    } else if crate::notaus::ausgeloest() {
        Rundenende::Abbruch("Notaus".into())
    } else {
        use myl_local_agent::schleife::Ende;
        match &ausgang.ende {
            Ende::Fertig => Rundenende::Normal,
            Ende::Steckengeblieben { .. } | Ende::Grenze(_) => Rundenende::Begrenzt,
            Ende::Tuer(t) => Rundenende::Abbruch(format!("{t}")),
        }
    };
    // ⚑ **Ohne Schlusssatz berichten die Werkzeuge.** Ein Modell, das mit
    //   einem Aufruf endet, hinterliesse sonst „(keine Antwort)" im
    //   Tagebuch, und die naechste Runde wuesste nicht, was geschah.
    let bericht = match ausgang.antwort.clone().filter(|a| !a.trim().is_empty()) {
        Some(a) => behauptungen_markieren(&a, sprache),
        None => {
            let erledigt = festhalten.lock().expect("Rundenstand").0.erledigt.clone();
            let liste: Vec<String> = erledigt
                .iter()
                .map(|e| format!("{}({})", e.werkzeug, e.argumente.chars().take(80).collect::<String>()))
                .collect();
            match (liste.is_empty(), sprache) {
                (true, Sprache::De) => "(keine Antwort, kein Werkzeug)".into(),
                (true, Sprache::En) => "(no answer, no tool)".into(),
                (false, Sprache::De) => format!("Werkzeuge dieser Runde: {}", liste.join(", ")),
                (false, Sprache::En) => format!("Tools used in this round: {}", liste.join(", ")),
            }
        }
    };
    let w = wahl.lock().expect("Rundenwahl").clone();

    // ⚠️ Bei einem Abbruch keine Pruefung und kein Tagebuch: Die Runde ist
    //    nicht zu Ende, sie wird fortgesetzt, und der Rundenstand bleibt.
    let unfertig = matches!(ende, Rundenende::Abbruch(_) | Rundenende::Unterbrochen);
    let pruefung = if grenzen.pruefen && !unfertig {
        modell
            .chat("lokal", &pruefauftrag(v, &w, &bericht, &festhalten.lock().expect("Rundenstand").0.erledigt, sprache), Some(120))
            .ok()
            .map(|a| pruefung_lesen(&a.text))
    } else {
        None
    };

    // ⚑ Die Abnahme laeuft nach jeder abgeschlossenen Runde (Punkt 4.7).
    let abnahme = if unfertig {
        None
    } else {
        v.abnahme.as_deref().map(|b| abnahme_fahren(b, ruestung.einhaengung.as_ref().map(|e| e.wurzel())))
    };
    v.laufzeit += anfang.elapsed().as_secs();
    let anschluss = abschliessen_mit_abnahme(v, &w, pruefung.as_ref(), &ende, grenzen, jetzt(), abnahme.as_ref());
    if !unfertig {
        // ⚑ **Das Tagebuch haelt Tatsachen fest, nicht nur Behauptungen.**
        //   Neben dem Bericht des Modells steht, welche Werkzeuge die Runde
        //   wirklich benutzt hat, auch die vor einer Unterbrechung.
        //   📌 Gemessen am 2026-09-26: Nach einer fortgesetzten Runde
        //   stand im Tagebuch nur der Text des Modells, der Rundenstand war
        //   geloescht, und die naechste Runde schrieb dieselben drei
        //   Dateien noch einmal.
        let erledigt = festhalten.lock().expect("Rundenstand").0.erledigt.clone();
        let mut eintrag = bericht.clone();
        // 📌 **Fund 491 (2026-09-26): Das Modell ahmte diese Zeile nach.** Sie
        //    hiess „Ausgeführt: …“, und das 30B schrieb in zwei Runden selbst
        //    „Ausgeführt: write_file(…)“ in seine Antwort, ohne ein Werkzeug
        //    zu rufen. Nichts war geschrieben, und im Tagebuch stand es wie
        //    eine Tatsache. Jetzt traegt die Zeile des Systems eine Marke,
        //    und eine Runde ohne Werkzeug sagt das ausdruecklich.
        if !bericht.starts_with("Werkzeuge dieser Runde") && !bericht.starts_with("Tools used") {
            let liste: Vec<String> = erledigt
                .iter()
                .map(|e| format!("{}({})", e.werkzeug, e.argumente.chars().take(80).collect::<String>()))
                .collect();
            eintrag.push_str(&match (liste.is_empty(), sprache) {
                (false, Sprache::De) => format!("\n\n{SYSTEMMARKE} Tatsächlich ausgeführt: {}", liste.join(", ")),
                (false, Sprache::En) => format!("\n\n{SYSTEMMARKE} Actually executed: {}", liste.join(", ")),
                (true, Sprache::De) => format!("\n\n{SYSTEMMARKE} In dieser Runde lief kein Werkzeug."),
                (true, Sprache::En) => format!("\n\n{SYSTEMMARKE} No tool ran in this round."),
            });
        }
        if let Some(a) = &abnahme {
            let code = a.code.map(|c| c.to_string()).unwrap_or_else(|| "-".into());
            eintrag.push_str(&format!("\n\nAbnahme `{}`: {} (Rueckgabe {code})", a.befehl, if a.bestanden { "bestanden" } else { "nicht bestanden" }));
        }
        ablage.tagebuch_anhaengen(&v.kennung, v.runden, &eintrag)?;
        ablage.rundenstand_loeschen(&v.kennung);
    }
    ablage.speichern(v)?;

    let mut neue = Vec::new();
    for z in anschluss {
        let mut n = ablage.anlegen(&z, jetzt())?;
        n.vorgaenger = Some(v.kennung.clone());
        ablage.speichern(&n)?;
        ablage.einreihen_nach(&n.kennung, &v.kennung)?;
        neue.push(n);
    }
    Ok(Runde { unterbrochen: ende == Rundenende::Unterbrochen, bericht, pruefung, neue, abnahme })
}

// ── Der Laeufer ─────────────────────────────────────────────────────

/// Wer die Vorhaben gerade faehrt: Fenster, Konsole oder `myl`.
///
/// ⛔️ **Immer nur einer.** Fenster und Konsole koennen gleichzeitig offen
/// sein; fuehren beide dieselbe Runde, liefe jede Handlung doppelt. Die
/// Sperre ist eine Datei mit der Prozessnummer; stirbt der Prozess, gilt
/// sie beim naechsten Oeffnen als verwaist.
#[derive(Debug)]
pub struct Laeufer {
    pub ablage: Ablage,
    sperre: PathBuf,
    wer: String,
    /// Wann der Herzschlag zuletzt geschrieben wurde.
    puls: std::cell::Cell<u64>,
    /// Offene Wartezeit, die noch nicht in die Vorhaben gebucht ist.
    ungebucht: std::cell::Cell<u64>,
}

/// Alle wie viele Sekunden der Herzschlag geschrieben wird.
///
/// ⚑ **Der Herzschlag ist die Sicherung gegen ein hartes Ende.** Beim
/// geordneten Schliessen rechnet `Drop` die Restwartezeiten aus. Stirbt
/// der Prozess (Absturz, `kill`, Stromausfall), laeuft kein `Drop`; dann
/// nimmt der naechste Laeufer den letzten Herzschlag als Zeitpunkt des
/// Anhaltens. Verloren gehen hoechstens so viele Sekunden.
pub const PULS_SEKUNDEN: u64 = 10;

impl Laeufer {
    /// Uebernimmt die Vorhaben: setzt Restwartezeiten fort. `Err` nennt,
    /// wer sie schon faehrt.
    pub fn oeffnen(ablage: Ablage, wer: &str) -> Result<Self, String> {
        std::fs::create_dir_all(&ablage.ordner).map_err(|e| e.to_string())?;
        let sperre = ablage.ordner.join(".laeufer");
        // Form der Sperre: `<pid> <letzter Herzschlag> <wer>`.
        let mut verwaist_seit: Option<u64> = None;
        if let Ok(t) = std::fs::read_to_string(&sperre) {
            let mut teile = t.split_whitespace();
            let pid: u32 = teile.next().and_then(|p| p.parse().ok()).unwrap_or(0);
            let puls: u64 = teile.next().and_then(|p| p.parse().ok()).unwrap_or(0);
            let name = teile.collect::<Vec<_>>().join(" ");
            if pid != std::process::id() && lebt(pid) {
                return Err(format!("der Loop läuft schon ({name}, Prozess {pid})"));
            }
            verwaist_seit = (puls > 0).then_some(puls);
        }
        let t = jetzt();
        let l = Self {
            ablage,
            sperre,
            wer: wer.to_string(),
            puls: std::cell::Cell::new(0),
            ungebucht: std::cell::Cell::new(0),
        };
        l.herzschlag(t)?;
        for mut v in l.ablage.alle() {
            // ⚑ Hart beendet: kein Rest gesichert. Dann gilt der letzte
            //   Herzschlag als Zeitpunkt des Anhaltens.
            if let (Some(seit), true, None) = (verwaist_seit, v.aktiv(), v.rest) {
                anhalten(&mut v, seit);
            }
            if v.rest.is_some() {
                fortsetzen(&mut v, t);
                l.ablage.speichern(&v)?;
            }
        }
        Ok(l)
    }

    /// Das vorderste aktive Vorhaben, falls es faellig ist.
    ///
    /// ⚑ **Nur das vorderste**, auch wenn es schlaeft und ein anderes
    /// dahinter bereit waere: Die Schlange ist eine Reihenfolge, keine
    /// Liste gleichzeitiger Arbeiten. Wer ein anderes zuerst will, stellt
    /// es nach vorn.
    pub fn faellig(&self, jetzt: u64) -> Option<Vorhaben> {
        self.ablage.vorn().filter(|v| v.faellig(jetzt))
    }

    /// Wann das vorderste Vorhaben faellig wird.
    pub fn naechster_termin(&self) -> Option<u64> {
        self.ablage.vorn().filter(|v| v.rest.is_none()).map(|v| v.naechste)
    }

    /// Gibt es ueberhaupt noch etwas zu tun?
    pub fn aktive(&self) -> usize {
        self.ablage.alle().iter().filter(|v| v.aktiv()).count()
    }

    /// Schreibt den Herzschlag in die Sperre.
    pub fn herzschlag(&self, t: u64) -> Result<(), String> {
        std::fs::write(&self.sperre, format!("{} {t} {}", std::process::id(), self.wer))
            .map_err(|e| e.to_string())?;
        self.puls.set(t);
        Ok(())
    }

    /// Verbucht offene Wartezeit als Laufzeit (die Hoechstdauer zaehlt
    /// nur offene Zeit, und dazu gehoert das Warten) und schlaegt, wenn
    /// es Zeit ist, das Herz.
    ///
    /// ⚑ **Gebucht wird im Herzschlagtakt**, nicht jede Sekunde: Sonst
    ///   schriebe ein wartender Loop jedes Vorhaben einmal je Sekunde.
    pub fn verbuchen(&self, sekunden: u64) {
        self.ungebucht.set(self.ungebucht.get() + sekunden);
        let t = jetzt();
        if t.saturating_sub(self.puls.get()) >= PULS_SEKUNDEN {
            self.buchen();
            let _ = self.herzschlag(t);
        }
    }

    fn buchen(&self) {
        let s = self.ungebucht.replace(0);
        if s == 0 {
            return;
        }
        for mut v in self.ablage.alle().into_iter().filter(|v| v.aktiv()) {
            v.laufzeit += s;
            let _ = self.ablage.speichern(&v);
        }
    }

    /// Beim Schliessen: Wartezeiten werden zu Resten, die Sperre geht.
    pub fn schliessen(self) {
        drop(self);
    }
}

impl Drop for Laeufer {
    fn drop(&mut self) {
        self.buchen();
        let t = jetzt();
        for mut v in self.ablage.alle() {
            if v.aktiv() && v.rest.is_none() {
                anhalten(&mut v, t);
                let _ = self.ablage.speichern(&v);
            }
        }
        let _ = std::fs::remove_file(&self.sperre);
    }
}

/// Lebt der Prozess noch?
#[cfg(unix)]
fn lebt(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: `kill` mit Signal 0 prueft nur, ob der Prozess existiert.
    let r = unsafe { libc::kill(pid as libc::pid_t, 0) };
    // ⚠️ **`EPERM` heisst: er lebt, gehoert aber jemand anderem.** Nur
    //    `ESRCH` heisst: es gibt ihn nicht. Wer `EPERM` als „tot" liest,
    //    uebergeht die Sperre eines Fensters, das unter einem anderen
    //    Nutzer laeuft.
    r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// ⚠️ Ohne `kill` gilt jede fremde Sperre als lebendig; wer sicher ist,
/// dass nichts laeuft, loescht `vorhaben/.laeufer`.
#[cfg(not(unix))]
fn lebt(pid: u32) -> bool {
    pid != 0
}

/// Der Hinweis, solange die Loop-Einstellungen auf ihren Vorgaben stehen
/// (Wunsch des Projektinhabers). ⚑ Ein Text fuer `myl`, Konsole und
/// Fenster.
pub fn hinweis_vorgaben(e: &crate::einstellungen::Einstellungen) -> Option<String> {
    let l = &e.schleife;
    if !l.ist_vorgabe() {
        return None;
    }
    Some(match e.oberflaeche.sprache {
        Sprache::De => format!(
            "Loop mit Standardeinstellungen: höchstens {} Runden und {} Stunden, {} Schritte je Runde, \
             Pause nach {} Runden ohne Fortschritt, Prüfdurchgang {}. Modus: {}. \
             Ändern in den Einstellungen unter „Loop“ (oder `myl setzen loop.runden …`).",
            l.runden, l.stunden, l.schritte, l.stillstand, if l.pruefen { "an" } else { "aus" }, e.agent.modus.kennung()
        ),
        Sprache::En => format!(
            "Loop with default settings: at most {} rounds and {} hours, {} steps per round, \
             pause after {} rounds without progress, review pass {}. Mode: {}. \
             Change in the settings under “Loop” (or `myl setzen loop.runden …`).",
            l.runden, l.stunden, l.schritte, l.stillstand, if l.pruefen { "on" } else { "off" }, e.agent.modus.kennung()
        ),
    })
}

// ── Der Antrieb ─────────────────────────────────────────────────────

/// Was der Antrieb der Oberflaeche meldet.
#[derive(Debug, Clone)]
pub enum Ereignis {
    /// Eine Runde beginnt.
    Beginnt { kennung: String, ziel: String, runde: u32 },
    /// Eine Runde ist zu Ende.
    Geendet { vorhaben: Box<Vorhaben>, bericht: String, pruefung: Option<Pruefung> },
    /// Ein neues Vorhaben aus einer Kette.
    Angestossen { kennung: String, ziel: String },
    /// Nichts faellig; das naechste um diese Zeit.
    Wartet { bis: u64 },
    /// Eine Runde liess sich nicht fahren.
    Fehler { kennung: String, grund: String },
    /// Das Modell war nicht zu haben; der Loop endet, das Vorhaben bleibt,
    /// wie es war.
    OhneModell { grund: String },
}

/// **Leiht das Modell fuer genau eine Runde.**
///
/// ⚑ **Eine Leihe je Runde und nicht ein Modell fuer den ganzen Loop.**
/// Ein Loop wartet Stunden zwischen zwei Runden; haelt er das Modell die
/// ganze Zeit, kann im Fenster so lange niemand chatten. Mit der Leihe
/// gehoert es dem Loop nur, solange eine Runde laeuft. Das Fenster
/// sperrt dafuer seinen Halter und laedt nach, falls das Modell in der
/// Zwischenzeit entladen wurde; `myl` und die Konsole reichen einfach
/// ihr Modell durch.
pub type Modellleihe<'a> = dyn Fn(&mut dyn FnMut(&dyn Modellweg)) -> Result<(), String> + 'a;

/// Faehrt faellige Runden, bis nichts mehr aktiv ist, `beenden` wahr wird
/// oder der Notaus greift.
///
/// ⚑ **Zwischen den Runden wird in Sekundenschritten gewartet**, damit
/// Schliessen und Notaus sofort greifen und nicht erst nach der
/// Wartezeit. Die gewartete Zeit wird als Laufzeit verbucht.
#[allow(clippy::too_many_arguments)]
pub fn fahren(
    laeufer: &Laeufer,
    leihen: &Modellleihe<'_>,
    ruesten: &Ruester<'_>,
    grenzen: &Loopeinstellung,
    sprache: Sprache,
    max_tokens: u32,
    melden: &dyn Fn(Ereignis),
    beenden: &dyn Fn() -> bool,
    melder: Option<&dyn Fn(myl_local_agent::schleife::Meldung<'_>)>,
) {
    let mut zuletzt_gemeldet = 0u64;
    loop {
        if beenden() || crate::notaus::ausgeloest() || schliessen_angefordert() {
            return;
        }
        let t = jetzt();
        if let Some(mut v) = laeufer.faellig(t) {
            let mut ergebnis = None;
            if let Err(grund) = leihen(&mut |modell| {
                // ⚑ **Erst gemeldet, wenn das Modell geliehen ist.** Laeuft
                //   im Fenster gerade ein Chat, wartet die Leihe auf ihn;
                //   ein frueheres „beginnt" zeigte eine Runde an, die noch
                //   gar nicht rechnet.
                melden(Ereignis::Beginnt { kennung: v.kennung.clone(), ziel: v.ziel.clone(), runde: v.runden + 1 });
                ergebnis = Some(runde(&laeufer.ablage, &mut v, modell, ruesten, grenzen, sprache, max_tokens, melder));
            }) {
                melden(Ereignis::OhneModell { grund });
                return;
            }
            let Some(ergebnis) = ergebnis else {
                melden(Ereignis::OhneModell { grund: "die Leihe rief die Runde nicht".into() });
                return;
            };
            match ergebnis {
                Ok(r) if r.unterbrochen => return,
                Ok(r) => {
                    for n in &r.neue {
                        melden(Ereignis::Angestossen { kennung: n.kennung.clone(), ziel: n.ziel.clone() });
                    }
                    melden(Ereignis::Geendet { vorhaben: Box::new(v), bericht: r.bericht, pruefung: r.pruefung });
                }
                Err(grund) => {
                    // ⚠️ Anhalten statt Wiederholen: Eine Runde, die an der
                    //    Ruestung scheitert, scheitert beim naechsten Mal
                    //    genauso, und eine Schleife daraus verbraucht nur Zeit.
                    v.zustand = Zustand::Angehalten { grund: grund.clone() };
                    let _ = laeufer.ablage.speichern(&v);
                    melden(Ereignis::Fehler { kennung: v.kennung.clone(), grund });
                }
            }
            continue;
        }
        let Some(bis) = laeufer.naechster_termin() else { return };
        if bis != zuletzt_gemeldet {
            melden(Ereignis::Wartet { bis });
            zuletzt_gemeldet = bis;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        laeufer.verbuchen(1);
    }
}

#[cfg(test)]
mod proben {
    use super::*;

    fn grenzen() -> Loopeinstellung {
        Loopeinstellung::default()
    }

    fn wahl() -> Rundenwahl {
        Rundenwahl::default()
    }

    #[test]
    fn runde_ohne_entscheidung_ist_sofort_wieder_faellig() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 100);
        let mut w = wahl();
        w.notizen_geaendert = true;
        abschliessen(&mut v, &w, None, &Rundenende::Normal, &grenzen(), 200);
        assert_eq!(v.runden, 1);
        assert_eq!(v.zustand, Zustand::Bereit);
        assert!(v.faellig(200));
    }

    #[test]
    fn selbstwecken_legt_die_naechste_runde_spaeter() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 100);
        let mut w = wahl();
        w.notizen_geaendert = true;
        w.wecken_minuten = Some(90);
        abschliessen(&mut v, &w, None, &Rundenende::Normal, &grenzen(), 1000);
        assert_eq!(v.zustand, Zustand::Schlaeft);
        assert_eq!(v.naechste, 1000 + 90 * 60);
        assert!(!v.faellig(1000 + 89 * 60));
        assert!(v.faellig(1000 + 90 * 60));
    }

    /// ⚑ **Schliessen und Oeffnen: genau der Rest der Wartezeit.**
    #[test]
    fn anhalten_und_fortsetzen_behalten_den_rest() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        v.zustand = Zustand::Schlaeft;
        v.naechste = 10_000;
        anhalten(&mut v, 4_600); // noch 5 400 s = 90 Minuten
        assert_eq!(v.rest, Some(5_400));
        assert!(!v.faellig(50_000), "angehalten ist nie faellig");
        fortsetzen(&mut v, 100_000); // Stunden spaeter geoeffnet
        assert_eq!(v.naechste, 105_400);
        assert!(v.rest.is_none());
    }

    #[test]
    fn fertig_mit_kette() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        let mut w = wahl();
        w.fertig = Some("erledigt".into());
        w.anschluss = vec!["Folgeziel".into()];
        let neu = abschliessen(&mut v, &w, None, &Rundenende::Normal, &grenzen(), 10);
        assert_eq!(v.zustand, Zustand::Fertig);
        assert_eq!(v.ergebnis.as_deref(), Some("erledigt"));
        assert_eq!(neu, ["Folgeziel"]);
    }

    /// ⚑ Kombination: Selbstwecken jetzt, Kette nach dem Fertigwerden.
    #[test]
    fn kette_wartet_auf_das_fertigwerden() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        let mut w = wahl();
        w.notizen_geaendert = true;
        w.wecken_minuten = Some(5);
        w.anschluss = vec!["danach".into()];
        let neu = abschliessen(&mut v, &w, None, &Rundenende::Normal, &grenzen(), 0);
        assert!(neu.is_empty(), "noch nicht fertig, also noch keine Kette");
        assert_eq!(v.anschluss, ["danach"]);
        let mut w2 = wahl();
        w2.notizen = v.notizen.clone();
        w2.fertig = Some("ok".into());
        let neu = abschliessen(&mut v, &w2, None, &Rundenende::Normal, &grenzen(), 400);
        assert_eq!(neu, ["danach"]);
    }

    /// ⛔️ Die Pruefung widerspricht: nicht fertig, und der Grund steht in den Notizen.
    #[test]
    fn pruefung_verhindert_fertig_ohne_ergebnis() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        let mut w = wahl();
        w.fertig = Some("alles erledigt".into());
        let p = Pruefung { fortschritt: true, erreicht: false, grund: "die Datei fehlt".into() };
        abschliessen(&mut v, &w, Some(&p), &Rundenende::Normal, &grenzen(), 0);
        assert_ne!(v.zustand, Zustand::Fertig);
        assert!(v.notizen["pruefung"].contains("die Datei fehlt"));
    }

    /// ⚑ Die Pruefung sagt „erreicht", der Agent hat sich nicht abgemeldet:
    /// fertig, mit dem Grund der Pruefung als Ergebnis.
    #[test]
    fn pruefung_erkennt_fertig_ohne_abmeldung() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        let p = Pruefung { fortschritt: true, erreicht: true, grund: "die Datei steht".into() };
        abschliessen(&mut v, &wahl(), Some(&p), &Rundenende::Normal, &grenzen(), 0);
        assert_eq!(v.zustand, Zustand::Fertig);
        assert_eq!(v.ergebnis.as_deref(), Some("die Datei steht"));
    }

    #[test]
    fn stillstand_haelt_an() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        let g = grenzen();
        for _ in 0..g.stillstand {
            abschliessen(&mut v, &wahl(), None, &Rundenende::Normal, &g, 0);
        }
        assert!(matches!(&v.zustand, Zustand::Angehalten { grund } if grund.contains("ohne Fortschritt")));
    }

    #[test]
    fn hoechstzahl_an_runden_haelt_an() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        let g = Loopeinstellung { runden: 2, ..grenzen() };
        let mut w = wahl();
        w.notizen_geaendert = true;
        abschliessen(&mut v, &w, None, &Rundenende::Normal, &g, 0);
        assert!(v.aktiv());
        abschliessen(&mut v, &w, None, &Rundenende::Normal, &g, 0);
        assert!(matches!(&v.zustand, Zustand::Angehalten { grund } if grund.contains("Runden")));
    }

    #[test]
    fn hoechstdauer_zaehlt_nur_laufzeit() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        let g = Loopeinstellung { stunden: 1, ..grenzen() };
        let mut w = wahl();
        w.notizen_geaendert = true;
        // Tage vergangen, aber kaum Laufzeit: laeuft weiter.
        v.laufzeit = 100;
        abschliessen(&mut v, &w, None, &Rundenende::Normal, &g, 10 * 86_400);
        assert!(v.aktiv());
        v.laufzeit = 3_600;
        abschliessen(&mut v, &w, None, &Rundenende::Normal, &g, 10 * 86_400);
        assert!(matches!(&v.zustand, Zustand::Angehalten { grund } if grund.contains("Stunden")));
    }

    #[test]
    fn schliessen_laesst_das_vorhaben_wie_es_war() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        let vorher = v.clone();
        let mut w = wahl();
        w.notizen.insert("halb".into(), "fertig".into());
        abschliessen(&mut v, &w, None, &Rundenende::Unterbrochen, &grenzen(), 0);
        assert_eq!(v, vorher, "keine Runde gezaehlt, nichts angehalten");
    }

    #[test]
    fn notaus_haelt_an_und_zaehlt_keine_runde() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        abschliessen(&mut v, &wahl(), None, &Rundenende::Abbruch("Notaus".into()), &grenzen(), 0);
        assert_eq!(v.runden, 0);
        assert!(matches!(&v.zustand, Zustand::Angehalten { grund } if grund == "Notaus"));
    }

    #[test]
    fn pruefung_lesen_im_zweifel_nein() {
        let p = pruefung_lesen("FORTSCHRITT: JA\nERREICHT: nein\nGRUND: fehlt noch");
        assert!(p.fortschritt && !p.erreicht);
        assert_eq!(p.grund, "fehlt noch");
        let e = pruefung_lesen("PROGRESS: yes\nREACHED: YES\nREASON: done");
        assert!(e.fortschritt && e.erreicht);
        let unklar = pruefung_lesen("Ich denke schon.");
        assert!(!unklar.fortschritt && !unklar.erreicht);
    }

    #[test]
    fn wecken_bleibt_in_den_grenzen() {
        let g = Arc::new(Mutex::new(Rundenwahl::default()));
        let w = Wecken(g.clone());
        w.ausfuehren(&serde_json::json!({"minutes": 0})).unwrap();
        assert_eq!(g.lock().unwrap().wecken_minuten, Some(WECKEN_AB_MINUTEN));
        w.ausfuehren(&serde_json::json!({"minutes": 99_999_999})).unwrap();
        assert_eq!(g.lock().unwrap().wecken_minuten, Some(WECKEN_BIS_MINUTEN));
    }

    #[test]
    fn notizblock_ist_begrenzt() {
        let g = Arc::new(Mutex::new(Rundenwahl::default()));
        let n = Notiz(g.clone());
        for i in 0..NOTIZEN_HOECHSTENS {
            n.ausfuehren(&serde_json::json!({"key": format!("k{i}"), "value": "x"})).unwrap();
        }
        assert!(n.ausfuehren(&serde_json::json!({"key": "zu viel", "value": "x"})).is_err());
        n.ausfuehren(&serde_json::json!({"key": "k0", "value": ""})).unwrap();
        n.ausfuehren(&serde_json::json!({"key": "jetzt", "value": "y".repeat(5000)})).unwrap();
        assert_eq!(g.lock().unwrap().notizen["jetzt"].chars().count(), NOTIZ_ZEICHEN);
    }

    #[test]
    fn auftrag_nennt_ziel_notizen_und_unterbrechung() {
        let mut v = Vorhaben::neu("v1".into(), "Berichte sammeln", 0);
        v.notizen.insert("stand".into(), "3 von 5".into());
        let stand = Rundenstand {
            erledigt: vec![Erledigt { werkzeug: "write_file".into(), argumente: "{\"path\":\"a.md\"}".into(), ergebnis: "ok".into() }],
            notizen: v.notizen.clone(),
        };
        let a = auftrag(&v, &["Runde 1\n\nangefangen".into()], Some(&stand), &grenzen(), Sprache::De);
        assert!(a.contains("Berichte sammeln"));
        assert!(a.contains("stand: 3 von 5"));
        assert!(a.contains("UNTERBROCHEN"));
        assert!(a.contains("write_file({\"path\":\"a.md\"}) -> ok"));
        assert!(a.contains("Runde 1 von höchstens 50"));
    }

    fn ordner(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("myl-vorhaben-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn laeufer_ist_einzeln_und_haelt_beim_schliessen_an() {
        let a = Ablage::neu(ordner("laeufer"));
        let mut v = a.anlegen("Ziel", jetzt()).unwrap();
        v.zustand = Zustand::Schlaeft;
        v.naechste = jetzt() + 3_600;
        a.speichern(&v).unwrap();
        let l = Laeufer::oeffnen(a.clone(), "Probe").unwrap();
        assert_eq!(l.aktive(), 1);
        assert!(l.faellig(jetzt()).is_none(), "schlaeft noch");
        // Eine fremde, lebende Sperre haelt einen zweiten Laeufer ab.
        #[cfg(unix)]
        {
            std::fs::write(a.ordner.join(".laeufer"), "1 0 init").unwrap();
            assert!(Laeufer::oeffnen(a.clone(), "zweiter").unwrap_err().contains("läuft schon"));
            std::fs::write(a.ordner.join(".laeufer"), format!("{} 0 Probe", std::process::id())).unwrap();
        }
        l.schliessen();
        let v = a.laden(&v.kennung).unwrap();
        assert!(v.rest.is_some_and(|r| r > 3_500 && r <= 3_600), "{:?}", v.rest);
        assert!(!a.ordner.join(".laeufer").exists());
        // Wieder geoeffnet: der Rest wird ein Zeitpunkt.
        let l = Laeufer::oeffnen(a.clone(), "Probe").unwrap();
        let v = a.laden(&v.kennung).unwrap();
        assert!(v.rest.is_none());
        assert!(v.naechste > jetzt() + 3_500);
        drop(l);
    }

    /// ⚑ **Hart beendet:** kein `Drop`, kein Rest. Der naechste Laeufer
    /// nimmt den letzten Herzschlag als Zeitpunkt des Anhaltens.
    #[cfg(unix)]
    #[test]
    fn herzschlag_rettet_die_wartezeit_nach_hartem_ende() {
        let a = Ablage::neu(ordner("herzschlag"));
        let mut v = a.anlegen("Ziel", 0).unwrap();
        v.zustand = Zustand::Schlaeft;
        // Letzter Herzschlag vor einer Stunde, die Runde war fuer eine
        // halbe Stunde danach geplant.
        let puls = jetzt() - 3_600;
        v.naechste = puls + 1_800;
        a.speichern(&v).unwrap();
        std::fs::write(a.ordner.join(".laeufer"), format!("999999 {puls} abgestuerzt")).unwrap();
        let _l = Laeufer::oeffnen(a.clone(), "neu").unwrap();
        let v = a.laden(&v.kennung).unwrap();
        assert!(v.naechste >= jetzt() + 1_790, "noch eine halbe Stunde, nicht sofort: {}", v.naechste as i64 - jetzt() as i64);
    }

    /// Eine verwaiste Sperre (Prozess tot) haelt niemanden ab.
    #[cfg(unix)]
    #[test]
    fn verwaiste_sperre_gilt_nicht() {
        let a = Ablage::neu(ordner("verwaist"));
        std::fs::create_dir_all(&a.ordner).unwrap();
        std::fs::write(a.ordner.join(".laeufer"), "999999 0 alt").unwrap();
        assert!(Laeufer::oeffnen(a, "neu").is_ok());
    }

    #[test]
    fn ablage_hin_und_zurueck() {
        let a = Ablage::neu(ordner("ablage"));
        let v = a.anlegen("Ziel A", 5).unwrap();
        let w = a.anlegen("Ziel B", 5).unwrap();
        assert_ne!(v.kennung, w.kennung);
        a.tagebuch_anhaengen(&v.kennung, 1, "eins").unwrap();
        a.tagebuch_anhaengen(&v.kennung, 2, "zwei").unwrap();
        assert_eq!(a.tagebuch_letzte(&v.kennung, 1), ["Runde 2\n\nzwei"]);
        assert_eq!(a.alle().len(), 2);
        assert!(a.anlegen("  ", 5).is_err());
    }

    struct Zaehlend(&'static str, Arc<std::sync::atomic::AtomicU64>);
    impl Werkzeugausfuehrung for Zaehlend {
        fn name(&self) -> &str {
            self.0
        }
        fn ausfuehren(&self, _a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok("echt".into())
        }
    }

    /// ⛔️ **Fund 488: ein lesender Kreis wird gebremst, eine Aenderung
    /// dazwischen hebt die Bremse auf.**
    #[test]
    fn die_wiederholungsbremse_bremst_nur_ohne_aenderung() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let gesehen = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let welt = Arc::new(AtomicU64::new(0));
        let lesen_zaehler = Arc::new(AtomicU64::new(0));
        let schreiben_zaehler = Arc::new(AtomicU64::new(0));
        let hulle = |inner: Box<dyn Werkzeugausfuehrung>| Wiederholungsbremse {
            inner,
            gesehen: Arc::clone(&gesehen),
            welt: Arc::clone(&welt),
            de: true,
        };
        let lesen = hulle(Box::new(Zaehlend("read_file", Arc::clone(&lesen_zaehler))));
        let schreiben = hulle(Box::new(Zaehlend("edit_file", Arc::clone(&schreiben_zaehler))));
        let notiz = hulle(Box::new(Zaehlend("note_set", Arc::new(AtomicU64::new(0)))));
        let a = serde_json::json!({"pfad": "x.py"});
        assert_eq!(lesen.ausfuehren(&a).unwrap(), "echt");
        assert!(lesen.ausfuehren(&a).unwrap().starts_with("(Nicht noch einmal"), "der Kreis laeuft weiter");
        assert_eq!(lesen.ausfuehren(&serde_json::json!({"pfad": "y.py"})).unwrap(), "echt", "anderes Argument");
        // 📌 Eine Notiz dazwischen ist keine Aenderung (Nachtrag, Lauf 3).
        notiz.ausfuehren(&serde_json::json!({"key": "k", "value": "v"})).unwrap();
        assert!(lesen.ausfuehren(&a).unwrap().starts_with("(Nicht noch einmal"), "eine Notiz hebt die Bremse auf");
        schreiben.ausfuehren(&a).unwrap();
        schreiben.ausfuehren(&a).unwrap();
        assert_eq!(schreiben_zaehler.load(Ordering::SeqCst), 2, "ein schreibender Aufruf wird nie gebremst");
        assert_eq!(lesen.ausfuehren(&a).unwrap(), "echt", "nach einer Aenderung liest er wieder");
        assert_eq!(lesen_zaehler.load(Ordering::SeqCst), 3);
    }

    /// ⛔️ **Fund 491: Eine behauptete Ausfuehrung wird als solche markiert.**
    #[test]
    fn behauptete_aufrufe_werden_markiert() {
        let t = "Ich habe die Datei angelegt.\n\nAusgeführt: write_file({\"pfad\":\"x\"})\n[System] Tatsächlich ausgeführt: alles";
        let m = behauptungen_markieren(t, Sprache::De);
        assert!(m.starts_with("Ich habe die Datei angelegt."), "{m}");
        assert!(m.contains("(behauptet, nicht ausgeführt) Ausgeführt: write_file"), "{m}");
        assert!(m.contains("(behauptet, nicht ausgeführt) [System]"), "die Systemmarke laesst sich faelschen: {m}");
    }

    /// ⚑ **Punkt 4.7: Die Abnahme entscheidet ueber „fertig“**, in beide
    /// Richtungen: bestanden ist fertig, auch an der Schrittgrenze und ohne
    /// Abmeldung; nicht bestanden ist nie fertig, auch wenn Modell und
    /// Pruefung es behaupten, und die Ausgabe steht in den Notizen.
    #[test]
    fn die_abnahme_entscheidet_ueber_fertig() {
        let ok = Abnahme { befehl: "python3 a.py".into(), bestanden: true, code: Some(0), ausgabe: String::new() };
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        abschliessen_mit_abnahme(&mut v, &wahl(), None, &Rundenende::Begrenzt, &grenzen(), 0, Some(&ok));
        assert_eq!(v.zustand, Zustand::Fertig, "bestanden an der Schrittgrenze");
        assert!(v.ergebnis.as_deref().unwrap_or("").contains("Abnahme bestanden"));

        let rot = Abnahme { befehl: "python3 a.py".into(), bestanden: false, code: Some(1), ausgabe: "KeyError: 'sensor'".into() };
        let mut v = Vorhaben::neu("v2".into(), "Ziel", 0);
        let mut w = wahl();
        w.fertig = Some("alles erledigt".into());
        let p = Pruefung { fortschritt: true, erreicht: true, grund: "sieht gut aus".into() };
        abschliessen_mit_abnahme(&mut v, &w, Some(&p), &Rundenende::Normal, &grenzen(), 0, Some(&rot));
        assert_ne!(v.zustand, Zustand::Fertig, "nicht bestanden und trotzdem fertig");
        assert_eq!(v.ohne_fortschritt, 0, "der Fortschritt der Pruefung zaehlt weiter");
        assert!(v.notizen["abnahme"].contains("KeyError: 'sensor'"), "{:?}", v.notizen);
        assert!(v.notizen["abnahme"].contains("endete mit 1"));

        // Ohne Abnahme bleibt alles beim Alten.
        let mut v = Vorhaben::neu("v3".into(), "Ziel", 0);
        abschliessen_mit_abnahme(&mut v, &w, Some(&p), &Rundenende::Normal, &grenzen(), 0, None);
        assert_eq!(v.zustand, Zustand::Fertig);
    }

    #[cfg(unix)]
    #[test]
    fn der_abnahmebefehl_laeuft_im_arbeitsordner() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("da.txt"), "x").unwrap();
        let a = abnahme_fahren("test -f da.txt", Some(d.path()));
        assert!(a.bestanden && a.code == Some(0), "{a:?}");
        let b = abnahme_fahren("echo kaputt >&2; exit 3", Some(d.path()));
        assert!(!b.bestanden && b.code == Some(3) && b.ausgabe.contains("kaputt"), "{b:?}");
        assert!(!abnahme_fahren("true", None).bestanden, "ohne Ordner bestanden");
    }

    struct Schreibend(PathBuf);
    impl Werkzeugausfuehrung for Schreibend {
        fn name(&self) -> &str {
            "write_file"
        }
        fn ausfuehren(&self, a: &serde_json::Value) -> Result<String, Werkzeugfehler> {
            let pfad = a["pfad"].as_str().unwrap();
            std::fs::write(self.0.join(pfad), a["inhalt"].as_str().unwrap()).unwrap();
            Ok("angelegt".into())
        }
    }

    /// ⚑ **Punkt 4.8: der Pendelwaechter** sagt an, wenn eine Aenderung
    /// nichts aenderte oder eine Datei auf einen frueheren Stand zurueckfiel.
    #[test]
    fn der_pendelwaechter_sieht_den_kreis() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.py"), "sensor").unwrap();
        let w = Pendelwaechter {
            inner: Box::new(Schreibend(d.path().to_path_buf())),
            wurzel: d.path().to_path_buf(),
            staende: Arc::new(Mutex::new(std::collections::HashMap::new())),
            de: true,
        };
        let schreib = |t: &str| w.ausfuehren(&serde_json::json!({"pfad": "a.py", "inhalt": t})).unwrap();
        assert_eq!(schreib("sens"), "angelegt", "die erste Aenderung ist kein Kreis");
        assert!(schreib("sensor").contains("im Kreis"), "zurueck zum Ausgangsstand");
        assert!(schreib("sensor").contains("nichts geändert"), "Aenderung ohne Wirkung");
        assert_eq!(schreib("sensor;neu"), "angelegt", "ein neuer Stand ist kein Kreis");
    }

    /// ⛔️ **Fund 487: An der Schrittgrenze ist nichts fertig**, auch wenn
    /// die Pruefung „erreicht" sagt; der Fortschritt zaehlt trotzdem.
    #[test]
    fn an_der_schrittgrenze_ist_nichts_fertig() {
        let mut v = Vorhaben::neu("v1".into(), "Ziel", 0);
        let mut w = wahl();
        w.fertig = Some("behauptet".into());
        let p = Pruefung { fortschritt: true, erreicht: true, grund: "sieht gut aus".into() };
        abschliessen(&mut v, &w, Some(&p), &Rundenende::Begrenzt, &grenzen(), 0);
        assert_ne!(v.zustand, Zustand::Fertig, "an der Grenze fertig gemeldet");
        assert_eq!(v.ohne_fortschritt, 0, "der Fortschritt der Pruefung zaehlt nicht mehr");
        assert!(v.notizen["runde"].contains("Schrittgrenze"));
        // Dieselbe Runde mit Abschluss ist fertig.
        abschliessen(&mut v, &w, Some(&p), &Rundenende::Normal, &grenzen(), 0);
        assert_eq!(v.zustand, Zustand::Fertig);
    }

    /// ⚑ **Die Pruefung sieht die Belege** (Fund 487), und ohne Werkzeug
    /// steht dort, dass keines lief.
    #[test]
    fn der_pruefauftrag_zeigt_die_belege() {
        let v = Vorhaben::neu("v1".into(), "Skript reparieren", 0);
        let belege = vec![Erledigt { werkzeug: "run_command".into(), argumente: "python3 a.py".into(), ergebnis: "KeyError: 'sensor'".into() }];
        let t = pruefauftrag(&v, &wahl(), "Alles läuft.", &belege, Sprache::De).remove(0).content;
        assert!(t.contains("KeyError: 'sensor'"), "{t}");
        assert!(t.contains("ohne passenden Beleg zählt nicht"), "{t}");
        let leer = pruefauftrag(&v, &wahl(), "Alles läuft.", &[], Sprache::De).remove(0).content;
        assert!(leer.contains("kein Werkzeug"), "{leer}");
    }

    /// ⚑ **Nur das vorderste laeuft**; die anderen sind „queued", und
    /// Umstellen aendert, welches vorn steht.
    #[test]
    fn die_schlange_faehrt_nur_das_vorderste() {
        let a = Ablage::neu(ordner("schlange"));
        let x = a.anlegen("erstes", 5).unwrap();
        let y = a.anlegen("zweites", 5).unwrap();
        let z = a.anlegen("drittes", 5).unwrap();
        let l = Laeufer::oeffnen(a.clone(), "probe").unwrap();
        assert_eq!(l.faellig(jetzt()).unwrap().kennung, x.kennung, "aelteste zuerst");
        let st: Vec<Stellung> = a.stellungen().into_iter().map(|(_, s)| s).collect();
        assert_eq!(st, [Stellung::Vorn, Stellung::Wartend, Stellung::Wartend]);
        assert_eq!(Stellung::Vorn.wort(&x, true, Sprache::De), "läuft");
        assert_eq!(Stellung::Wartend.wort(&y, true, Sprache::De), "queued");

        // Umstellen per Ziehen: das dritte nach vorn.
        a.reihe_setzen(&[z.kennung.clone(), x.kennung.clone(), y.kennung.clone()]).unwrap();
        assert_eq!(l.faellig(jetzt()).unwrap().kennung, z.kennung);

        // ⚑ Schlaeft das vorderste, wartet die Schlange mit, auch wenn
        //   dahinter eines bereit waere.
        let mut v = a.laden(&z.kennung).unwrap();
        v.zustand = Zustand::Schlaeft;
        v.naechste = jetzt() + 3_600;
        a.speichern(&v).unwrap();
        assert!(l.faellig(jetzt()).is_none(), "das zweite laeuft nicht vor");
        assert_eq!(l.naechster_termin(), Some(v.naechste));

        // Fertig: das naechste in der Reihe ruckt vor.
        v.zustand = Zustand::Fertig;
        a.speichern(&v).unwrap();
        assert_eq!(l.faellig(jetzt()).unwrap().kennung, x.kennung);
    }

    /// Unbekannte Kennungen fallen heraus, fehlende rutschen nach hinten,
    /// und ein neues steht hinten.
    #[test]
    fn die_reihe_verzeiht_luecken() {
        let a = Ablage::neu(ordner("luecken"));
        let x = a.anlegen("eins", 5).unwrap();
        let y = a.anlegen("zwei", 5).unwrap();
        a.reihe_setzen(&["gibt-es-nicht".into(), y.kennung.clone(), y.kennung.clone()]).unwrap();
        let k: Vec<String> = a.reihe().into_iter().map(|v| v.kennung).collect();
        assert_eq!(k, [y.kennung.clone(), x.kennung.clone()]);
        let z = a.anlegen("drei", 6).unwrap();
        assert_eq!(a.reihe().last().unwrap().kennung, z.kennung);
    }

    /// ⚑ Eine Kette steht direkt hinter ihrem Vorgaenger, nicht hinten.
    #[test]
    fn die_kette_reiht_sich_hinter_dem_vorgaenger_ein() {
        let a = Ablage::neu(ordner("kettenreihe"));
        let x = a.anlegen("vorn", 5).unwrap();
        let y = a.anlegen("hinten", 5).unwrap();
        let n = a.anlegen("folge", 6).unwrap();
        a.einreihen_nach(&n.kennung, &x.kennung).unwrap();
        let k: Vec<String> = a.reihe().into_iter().map(|v| v.kennung).collect();
        assert_eq!(k, [x.kennung, n.kennung, y.kennung]);
    }

    /// ⛔️ Entfernen loescht nur einen Vorhabenordner und nie etwas, das
    /// ueber eine Kennung wie `..` erreichbar waere.
    #[test]
    fn entfernen_loescht_nur_ein_vorhaben() {
        let a = Ablage::neu(ordner("entfernen"));
        let x = a.anlegen("weg", 5).unwrap();
        let y = a.anlegen("bleibt", 5).unwrap();
        a.reihe_setzen(&[y.kennung.clone(), x.kennung.clone()]).unwrap();
        for boese in ["..", "", "../x", "a/b", "."] {
            assert!(a.entfernen(boese).is_err(), "{boese}");
        }
        assert!(a.ordner.is_dir());
        a.entfernen(&x.kennung).unwrap();
        assert!(a.laden(&x.kennung).is_err());
        let k: Vec<String> = a.reihe().into_iter().map(|v| v.kennung).collect();
        assert_eq!(k, [y.kennung]);
    }

    #[test]
    fn stoppen_und_weitermachen() {
        let a = Ablage::neu(ordner("stoppen"));
        let x = a.anlegen("eins", 5).unwrap();
        let v = a.stoppen(&x.kennung, Sprache::De).unwrap();
        assert!(!v.aktiv());
        assert_eq!(a.stellungen()[0].1, Stellung::Angehalten);
        let mut v = a.weitermachen(&x.kennung).unwrap();
        assert!(v.faellig(jetzt()));
        assert_eq!(Stellung::Vorn.wort(&v, false, Sprache::En), "next");
        v.runden = 2;
        assert_eq!(Stellung::Vorn.wort(&v, false, Sprache::De), "pausiert");
    }
}
