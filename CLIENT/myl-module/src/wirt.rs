//! **Die Seite der Konsole**: ein geprueftes natives Modul starten, mit ihm
//! sprechen und seine Nachrichten gegen seine Befugnisse pruefen.
//!
//! ⚑ **Die Konsole wartet nie auf ein Modul.** Geschrieben wird in einem
//! eigenen Faden (eine volle Roehre haelt sonst den Schreiber an), gelesen
//! in einem zweiten; die Konsole holt ab, was da ist ([`Lauf::abholen`]).
//! Ein haengendes Modul tut dann nichts, aber es haelt nichts an.
//!
//! ⚑ **Ausgefuehrt wird eine eigene Kopie der gepruefen Bytes**, im
//! Zwischenspeicher unter ihrem Hash, nur fuer den Besitzer lesbar. Wer die
//! Datei im Modulordner nach der Pruefung tauscht, tauscht nicht, was
//! laeuft.

use crate::beschreibung::Befugnisse;
use crate::nachricht::{self, AnModul, Teil, VomModul};
use crate::signatur::Geprueft;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// So lange hat ein Modul fuer den Handschlag.
pub const HANDSCHLAG_FRIST: Duration = Duration::from_secs(10);

/// So viele Textzeilen nimmt die Konsole je Minute von einem Modul an; der
/// Rest wird verworfen, einmal gemeldet.
pub const ZEILEN_JE_MINUTE: usize = 600;

/// Nach so vielen Nachrichten in Folge, die gegen die Befugnisse
/// verstossen, wird das Modul beendet.
pub const VERSTOESSE_HOECHSTENS: u32 = 20;

/// Ein weitergebbarer Sender fuer Ereignisse an ein Modul.
#[derive(Clone)]
pub struct Sender(mpsc::Sender<String>);

impl Sender {
    pub fn senden(&self, e: &AnModul) {
        let _ = self.0.send(nachricht::zeile(e));
    }
}

/// **Was der Wirt an die Konsole weitergibt**: eine gepruefte Nachricht
/// oder ein Ereignis des Laufs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ereignis {
    Nachricht(VomModul),
    /// Eine Nachricht wurde abgelehnt; der Satz sagt warum.
    Abgelehnt(String),
    /// Eine Zeile auf stderr des Moduls.
    Fehlerausgabe(String),
    /// Das Modul ist beendet (Absturz oder Ende).
    Beendet(String),
}

/// **Ein laufendes Modul.**
pub struct Lauf {
    pub name: String,
    /// Die gepruefte Kopie des Programms im Zwischenspeicher, die laeuft.
    pub programm: PathBuf,
    pub befugnisse: Befugnisse,
    kind: std::process::Child,
    an: mpsc::Sender<String>,
    von: mpsc::Receiver<Ereignis>,
    pruefer: Pruefer,
}

/// **Prueft jede Nachricht eines Moduls gegen seine Befugnisse** und
/// saeubert ihren Text.
#[derive(Debug, Clone)]
pub struct Pruefer {
    befugnisse: Befugnisse,
    zeilen_minute: (Instant, usize),
    gemeldet: bool,
    verstoesse: u32,
}

impl Pruefer {
    pub fn neu(befugnisse: Befugnisse) -> Self {
        Self { befugnisse, zeilen_minute: (Instant::now(), 0), gemeldet: false, verstoesse: 0 }
    }

    /// Ob das Modul zu oft verstossen hat.
    pub fn erschoepft(&self) -> bool {
        self.verstoesse >= VERSTOESSE_HOECHSTENS
    }

    fn teile(t: Vec<Teil>) -> Vec<Teil> {
        t.into_iter().map(|x| Teil { text: crate::filter::eine_zeile(&x.text), stil: x.stil }).collect()
    }

    /// **Laesst eine Nachricht durch, gesaeubert, oder lehnt sie ab.**
    pub fn pruefen(&mut self, n: VomModul) -> Result<VomModul, String> {
        let r = self.pruefen_innen(n);
        match &r {
            Ok(_) => self.verstoesse = 0,
            Err(_) => self.verstoesse += 1,
        }
        r
    }

