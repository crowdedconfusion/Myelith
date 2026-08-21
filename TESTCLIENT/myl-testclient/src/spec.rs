//! Testplan — die Datei, die der Koordinator verteilt.
//!
//! ## Das Problem, das sie löst
//!
//! Der Cross-Hardware-Nachweis verlangt, dass **alle Beteiligten exakt
//! dieselben Parameter** verwenden. Ein Leerzeichen zu viel im Prompt,
//! eine andere Tokenzahl — und die Digests weichen ab. Das sieht dann
//! aus wie ein Befund an der Kernthese, ist aber ein Tippfehler.
//!
//! Bisher stand in der Anleitung „nehmt exakt dieselben Werte". Das ist
//! eine Bitte, keine Absicherung. Der Testplan macht daraus eine Datei:
//! Der Koordinator erzeugt sie einmal, schickt sie herum, und der Client
//! **weigert sich**, mit einer veränderten Datei zu arbeiten.
//!
//! ## Format
//!
//! Bewusst `schlüssel = wert` je Zeile, kein TOML/JSON: Die Datei wird
//! per Chat und Mail weitergereicht, von Menschen gelesen und
//! gelegentlich von Hand angelegt. Sie muss ohne Werkzeug verständlich
//! sein, und der Client soll sie ohne Fremd-Crate lesen können.
//!
//! **Der Prompt steht in Anführungszeichen.** Das ist keine Kosmetik:
//! Ein führendes oder abschließendes Leerzeichen ist Teil des Prompts
//! und verändert das Ergebnis — ohne Anführungszeichen würde es beim
//! Einlesen wegfallen und der Digest wäre ein anderer, ohne dass jemand
//! sieht warum. `\n`, `\"` und `\\` werden maskiert.
//!
//! **Mehrere `prompt`-Zeilen ergeben eine Reihe**, in der Reihenfolge
//! der Datei. Ein einzelner Prompt übt einen einzigen Pfad durch das
//! Modell aus; ein Rundungsfehler, der nur bei langen Sequenzen oder in
//! einem selten getroffenen LUT-Bereich auftritt, bliebe unentdeckt und
//! der Vergleichswert sähe trotzdem beruhigend aus.
//!
//! Wiederholte Schlüssel statt `prompt.1`, `prompt.2`: Eine Datei mit
//! einem einzigen Prompt bleibt damit unverändert gültig, und beim
//! Erweitern hängt man eine Zeile an, statt durchzunummerieren.
//!
//! ```text
//! plan_id     = 2026-08-18-cross-arch-01
//! prompt      = "Die Hauptstadt von Frankreich ist"
//! prompt      = "The capital of France is"
//! prompt      = "Es war einmal"
//! steps       = 8
//! shards      = 4
//! model       = qwen2.5-0.5b
//! spec_sha256 = 9f2c…
//! ```
//!
//! ## Die Prüfsumme
//!
//! `spec_sha256` deckt **genau die Felder ab, die gleich sein müssen** —
//! nicht die Kommentare, nicht die Reihenfolge, nicht den `plan_id`.
//! Wer den Prompt ändert, bekommt beim nächsten Lauf einen Fehler statt
//! eines falschen Befunds. Wer einen Kommentar ergänzt, wird nicht
//! behelligt.
//!
//! Derselbe Wert (gekürzt) benennt auch das Protokollverzeichnis —
//! damit landen die Läufe aller Teilnehmer im gleichnamigen Ordner und
//! sind ohne Zuordnungsarbeit vergleichbar.

use std::fmt::Write as _;
use std::path::Path;

use crate::logging::sha256_hex;