    fn pruefen_innen(&mut self, n: VomModul) -> Result<VomModul, String> {
        let b = &self.befugnisse;
        Ok(match n {
            VomModul::Zeilen { text, stil } => {
                let text = crate::filter::saeubern(&text);
                if self.zeilen_minute.0.elapsed() >= Duration::from_secs(60) {
                    self.zeilen_minute = (Instant::now(), 0);
                    self.gemeldet = false;
                }
                self.zeilen_minute.1 += text.lines().count().max(1);
                if self.zeilen_minute.1 > ZEILEN_JE_MINUTE {
                    let erstes = !self.gemeldet;
                    self.gemeldet = true;
                    return Err(if erstes { format!("mehr als {ZEILEN_JE_MINUTE} Zeilen in einer Minute; der Rest dieser Minute wird verworfen") } else { String::new() });
                }
                VomModul::Zeilen { text, stil }
            }
            VomModul::Fusszeile { teile } if b.fusszeile => VomModul::Fusszeile { teile: Self::teile(teile) },
            VomModul::Unten { zeilen } if b.unten > 0 => VomModul::Unten { zeilen: zeilen.into_iter().take(b.unten as usize).map(Self::teile).collect() },
            VomModul::ModellFragen { id, frage, grenze, denken, strom } if b.modell => {
                VomModul::ModellFragen { id, frage, grenze: grenze.min(8_192), denken, strom }
            }
            VomModul::Agentenlauf { id, auftrag } if b.agent => VomModul::Agentenlauf { id, auftrag },
            VomModul::NutzerFragen { id, frage } => VomModul::NutzerFragen { id, frage: nutzerfrage_saeubern(frage)? },
            VomModul::Modus { aktiv, banner } => VomModul::Modus { aktiv, banner: banner.map(|z| z.iter().take(40).map(|x| crate::filter::eine_zeile(x)).collect()) },
            VomModul::Protokoll { text } => VomModul::Protokoll { text: crate::filter::saeubern(&text) },
            n @ (VomModul::WeckenIn { .. } | VomModul::Hallo { .. }) => n,
            andere => return Err(format!("nicht befugt: {}", art(&andere))),
        })
    }
}

fn art(n: &VomModul) -> &'static str {
    match n {
        VomModul::Fusszeile { .. } => "Fusszeile",
        VomModul::Unten { .. } => "Bereich unten",
        VomModul::ModellFragen { .. } => "Modell fragen",
        VomModul::Agentenlauf { .. } => "Agentenlauf",
        _ => "Nachricht",
    }
}

fn nutzerfrage_saeubern(f: crate::nachricht::Nutzerfrage) -> Result<crate::nachricht::Nutzerfrage, String> {
    use crate::nachricht::{Auswahlpunkt, Nutzerfrage as N};
    let s = |t: &str| crate::filter::saeubern(t);
    Ok(match f {
        N::Wort { text, wort } => {
            let wort = crate::filter::eine_zeile(&wort);
            if wort.trim().is_empty() || wort.chars().count() > 32 {
                return Err("Bestaetigungswort leer oder zu lang".into());
            }
            N::Wort { text: s(&text), wort }
        }
        N::JaNein { text } => N::JaNein { text: s(&text) },
        N::Auswahl { text, punkte } => {
            if punkte.is_empty() || punkte.len() > 64 {
                return Err("Auswahl mit 1 bis 64 Punkten".into());
            }
            N::Auswahl { text: s(&text), punkte: punkte.into_iter().map(|p| Auswahlpunkt { titel: crate::filter::eine_zeile(&p.titel), hinweis: crate::filter::eine_zeile(&p.hinweis) }).collect() }
        }
    })
}

/// **Legt die gepruefen Bytes im Zwischenspeicher ab** und gibt den Pfad
/// zurueck: `<speicher>/<hash>/<dateiname>`, nur fuer den Besitzer. Liegt
/// dort schon eine Datei, wird sie gegen den Hash geprueft und sonst
/// ersetzt.
pub fn ablegen(g: &Geprueft, speicher: &Path) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let name = g.beschreibung.programm_hier().and_then(|p| p.rsplit('/').next()).unwrap_or("modul").to_string();
    let ordner = speicher.join(&g.programm_hash);
    let pfad = ordner.join(&name);
    if std::fs::read(&pfad).is_ok_and(|b| hex::encode(Sha256::digest(&b)) == g.programm_hash) {
        return Ok(pfad);
    }
    std::fs::create_dir_all(&ordner).map_err(|f| format!("{}: {f}", ordner.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(speicher, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o700));
    }
    let tmp = ordner.join(format!("{name}.unfertig"));
    std::fs::write(&tmp, &g.programm).map_err(|f| format!("{}: {f}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o700)).map_err(|f| f.to_string())?;
    }
    std::fs::rename(&tmp, &pfad).map_err(|f| format!("{}: {f}", pfad.display()))?;
    Ok(pfad)
}

/// Was die Konsole beim Start mitgibt.
pub struct Start<'a> {
    pub konsole: &'a str,
    pub sprache: &'a str,
    pub modell: Option<String>,
    pub breite: u16,
    pub hoehe: u16,
    /// Wohin die gepruefen Programme kopiert werden.
    pub speicher: &'a Path,
    /// Wird gerufen, sobald eine Nachricht des Moduls ankommt (aus dem
    /// Lesefaden), etwa um die Eingabezeile zu wecken.
    pub wecker: Option<fn()>,
}

impl Lauf {
    /// **Startet ein geprueftes natives Modul** und wartet auf den
    /// Handschlag. Arbeitsverzeichnis ist der Modulordner.
    pub fn starten(g: &Geprueft, s: &Start) -> Result<Lauf, String> {
        let programm = ablegen(g, s.speicher)?;
        let mut kind = std::process::Command::new(&programm)
            .current_dir(&g.ordner)
            .env("MYL_MODUL_ORDNER", &g.ordner)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|f| format!("{}: {f}", g.beschreibung.name))?;
        let (an, an_ein) = mpsc::channel::<String>();
        let (von_aus, von) = mpsc::channel::<Ereignis>();
        let mut ein = kind.stdin.take().ok_or("keine Eingabe")?;
        std::thread::spawn(move || {
            for z in an_ein {
                if writeln!(ein, "{z}").and_then(|_| ein.flush()).is_err() {
                    return;
                }
            }
        });
        let stdout = kind.stdout.take().ok_or("keine Ausgabe")?;
        let aus = von_aus.clone();
        let wecker = s.wecker;
        std::thread::spawn(move || {
            let mut leser = std::io::BufReader::new(stdout);
            let mut zeile = String::new();
            loop {
                zeile.clear();
                // Eine Zeile wird hoechstens bis zur Grenze gelesen.
                match std::io::Read::take(&mut leser, nachricht::ZEILE_HOECHSTENS as u64 + 1).read_line(&mut zeile) {
                    Ok(0) | Err(_) => {
                        let _ = aus.send(Ereignis::Beendet("Ausgabe geschlossen".into()));
                        return;
                    }
                    Ok(_) => {}
                }
                let e = match nachricht::lesen::<VomModul>(&zeile) {
                    Ok(n) => Ereignis::Nachricht(n),
                    Err(f) => Ereignis::Abgelehnt(f),
                };
                if aus.send(e).is_err() {
                    return;
                }
                if let Some(w) = wecker {
                    w();
                }
            }
        });
        if let Some(stderr) = kind.stderr.take() {
            let aus = von_aus;
            std::thread::spawn(move || {
                for z in std::io::BufReader::new(stderr).lines().map_while(Result::ok).take(100_000) {
                    if aus.send(Ereignis::Fehlerausgabe(crate::filter::eine_zeile(&z))).is_err() {
                        return;
                    }
                }
            });
        }
        let mut lauf = Lauf { name: g.beschreibung.name.clone(), programm: programm.clone(), befugnisse: g.beschreibung.befugnisse.clone(), kind, an, von, pruefer: Pruefer::neu(g.beschreibung.befugnisse.clone()) };
        lauf.senden(&AnModul::Hallo { protokoll: crate::PROTOKOLL, konsole: s.konsole.into(), sprache: s.sprache.into(), modell: s.modell.clone(), breite: s.breite, hoehe: s.hoehe });
        let bis = Instant::now() + HANDSCHLAG_FRIST;
        loop {
            let rest = bis.saturating_duration_since(Instant::now());
            match lauf.von.recv_timeout(rest) {
                Ok(Ereignis::Nachricht(VomModul::Hallo { protokoll, name, .. })) => {
                    if protokoll != crate::PROTOKOLL || name != g.beschreibung.name {
                        lauf.beenden();
                        return Err(format!("{}: Handschlag passt nicht (Protokoll {protokoll}, Name {})", g.beschreibung.name, crate::filter::eine_zeile(&name)));
                    }
                    return Ok(lauf);
                }
                Ok(Ereignis::Beendet(w)) => return Err(format!("{}: beendet vor dem Handschlag ({w})", g.beschreibung.name)),
                Ok(_) => continue,
                Err(_) => {
                    lauf.beenden();
                    return Err(format!("{}: kein Handschlag in {} s", g.beschreibung.name, HANDSCHLAG_FRIST.as_secs()));
                }
            }
        }
    }

    /// Schickt ein Ereignis; blockiert nie.
    pub fn senden(&mut self, e: &AnModul) {
        let _ = self.an.send(nachricht::zeile(e));
    }

    /// **Ein Sender fuer Ereignisse**, der sich weitergeben laesst (etwa an
    /// einen Beobachter, der Stuecke einer Modellantwort schickt).
    pub fn sender(&self) -> Sender {
        Sender(self.an.clone())
    }

    /// **Alles, was seit dem letzten Mal kam**, geprueft. Verstoesst das
    /// Modul zu oft, wird es beendet.
    pub fn abholen(&mut self) -> Vec<Ereignis> {
        let mut aus = Vec::new();
        while let Ok(e) = self.von.try_recv() {
            match e {
                Ereignis::Nachricht(n) => match self.pruefer.pruefen(n) {
                    Ok(n) => aus.push(Ereignis::Nachricht(n)),
                    Err(f) if f.is_empty() => {}
                    Err(f) => aus.push(Ereignis::Abgelehnt(f)),
                },
                Ereignis::Abgelehnt(f) => {
                    self.pruefer.verstoesse += 1;
                    aus.push(Ereignis::Abgelehnt(f));
                }
                andere => aus.push(andere),
            }
            if self.pruefer.erschoepft() {
                self.beenden();
                aus.push(Ereignis::Beendet(format!("{VERSTOESSE_HOECHSTENS} Verstoesse in Folge; beendet")));
                break;
            }
        }
        aus
    }

    /// Ob der Prozess noch laeuft.
    pub fn laeuft(&mut self) -> bool {
        matches!(self.kind.try_wait(), Ok(None))
    }

    /// **Beendet das Modul**: erst `Ende`, nach einer Sekunde hart.
    pub fn beenden(&mut self) {
        self.senden(&AnModul::Ende);
        let bis = Instant::now() + Duration::from_secs(1);
        while Instant::now() < bis {
            if !self.laeuft() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}

impl Drop for Lauf {
    fn drop(&mut self) {
        if self.laeuft() {
            self.beenden();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nachricht::{Nutzerfrage, Stil};

    fn befugt(unten: u16) -> Befugnisse {
        Befugnisse { modell: true, unten, fusszeile: true, ..Default::default() }
    }

    #[test]
    fn befugnisse_greifen() {
        let mut p = Pruefer::neu(Befugnisse::default());
        assert!(p.pruefen(VomModul::ModellFragen { id: 1, frage: "?".into(), grenze: 10, denken: false, strom: false }).is_err(), "ohne Befugnis kein Modell");
        assert!(p.pruefen(VomModul::Agentenlauf { id: 2, auftrag: "x".into() }).is_err());
        assert!(p.pruefen(VomModul::Fusszeile { teile: vec![] }).is_err());
        assert!(p.pruefen(VomModul::Unten { zeilen: vec![] }).is_err());
        assert!(p.pruefen(VomModul::Zeilen { text: "ok".into(), stil: Stil::Normal }).is_ok(), "Text geht immer");
        let mut p = Pruefer::neu(befugt(2));
        let unten = p.pruefen(VomModul::Unten { zeilen: vec![vec![]; 5] }).unwrap();
        assert_eq!(unten, VomModul::Unten { zeilen: vec![vec![]; 2] }, "auf die Befugnis gekuerzt");
        let gross = p.pruefen(VomModul::ModellFragen { id: 1, frage: "?".into(), grenze: 1_000_000, denken: false, strom: false }).unwrap();
        assert!(matches!(gross, VomModul::ModellFragen { grenze: 8_192, .. }));
    }

    #[test]
    fn text_kommt_gesaeubert() {
        let mut p = Pruefer::neu(befugt(1));
        let z = p.pruefen(VomModul::Zeilen { text: "\x1b[2Jhallo".into(), stil: Stil::Normal }).unwrap();
        assert_eq!(z, VomModul::Zeilen { text: "[2Jhallo".into(), stil: Stil::Normal });
        let f = p.pruefen(VomModul::Fusszeile { teile: vec![Teil { text: "a\nb\x1b".into(), stil: Stil::Gewinn }] }).unwrap();
        assert_eq!(f, VomModul::Fusszeile { teile: vec![Teil { text: "a b".into(), stil: Stil::Gewinn }] });
        let w = p.pruefen(VomModul::NutzerFragen { id: 1, frage: Nutzerfrage::Wort { text: "x".into(), wort: "".into() } });
        assert!(w.is_err(), "leeres Bestaetigungswort");
    }

    #[test]
    fn zu_viele_zeilen_und_verstoesse() {
        let mut p = Pruefer::neu(befugt(0));
        for _ in 0..ZEILEN_JE_MINUTE {
            assert!(p.pruefen(VomModul::Zeilen { text: "x".into(), stil: Stil::Normal }).is_ok());
        }
        let erste = p.pruefen(VomModul::Zeilen { text: "x".into(), stil: Stil::Normal }).unwrap_err();
        assert!(!erste.is_empty(), "einmal gemeldet");
        assert_eq!(p.pruefen(VomModul::Zeilen { text: "x".into(), stil: Stil::Normal }).unwrap_err(), "", "danach still verworfen");
        let mut p = Pruefer::neu(Befugnisse::default());
        for _ in 0..VERSTOESSE_HOECHSTENS {
            let _ = p.pruefen(VomModul::Agentenlauf { id: 1, auftrag: String::new() });
        }
        assert!(p.erschoepft());
    }

    /// Ein signiertes Skriptmodul in einem eigenen Ordner (nur Unix).
    #[cfg(unix)]
    fn skriptmodul(kennung: &str, skript: &str) -> (PathBuf, Geprueft) {
        use crate::signatur::{schluessel_erzeugen, signieren, Versionsstand, Vertrauensliste, Vertrauter};
        let o = std::env::temp_dir().join(format!("myl-wirt-{kennung}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&o);
        std::fs::create_dir_all(o.join("bin")).unwrap();
        let ziel = crate::beschreibung::zielsystem();
        std::fs::write(o.join(crate::BESCHREIBUNG), crate::beschreibung::tests::beispiel().replace("aarch64-macos", &ziel)).unwrap();
        std::fs::write(o.join("bin/beispiel"), skript).unwrap();
        let (geheim, oeffentlich) = schluessel_erzeugen().unwrap();
        signieren(&o, &geheim).unwrap();
        let v = Vertrauensliste { schluessel: vec![Vertrauter { name: "P".into(), oeffentlich, nativ: true, widerrufen: false }] };
        let g = crate::signatur::pruefen(&o, &v, &Versionsstand::default()).unwrap();
        (o, g)
    }

    /// ⚑ **Ein Handschlag mit fremdem Namen startet nicht.**
    #[cfg(unix)]
    #[test]
    fn falscher_handschlag() {
        let (o, g) = skriptmodul("name", "#!/bin/sh\nread z\necho '{\"art\":\"hallo\",\"protokoll\":1,\"name\":\"anderes\",\"version\":\"0.1.0\"}'\nsleep 5\n");
        let speicher = o.join("speicher");
        let r = Lauf::starten(&g, &Start { konsole: "probe", sprache: "de", modell: None, breite: 80, hoehe: 24, speicher: &speicher, wecker: None });
        assert!(r.as_ref().err().is_some_and(|f| f.contains("Handschlag passt nicht")), "{:?}", r.err());
        let _ = std::fs::remove_dir_all(&o);
    }

    /// ⚑ **Ein echter Lauf** mit einem kleinen Skriptmodul (nur Unix).
    #[cfg(unix)]
    #[test]
    fn handschlag_und_absturz() {
        // Antwortet auf den Handschlag, schreibt eine Zeile mit Steuerzeichen und endet.
        let (o, g) = skriptmodul(
            "lauf",
            "#!/bin/sh\nread z\necho '{\"art\":\"hallo\",\"protokoll\":1,\"name\":\"beispiel\",\"version\":\"0.1.0\"}'\necho '{\"art\":\"zeilen\",\"text\":\"\\u001b[2Jda\",\"stil\":\"normal\"}'\necho '{\"art\":\"agentenlauf\",\"id\":1,\"auftrag\":\"x\"}'\nexit 3\n",
        );
        // Nach der Pruefung im Ordner getauscht: laeuft trotzdem die gepruefte Kopie.
        std::fs::write(o.join("bin/beispiel"), "#!/bin/sh\nexit 9\n").unwrap();
        let speicher = o.join("speicher");
        let mut lauf = Lauf::starten(&g, &Start { konsole: "probe", sprache: "de", modell: None, breite: 80, hoehe: 24, speicher: &speicher, wecker: None }).unwrap();
        let bis = Instant::now() + Duration::from_secs(5);
        let mut alles = Vec::new();
        while Instant::now() < bis && !alles.iter().any(|e| matches!(e, Ereignis::Beendet(_))) {
            alles.extend(lauf.abholen());
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(alles.contains(&Ereignis::Nachricht(VomModul::Zeilen { text: "[2Jda".into(), stil: Stil::Normal })), "{alles:?}");
        assert!(alles.iter().any(|e| matches!(e, Ereignis::Abgelehnt(f) if f.contains("Agentenlauf"))), "{alles:?}");
        assert!(alles.iter().any(|e| matches!(e, Ereignis::Beendet(_))), "{alles:?}");
        let _ = std::fs::remove_dir_all(&o);
    }
}