/// Setzt einen Wert in Anführungszeichen und maskiert Sonderzeichen.
fn zitieren(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Entfernt Anführungszeichen und Maskierungen.
///
/// Ohne Anführungszeichen wird der Wert unverändert übernommen — damit
/// bleibt eine von Hand geschriebene Datei ohne Quotes lesbar, solange
/// der Prompt keine Randleerzeichen hat.
fn entzitieren(s: &str) -> String {
    if !(s.len() >= 2 && s.starts_with('"') && s.ends_with('"')) {
        return s.to_string();
    }
    let inner = &s[1..s.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Ein Testplan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPlan {
    /// Freie Kennung des Durchgangs, z. B. `2026-08-18-cross-arch-01`.
    /// Geht **nicht** in die Prüfsumme ein — sie benennt den Durchgang,
    /// sie bestimmt ihn nicht.
    pub plan_id: String,
    /// Die Prompts, zeichengenau und in dieser Reihenfolge.
    ///
    /// Die Reihenfolge ist wirksam: Sie geht in die Prüfsumme ein und
    /// bestimmt, in welcher Folge die Einzeldigests zum Gesamtwert
    /// zusammengefasst werden.
    pub prompts: Vec<String>,
    /// Zu erzeugende Token.
    pub steps: usize,
    /// Shards für den `shard`-Lauf.
    pub shards: usize,
    /// Modellkennung (Artefaktverzeichnis-Name).
    pub model: String,
}

/// Fehler beim Lesen oder Prüfen eines Plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// Datei nicht lesbar.
    NichtLesbar(String),
    /// Pflichtfeld fehlt.
    FeldFehlt(&'static str),
    /// Feld hat einen unbrauchbaren Wert.
    FeldUngueltig { feld: String, wert: String },
    /// Die Prüfsumme passt nicht zum Inhalt.
    PruefsummeFalsch { erwartet: String, berechnet: String },
    /// Die Datei trägt keine Prüfsumme.
    PruefsummeFehlt,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NichtLesbar(e) => write!(f, "Testplan nicht lesbar: {}", e),
            Self::FeldFehlt(k) => write!(f, "Pflichtfeld '{}' fehlt im Testplan", k),
            Self::FeldUngueltig { feld, wert } => {
                write!(f, "Feld '{}' hat den ungültigen Wert '{}'", feld, wert)
            }
            Self::PruefsummeFalsch {
                erwartet,
                berechnet,
            } => write!(
                f,
                "Der Testplan wurde verändert.\n     \
                 Prüfsumme in der Datei: {}\n     \
                 tatsächlicher Inhalt:   {}\n     \
                 Verwende die Originaldatei des Koordinators — ein \
                 geänderter Parameter erzeugt abweichende Ergebnisse, \
                 die wie ein Befund aussehen, aber keiner sind.",
                erwartet, berechnet
            ),
            Self::PruefsummeFehlt => write!(
                f,
                "Der Testplan trägt keine Prüfsumme (spec_sha256). \
                 Mit `myl-test plan --neu` erzeugen lassen."
            ),
        }
    }
}

impl std::error::Error for PlanError {}

impl TestPlan {
    /// Ein Plan mit den Vorgabewerten.
    pub fn vorgaben() -> Self {
        Self {
            plan_id: "unbenannt".to_string(),
            prompts: vec!["Die Hauptstadt von Frankreich ist".to_string()],
            steps: 8,
            shards: 4,
            model: crate::runs::DEFAULT_MODEL.to_string(),
        }
    }

    /// Kanonische Bytefolge der **wirksamen** Felder.
    ///
    /// `plan_id` fehlt hier bewusst: Zwei Koordinatoren, die denselben
    /// Test unter verschiedenen Namen fahren, sollen vergleichbare
    /// Ergebnisse bekommen.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut s = String::new();
        // Mit Nummer, damit die Reihenfolge wirksam ist und zwei Prompts
        // nicht zu einem verschmelzen können.
        for (i, p) in self.prompts.iter().enumerate() {
            let _ = writeln!(s, "prompt.{}={}", i + 1, p);
        }
        let _ = writeln!(s, "steps={}", self.steps);
        let _ = writeln!(s, "shards={}", self.shards);
        let _ = writeln!(s, "model={}", self.model);
        s.into_bytes()
    }

    /// Vollständige Prüfsumme über die wirksamen Felder.
    pub fn checksum(&self) -> String {
        sha256_hex(&self.canonical_bytes())
    }

    /// Kurzform der Prüfsumme — benennt das Protokollverzeichnis.
    ///
    /// Acht Hexzeichen: kurz genug für einen Verzeichnisnamen, lang
    /// genug, dass eine zufällige Kollision zwischen den Plänen eines
    /// Projekts praktisch ausgeschlossen ist.
    pub fn short_id(&self) -> String {
        self.checksum()[..8].to_string()
    }

    /// Schreibt den Plan als Textdatei.
    pub fn to_file_text(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "# Myelith-Testplan\n\
             #\n\
             # Diese Datei legt fest, womit gemessen wird. Alle Beteiligten\n\
             # MÜSSEN sie unverändert verwenden — sonst weichen die Ergebnisse\n\
             # ab, und das sieht aus wie ein Befund, ist aber ein Tippfehler.\n\
             #\n\
             # Aufruf:  myl-test --plan {}.plan determinismus\n\
             #    oder:  myl-test  (Menü, Punkt 8)\n\
             #\n\
             # Die Zeile spec_sha256 sichert genau das ab. Kommentare und\n\
             # Reihenfolge gehen NICHT in die Prüfsumme ein, die Werte schon.\n\
             \n",
            self.plan_id
        );
        let _ = writeln!(s, "plan_id     = {}", self.plan_id);
        for p in &self.prompts {
            let _ = writeln!(s, "prompt      = {}", zitieren(p));
        }
        let _ = writeln!(s, "steps       = {}", self.steps);
        let _ = writeln!(s, "shards      = {}", self.shards);
        let _ = writeln!(s, "model       = {}", self.model);
        let _ = writeln!(s, "\nspec_sha256 = {}", self.checksum());
        s
    }

    /// Speichert den Plan.
    pub fn save(&self, pfad: &Path) -> Result<(), PlanError> {
        if let Some(dir) = pfad.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| PlanError::NichtLesbar(e.to_string()))?;
            }
        }
        std::fs::write(pfad, self.to_file_text()).map_err(|e| PlanError::NichtLesbar(e.to_string()))
    }

    /// Liest einen Plan und **prüft die Prüfsumme**.
    pub fn load(pfad: &Path) -> Result<Self, PlanError> {
        let text =
            std::fs::read_to_string(pfad).map_err(|e| PlanError::NichtLesbar(e.to_string()))?;
        Self::parse(&text)
    }

    /// Zerlegt den Text und prüft die Prüfsumme.
    pub fn parse(text: &str) -> Result<Self, PlanError> {
        let mut felder: Vec<(String, String)> = Vec::new();
        for zeile in text.lines() {
            let z = zeile.trim();
            if z.is_empty() || z.starts_with('#') {
                continue;
            }
            let Some((k, v)) = z.split_once('=') else {
                continue;
            };
            felder.push((k.trim().to_string(), v.trim().to_string()));
        }
        let hole = |k: &str| felder.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());

        let zahl = |k: &'static str| -> Result<usize, PlanError> {
            let roh = hole(k).ok_or(PlanError::FeldFehlt(k))?;
            roh.parse::<usize>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or(PlanError::FeldUngueltig {
                    feld: k.to_string(),
                    wert: roh,
                })
        };

        let prompts: Vec<String> = felder
            .iter()
            .filter(|(k, _)| k == "prompt")
            .map(|(_, v)| entzitieren(v))
            .collect();
        if prompts.is_empty() {
            return Err(PlanError::FeldFehlt("prompt"));
        }

        let plan = Self {
            plan_id: hole("plan_id").unwrap_or_else(|| "unbenannt".to_string()),
            prompts,
            steps: zahl("steps")?,
            shards: zahl("shards")?,
            model: hole("model").ok_or(PlanError::FeldFehlt("model"))?,
        };

        let erwartet = hole("spec_sha256").ok_or(PlanError::PruefsummeFehlt)?;
        let berechnet = plan.checksum();
        if erwartet != berechnet {
            return Err(PlanError::PruefsummeFalsch {
                erwartet,
                berechnet,
            });
        }
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("myl-testclient-plan-{}", name));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn rundtrip_ueber_datei() {
        let dir = tempdir("rundtrip");
        let pfad = dir.join("test.plan");
        let plan = TestPlan {
            plan_id: "2026-08-18-cross-arch-01".into(),
            prompts: vec![
                "Die Hauptstadt von Frankreich ist".into(),
                "The capital of France is".into(),
            ],
            steps: 8,
            shards: 4,
            model: "qwen2.5-0.5b".into(),
        };
        plan.save(&pfad).expect("speichern");
        assert_eq!(TestPlan::load(&pfad).expect("laden"), plan);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Der Kern: Ein veränderter Prompt muss auffallen, statt still zu
    /// einem abweichenden Ergebnis zu führen.
    #[test]
    fn veraenderter_prompt_wird_erkannt() {
        let plan = TestPlan::vorgaben();
        let text = plan.to_file_text();
        let manipuliert = text.replace(
            "\"Die Hauptstadt von Frankreich ist\"",
            "\"Die Hauptstadt von Frankreich ist \"",
        );
        match TestPlan::parse(&manipuliert) {
            Err(PlanError::PruefsummeFalsch { .. }) => {}
            other => panic!("erwartete Prüfsummenfehler, bekam {:?}", other),
        }
    }

    #[test]
    fn veraenderte_tokenzahl_wird_erkannt() {
        let text = TestPlan::vorgaben().to_file_text().replace("steps       = 8", "steps       = 9");
        assert!(matches!(
            TestPlan::parse(&text),
            Err(PlanError::PruefsummeFalsch { .. })
        ));
    }

    /// Kommentare und Leerzeilen dürfen frei ergänzt werden — sie gehen
    /// nicht in die Prüfsumme ein.
    #[test]
    fn kommentare_stoeren_die_pruefsumme_nicht() {
        let text = TestPlan::vorgaben().to_file_text();
        let mit_notiz = format!("# Notiz eines Teilnehmers\n\n{}\n# Ende\n", text);
        assert_eq!(
            TestPlan::parse(&mit_notiz).expect("gültig"),
            TestPlan::vorgaben()
        );
    }

    /// Die Kennung benennt den Durchgang, sie bestimmt ihn nicht.
    #[test]
    fn plan_id_geht_nicht_in_die_pruefsumme_ein() {
        let mut a = TestPlan::vorgaben();
        let mut b = TestPlan::vorgaben();
        a.plan_id = "durchgang-a".into();
        b.plan_id = "durchgang-b".into();
        assert_eq!(a.checksum(), b.checksum());
        assert_eq!(a.short_id(), b.short_id());
    }

    #[test]
    fn wirksame_felder_aendern_die_pruefsumme() {
        let basis = TestPlan::vorgaben();
        for aendern in [
            |p: &mut TestPlan| p.prompts = vec!["anders".into()],
            |p: &mut TestPlan| p.prompts.push("noch einer".into()),
            |p: &mut TestPlan| p.steps = 16,
            |p: &mut TestPlan| p.shards = 8,
            |p: &mut TestPlan| p.model = "anderes-modell".into(),
        ] {
            let mut p = basis.clone();
            aendern(&mut p);
            assert_ne!(p.checksum(), basis.checksum());
        }
    }

    /// Mehrere `prompt`-Zeilen ergeben eine Reihe in der Reihenfolge der
    /// Datei — das ist das ganze Format.
    #[test]
    fn mehrere_prompts_bleiben_in_reihenfolge() {
        let mut plan = TestPlan::vorgaben();
        plan.prompts = vec!["eins".into(), "zwei".into(), "drei".into()];
        let zurueck = TestPlan::parse(&plan.to_file_text()).expect("gültig");
        assert_eq!(zurueck.prompts, vec!["eins", "zwei", "drei"]);
    }

    /// Die Reihenfolge ist wirksam: Sie bestimmt, in welcher Folge die
    /// Einzeldigests zum Gesamtwert zusammengefasst werden. Zwei Pläne mit
    /// denselben Prompts in anderer Folge sind verschiedene Pläne.
    #[test]
    fn reihenfolge_der_prompts_aendert_die_pruefsumme() {
        let mut a = TestPlan::vorgaben();
        let mut b = TestPlan::vorgaben();
        a.prompts = vec!["eins".into(), "zwei".into()];
        b.prompts = vec!["zwei".into(), "eins".into()];
        assert_ne!(a.checksum(), b.checksum());
    }

    /// Zwei Prompts dürfen nicht zu einem verschmelzen können — sonst
    /// hätten verschiedene Pläne dieselbe Prüfsumme.
    #[test]
    fn prompts_verschmelzen_nicht() {
        let mut a = TestPlan::vorgaben();
        let mut b = TestPlan::vorgaben();
        a.prompts = vec!["ab".into()];
        b.prompts = vec!["a".into(), "b".into()];
        assert_ne!(a.checksum(), b.checksum());

        let mut c = TestPlan::vorgaben();
        c.prompts = vec!["a\nb".into()];
        assert_ne!(c.checksum(), b.checksum());
    }

    /// Ein Plan aus der Zeit vor den Prompt-Reihen bleibt gültig — dort
    /// steht genau eine `prompt`-Zeile.
    #[test]
    fn plan_mit_einem_prompt_bleibt_gueltig() {
        let mut plan = TestPlan::vorgaben();
        plan.prompts = vec!["nur einer".into()];
        let zurueck = TestPlan::parse(&plan.to_file_text()).expect("gültig");
        assert_eq!(zurueck, plan);
    }

    #[test]
    fn fehlende_pruefsumme_wird_gemeldet() {
        let text = "prompt = x\nsteps = 1\nshards = 1\nmodell = y\nmodel = y\n";
        assert_eq!(TestPlan::parse(text), Err(PlanError::PruefsummeFehlt));
    }

    #[test]
    fn fehlende_pflichtfelder_werden_gemeldet() {
        assert_eq!(
            TestPlan::parse("steps = 1\nshards = 1\nmodel = y\n"),
            Err(PlanError::FeldFehlt("prompt"))
        );
    }

    #[test]
    fn unbrauchbare_zahlen_werden_gemeldet() {
        for text in [
            "prompt = x\nsteps = null\nshards = 1\nmodel = y\n",
            "prompt = x\nsteps = 0\nshards = 1\nmodel = y\n",
        ] {
            assert!(matches!(
                TestPlan::parse(text),
                Err(PlanError::FeldUngueltig { .. })
            ));
        }
    }

    /// Prompts mit `=` dürfen nicht zerschnitten werden, und
    /// Randleerzeichen müssen erhalten bleiben — sie sind Teil des
    /// Prompts und verändern den Digest.
    #[test]
    fn prompt_behaelt_sonderzeichen_und_randleerzeichen() {
        for prompt in [
            "Loese: 2 + 2 = ",
            "  fuehrende Leerzeichen",
            "mit \"Anfuehrungszeichen\"",
            "mit \\ Backslash",
            "Zeile eins\nZeile zwei",
        ] {
            let mut plan = TestPlan::vorgaben();
            plan.prompts = vec![prompt.to_string()];
            let zurueck = TestPlan::parse(&plan.to_file_text())
                .unwrap_or_else(|e| panic!("Prompt {:?}: {}", prompt, e));
            assert_eq!(zurueck.prompts, vec![prompt], "Prompt {:?} verändert", prompt);
        }
    }

    /// Eine von Hand ohne Anführungszeichen geschriebene Datei bleibt
    /// lesbar — solange der Prompt keine Randleerzeichen hat.
    #[test]
    fn unzitierter_prompt_bleibt_lesbar() {
        let plan = TestPlan::vorgaben();
        let text = format!(
            "prompt = {}\nsteps = {}\nshards = {}\nmodel = {}\nspec_sha256 = {}\n",
            plan.prompts[0],
            plan.steps,
            plan.shards,
            plan.model,
            plan.checksum()
        );
        assert_eq!(
            TestPlan::parse(&text).expect("gültig").prompts,
            plan.prompts
        );
    }

    #[test]
    fn short_id_ist_acht_hexzeichen() {
        let id = TestPlan::vorgaben().short_id();
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// Zwei Beteiligte mit demselben Plan müssen dasselbe Verzeichnis
    /// bekommen — das ist der ganze Zweck der Kurzkennung.
    #[test]
    fn gleicher_plan_gleiche_kurzkennung() {
        let a = TestPlan::parse(&TestPlan::vorgaben().to_file_text()).unwrap();
        let b = TestPlan::parse(&TestPlan::vorgaben().to_file_text()).unwrap();
        assert_eq!(a.short_id(), b.short_id());
    }
}
