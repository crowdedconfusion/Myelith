//! **Module als WebAssembly**: fremder Code, der nur kann, was die Konsole
//! ihm reicht.
//!
//! Ein abgeschottetes Modul spricht dieselben Nachrichten wie ein natives,
//! nur laeuft es in der Konsole und hat von sich aus weder Dateisystem noch
//! Netz. Was es braucht, sind Funktionen dieses Wirts:
//!
//! | Funktion | Grenze |
//! |---|---|
//! | `http` | nur Rechner, die `modul.json` nennt **und** der Mensch genehmigt hat; keine Weiterleitung, keine privaten Adressen |
//! | `datei_lesen`, `datei_schreiben` | nur der eigene Datenordner, mit Deckel |
//! | `geheimnis` | nur die eigenen, nur lesend |
//!
//! ⚑ **Die Laufzeit ist ein Interpreter** (2026-11-09): Sie erzeugt keinen
//! Maschinencode, und ihr Baum ist klein (30 Kisten, 3 MB im Vorrat). Ein
//! Modul rechnet wenig, die Zeit liegt in Netz und Modell: gemessen rund 12 ns
//! je Schleifendurchlauf mit vier Rechnungen und einem Speicherzugriff.
//!
//! ⚠️ **Was das nicht abwehrt:** Seitenkanaele ueber die Zeit und Fehler in der
//! Laufzeit selbst. Deshalb bleibt die Signatur davor: Abschottung ersetzt die
//! Herkunft nicht.
//!
//! ⚑ **Treibstoff je Ereignis, nicht je Lauf.** Ein Faden laesst sich nicht
//! von aussen anhalten; was ein Modul in einem Ereignis rechnen darf, ist
//! deshalb begrenzt ([`TREIBSTOFF_JE_EREIGNIS`]), und nach der Grenze endet es.

use crate::ablage::{genehmigt, ist_schluesselname, modulort};
use crate::beschreibung::{sicherer_pfad, Laufzeit};
use crate::nachricht::{self, AnModul, Httpanfrage, Httpantwort, VomModul};
use crate::signatur::{schreiben_atomar, Geprueft};
use crate::wirt::Ereignis;
use std::io::{Read, Write};
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use wasmi::{Caller, Config, Engine, Error, Extern, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder};

/// So gross darf ein Modul sein, bevor es uebersetzt wird.
pub const MODUL_HOECHSTENS: usize = 16 << 20;
/// Linearer Speicher eines Moduls.
pub const SPEICHER_HOECHSTENS: usize = 64 << 20;
/// Rechnung je Ereignis: gemessen 0,5 bis 1 s auf dem M5 Pro.
pub const TREIBSTOFF_JE_EREIGNIS: u64 = 2_000_000_000;
/// Nachrichten an die Konsole je Ereignis.
pub const AUSGABEN_JE_EREIGNIS: usize = 1_000;
/// Bytes an die Konsole je Ereignis.
pub const AUSGABE_BYTES_JE_EREIGNIS: usize = 16 << 20;
/// Protokollzeilen eines Laufs, wie bei einem nativen Modul.
pub const PROTOKOLLZEILEN: usize = 100_000;
/// Netzanfragen je Minute.
pub const HTTP_JE_MINUTE: usize = 60;
/// Eine Anfrage ins Netz, samt Kopf und Koerper.
pub const HTTP_ANFRAGE_HOECHSTENS: usize = 1 << 20;
/// Eine Antwort aus dem Netz.
pub const HTTP_ANTWORT_HOECHSTENS: usize = 8 << 20;
/// Sekunden fuer eine Anfrage.
pub const HTTP_FRIST_S: u32 = 20;
/// Eine Datei im Datenordner.
pub const DATEI_HOECHSTENS: usize = 16 << 20;
/// Der ganze Datenordner eines Moduls.
pub const DATEN_HOECHSTENS: u64 = 256 << 20;

/// Eine zerlegte Adresse.
#[derive(Debug, PartialEq, Eq)]
struct Adresse {
    https: bool,
    rechner: String,
    port: u16,
    eintrag: String,
}

/// ⛔️ **Von Hand und streng.** Alles, was zwei Parser verschieden lesen
/// koennten (`user@`, Rueckstriche, Leerzeichen), ist hier schon falsch.
fn adresse_lesen(url: &str) -> Result<Adresse, String> {
    let (https, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err("nur https".into());
    };
    if !url.bytes().all(|c| c.is_ascii_graphic()) || url.contains('\\') {
        return Err("Zeichen, die in keine Adresse gehoeren".into());
    }
    let ende = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let eintrag = &rest[..ende];
    if eintrag.contains('@') || eintrag.is_empty() {
        return Err("kein Rechner in der Adresse".into());
    }
    let (rechner, port) = match eintrag.rsplit_once(':') {
        Some((r, p)) => (r, p.parse::<u16>().map_err(|_| "kein Port".to_string())?),
        None => (eintrag, if https { 443 } else { 80 }),
    };
    if rechner.is_empty() || !rechner.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-') {
        return Err("kein Rechnername".into());
    }
    Ok(Adresse { https, rechner: rechner.to_ascii_lowercase(), port, eintrag: eintrag.to_ascii_lowercase() })
}

fn ist_lokaler_name(rechner: &str) -> bool {
    rechner == "localhost" || rechner == "127.0.0.1"
}

/// ⛔️ **Ob eine aufgeloeste Adresse ins oeffentliche Netz zeigt.** Ein genehmigter
/// Name kann auf `127.0.0.1` zeigen; dann erreichte das Modul die Tuer und die
/// Beobachtung dieses Rechners.
fn oeffentlich(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_broadcast()
                || v.is_documentation()
                || v.is_unspecified()
                || v.is_multicast()
                || (o[0] == 100 && (64..128).contains(&o[1]))
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
                || o[0] >= 240)
        }
        IpAddr::V6(v) => match v.to_ipv4_mapped() {
            Some(m) => oeffentlich(IpAddr::V4(m)),
            None => {
                let s = v.segments()[0];
                !(v.is_loopback()
                    || v.is_unspecified()
                    || v.is_multicast()
                    || (s & 0xfe00) == 0xfc00
                    || (s & 0xffc0) == 0xfe80)
            }
        },
    }
}

/// ⛔️ **Alle Adressen eines Namens, nicht eine.** Die erste wird danach fest
/// vorgegeben (`resolve`), das schliesst das Umbiegen zwischen Pruefen und
/// Verbinden.
fn ziel_ip(rechner: &str, lokal: bool, adressen: &[IpAddr]) -> Result<IpAddr, String> {
    let ip = adressen.first().copied().ok_or("keine Adresse")?;
    if !lokal && !adressen.iter().all(|a| oeffentlich(*a)) {
        return Err(format!("{rechner} zeigt auf eine private Adresse"));
    }
    Ok(ip)
}

/// Eine Zeichenkette fuer eine `curl`-Konfiguration.
fn maskiert(s: &str) -> String {
    let mut a = String::with_capacity(s.len() + 2);
    a.push('"');
    for c in s.chars() {
        match c {
            '\\' => a.push_str("\\\\"),
            '"' => a.push_str("\\\""),
            '\n' => a.push_str("\\n"),
            '\r' => a.push_str("\\r"),
            '\t' => a.push_str("\\t"),
            c => a.push(c),
        }
    }
    a.push('"');
    a
}

/// ⚑ **Ueber die Standardeingabe**, damit Schluessel in Kopfzeilen nicht in
/// der Prozessliste stehen; `curl` ist derselbe Weg wie bei der Aktualisierung.
fn konfiguration(a: &Httpanfrage, ziel: &Adresse, ip: IpAddr) -> Result<String, String> {
    let methode = a.methode.as_deref().unwrap_or("GET");
    if !matches!(methode, "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") {
        return Err(format!("keine Methode: {}", crate::filter::eine_zeile(methode)));
    }
    let ip = match ip {
        IpAddr::V6(v) => format!("[{v}]"),
        v => v.to_string(),
    };
    let mut k = format!(
        "url = {}\nrequest = {}\nresolve = {}\nsilent\nshow-error\nmax-redirs = 0\nmax-time = {HTTP_FRIST_S}\nconnect-timeout = 10\nproto = \"={}\"\nwrite-out = \"\\n%{{http_code}}\"\n",
        maskiert(&a.url),
        maskiert(methode),
        maskiert(&format!("{}:{}:{ip}", ziel.rechner, ziel.port)),
        if ziel.https { "https" } else { "http" },
    );
    for (n, w) in &a.kopf {
        let gut = !n.is_empty() && n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') && !w.chars().any(char::is_control);
        if !gut {
            return Err("eine Kopfzeile, die keine ist".into());
        }
        k.push_str(&format!("header = {}\n", maskiert(&format!("{n}: {w}"))));
    }
    if let Some(b) = &a.koerper {
        k.push_str(&format!("data-raw = {}\n", maskiert(b)));
    }
    Ok(k)
}

/// Der Zugang eines Laufs zum Netz.
struct Netz {
    erlaubt: Vec<String>,
    minute: (Instant, usize),
}

impl Netz {
    fn neu(erlaubt: Vec<String>) -> Self {
        Self { erlaubt: erlaubt.into_iter().map(|e| e.to_ascii_lowercase()).collect(), minute: (Instant::now(), 0) }
    }

    /// Prueft Rechner und Adresse; gibt das Ziel und die feste IP zurueck.
    fn pruefen(&self, url: &str) -> Result<(Adresse, IpAddr), String> {
        let ziel = adresse_lesen(url)?;
        if !self.erlaubt.contains(&ziel.eintrag) && !(ziel.port == 443 && ziel.https && self.erlaubt.contains(&ziel.rechner)) {
            return Err(format!("nicht genehmigt: {}", ziel.eintrag));
        }
        let lokal = ist_lokaler_name(&ziel.rechner);
        if !ziel.https && !lokal {
            return Err("nur https, ausser auf diesem Rechner".into());
        }
        let adressen: Vec<IpAddr> = (ziel.rechner.as_str(), ziel.port)
            .to_socket_addrs()
            .map_err(|f| format!("{}: {f}", ziel.rechner))?
            .map(|a| a.ip())
            .collect();
        let ip = ziel_ip(&ziel.rechner, lokal, &adressen)?;
        Ok((ziel, ip))
    }

    fn anfragen(&mut self, a: &Httpanfrage) -> Result<Httpantwort, String> {
        if self.minute.0.elapsed().as_secs() >= 60 {
            self.minute = (Instant::now(), 0);
        }
        if self.minute.1 >= HTTP_JE_MINUTE {
            return Err(format!("mehr als {HTTP_JE_MINUTE} Anfragen in einer Minute"));
        }
        self.minute.1 += 1;
        let (ziel, ip) = self.pruefen(&a.url)?;
        let k = konfiguration(a, &ziel, ip)?;
        let mut kind = Command::new("curl")
            .args(["--config", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|f| format!("curl: {f}"))?;
        let mut ein = kind.stdin.take().ok_or("keine Eingabe")?;
        let schreiber = std::thread::spawn(move || {
            let _ = ein.write_all(k.as_bytes());
        });
        let mut aus = Vec::new();
        let gelesen = kind
            .stdout
            .take()
            .ok_or("keine Ausgabe")?
            .take(HTTP_ANTWORT_HOECHSTENS as u64 + 16)
            .read_to_end(&mut aus);
        let zu_gross = aus.len() > HTTP_ANTWORT_HOECHSTENS;
        if zu_gross {
            let _ = kind.kill();
        }
        let mut fehler = String::new();
        if let Some(mut e) = kind.stderr.take() {
            let _ = e.by_ref().take(4_096).read_to_string(&mut fehler);
        }
        let status = kind.wait().map_err(|f| f.to_string())?;
        let _ = schreiber.join();
        gelesen.map_err(|f| f.to_string())?;
        if zu_gross {
            return Err(format!("Antwort groesser als {HTTP_ANTWORT_HOECHSTENS} Bytes"));
        }
        if !status.success() {
            return Err(crate::filter::eine_zeile(fehler.trim()));
        }
        let trenn = aus.iter().rposition(|b| *b == b'\n').ok_or("Antwort ohne Status")?;
        let code: u16 = String::from_utf8_lossy(&aus[trenn + 1..]).trim().parse().map_err(|_| "Antwort ohne Status".to_string())?;
        aus.truncate(trenn);
        Ok(Httpantwort { status: code, koerper: String::from_utf8_lossy(&aus).into_owned() })
    }
}

/// Der Datenordner eines Moduls.
struct Daten {
    ordner: PathBuf,
}

impl Daten {
    /// ⛔️ **Kein Teil des Weges darf eine Verknuepfung sein**: Der Name ist
    /// sauber, aber was auf der Platte liegt, koennte hinausfuehren.
    fn pfad(&self, name: &str) -> Result<PathBuf, String> {
        if name.len() > 200 || !sicherer_pfad(name) || name.split('/').any(|t| t.starts_with('.')) {
            return Err(format!("kein Dateiname: {}", crate::filter::eine_zeile(name)));
        }
        let mut p = self.ordner.clone();
        for teil in name.split('/') {
            p.push(teil);
            if std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err("eine Verknuepfung im Weg".into());
            }
        }
        Ok(p)
    }

    fn lesen(&self, name: &str) -> Result<Vec<u8>, String> {
        let p = self.pfad(name)?;
        let m = std::fs::metadata(&p).map_err(|_| format!("keine Datei: {name}"))?;
        if m.len() > DATEI_HOECHSTENS as u64 {
            return Err("zu gross".into());
        }
        std::fs::read(&p).map_err(|_| format!("keine Datei: {name}"))
    }

    fn belegt(ordner: &Path) -> u64 {
        std::fs::read_dir(ordner)
            .map(|d| {
                d.flatten()
                    .map(|e| match e.file_type() {
                        Ok(t) if t.is_dir() => Self::belegt(&e.path()),
                        Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
                        _ => 0,
                    })
                    .sum()
            })
            .unwrap_or(0)
    }

    fn schreiben(&self, name: &str, daten: &[u8]) -> Result<(), String> {
        if daten.len() > DATEI_HOECHSTENS {
            return Err("zu gross".into());
        }
        let p = self.pfad(name)?;
        let alt = std::fs::metadata(&p).map_or(0, |m| m.len());
        if Self::belegt(&self.ordner).saturating_sub(alt) + daten.len() as u64 > DATEN_HOECHSTENS {
            return Err(format!("der Datenordner ist voll ({DATEN_HOECHSTENS} Bytes)"));
        }
        if let Some(o) = p.parent() {
            std::fs::create_dir_all(o).map_err(|_| "Ordner nicht anlegbar".to_string())?;
        }
        schreiben_atomar(&p, daten, false).map_err(|_| format!("nicht schreibbar: {name}"))
    }
}

/// Was der Wirt je Lauf haelt.
struct Gast {
    grenzen: StoreLimits,
    aus: mpsc::Sender<Ereignis>,
    wecker: Option<fn()>,
    abbruch: Arc<AtomicBool>,
    ergebnis: Vec<u8>,
    ausgaben: usize,
    ausgabe_bytes: usize,
    protokollzeilen: usize,
    netz: Netz,
    daten: Daten,
    geheimnisse: PathBuf,
}

fn speicher(c: &Caller<'_, Gast>) -> Result<Memory, Error> {
    match c.get_export("memory") {
        Some(Extern::Memory(m)) => Ok(m),
        _ => Err(Error::new("das Modul exportiert keinen Speicher")),
    }
}

/// Liest aus dem Speicher des Moduls. ⚑ Zeiger und Laenge sind fremde Werte;
/// was ausserhalb liegt oder zu lang ist, beendet das Modul.
fn lesen(c: &Caller<'_, Gast>, p: i32, n: i32, grenze: usize) -> Result<Vec<u8>, Error> {
    let (p, n) = (p as u32 as usize, n as u32 as usize);
    if n > grenze {
        return Err(Error::new("zu lang fuer diese Funktion"));
    }
    let mut v = vec![0u8; n];
    speicher(c)?.read(c, p, &mut v).map_err(|_| Error::new("Zeiger ausserhalb des Speichers"))?;
    Ok(v)
}

fn lesen_text(c: &Caller<'_, Gast>, p: i32, n: i32, grenze: usize) -> Result<String, Error> {
    String::from_utf8(lesen(c, p, n, grenze)?).map_err(|_| Error::new("kein UTF-8"))
}

/// Legt ein Ergebnis zum Abholen bereit: `n >= 0` Bytes, sonst `-1 - n` Bytes
/// Fehlertext.
fn antwort(c: &mut Caller<'_, Gast>, r: Result<Vec<u8>, String>) -> i64 {
    match r {
        Ok(b) => {
            let n = b.len() as i64;
            c.data_mut().ergebnis = b;
            n
        }
        Err(t) => {
            let b = t.into_bytes();
            let n = b.len() as i64;
            c.data_mut().ergebnis = b;
            -1 - n
        }
    }
}

fn wach(c: &Caller<'_, Gast>) -> Result<(), Error> {
    if c.data().abbruch.load(Ordering::SeqCst) {
        return Err(Error::new("abgebrochen"));
    }
    Ok(())
}

fn funktionen(l: &mut Linker<Gast>) -> Result<(), Error> {
    l.func_wrap("myl", "ausgabe", |c: Caller<'_, Gast>, p: i32, n: i32| -> Result<(), Error> {
        wach(&c)?;
        let z = lesen_text(&c, p, n, nachricht::ZEILE_HOECHSTENS)?;
        let mut c = c;
        let d = c.data_mut();
        d.ausgaben += 1;
        d.ausgabe_bytes += z.len();
        if d.ausgaben > AUSGABEN_JE_EREIGNIS || d.ausgabe_bytes > AUSGABE_BYTES_JE_EREIGNIS {
            return Err(Error::new("zu viel Ausgabe in einem Ereignis"));
        }
        let e = match nachricht::lesen::<VomModul>(&z) {
            Ok(n) => Ereignis::Nachricht(n),
            Err(f) => Ereignis::Abgelehnt(f),
        };
        let _ = d.aus.send(e);
        if let Some(w) = d.wecker {
            w();
        }
        Ok(())
    })?;
    l.func_wrap("myl", "protokoll", |c: Caller<'_, Gast>, p: i32, n: i32| -> Result<(), Error> {
        wach(&c)?;
        let z = lesen_text(&c, p, n, 64 << 10)?;
        let mut c = c;
        let d = c.data_mut();
        d.protokollzeilen += 1;
        if d.protokollzeilen <= PROTOKOLLZEILEN {
            let _ = d.aus.send(Ereignis::Fehlerausgabe(crate::filter::eine_zeile(&z)));
        }
        Ok(())
    })?;
    l.func_wrap("myl", "http", |c: Caller<'_, Gast>, p: i32, n: i32| -> Result<i64, Error> {
        wach(&c)?;
        let z = lesen_text(&c, p, n, HTTP_ANFRAGE_HOECHSTENS)?;
        let mut c = c;
        let r = nachricht::lesen::<Httpanfrage>(&z).and_then(|a| c.data_mut().netz.anfragen(&a));
        let r = r.map(|a| nachricht::zeile(&a).into_bytes());
        Ok(antwort(&mut c, r))
    })?;
    l.func_wrap("myl", "datei_lesen", |c: Caller<'_, Gast>, p: i32, n: i32| -> Result<i64, Error> {
        wach(&c)?;
        let name = lesen_text(&c, p, n, 1 << 10)?;
        let mut c = c;
        let r = c.data().daten.lesen(&name);
        Ok(antwort(&mut c, r))
    })?;
    l.func_wrap("myl", "datei_schreiben", |c: Caller<'_, Gast>, p: i32, n: i32, q: i32, m: i32| -> Result<i64, Error> {
        wach(&c)?;
        let name = lesen_text(&c, p, n, 1 << 10)?;
        let inhalt = lesen(&c, q, m, DATEI_HOECHSTENS)?;
        let mut c = c;
        let r = c.data().daten.schreiben(&name, &inhalt).map(|_| Vec::new());
        Ok(antwort(&mut c, r))
    })?;
    l.func_wrap("myl", "geheimnis", |c: Caller<'_, Gast>, p: i32, n: i32| -> Result<i64, Error> {
        wach(&c)?;
        let name = lesen_text(&c, p, n, 128)?;
        let mut c = c;
        let r = if ist_schluesselname(&name) {
            std::fs::read(c.data().geheimnisse.join(&name)).map_err(|_| format!("kein Geheimnis: {name}"))
        } else {
            Err("kein Name fuer ein Geheimnis".to_string())
        };
        Ok(antwort(&mut c, r))
    })?;
    l.func_wrap("myl", "ergebnis", |mut c: Caller<'_, Gast>, p: i32, n: i32| -> Result<(), Error> {
        let (p, n) = (p as u32 as usize, n as u32 as usize);
        let b = std::mem::take(&mut c.data_mut().ergebnis);
        if n != b.len() {
            return Err(Error::new("Laenge passt nicht zum Ergebnis"));
        }
        speicher(&c)?.write(&mut c, p, &b).map_err(|_| Error::new("Zeiger ausserhalb des Speichers"))
    })?;
    l.func_wrap("myl", "jetzt_ms", || -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
    })?;
    Ok(())
}

/// Der Griff der Konsole an einem laufenden abgeschotteten Modul.
pub struct Griff {
    abbruch: Arc<AtomicBool>,
    faden: std::thread::JoinHandle<()>,
}

impl Griff {
    /// Ob der Faden noch rechnet.
    pub fn laeuft(&self) -> bool {
        !self.faden.is_finished()
    }

    /// ⚑ **Kein Anhalten, sondern ein Merker**: Die naechste Wirtsfunktion
    /// bricht ab, und der Treibstoff begrenzt, was bis dahin laeuft.
    pub fn abbrechen(&self) {
        self.abbruch.store(true, Ordering::SeqCst);
    }
}

/// ⛔️ **Was ein abgeschottetes Modul nicht darf**, geprueft vor dem Start: Eine
/// Werkzeugkiste und ein Agentenlauf starten gewoehnliche Programme mit den
/// Rechten des Nutzers, und das hiesse, die Abschottung selbst zu oeffnen.
pub fn zugelassen(g: &Geprueft) -> Result<(), String> {
    let b = &g.beschreibung;
    if b.befugnisse.agent || b.kiste.is_some() {
        return Err(format!("{}: ein abgeschottetes Modul kann noch keine Werkzeugkiste und keinen Agentenlauf haben", b.name));
    }
    if !matches!(b.laufzeit, Laufzeit::Wasm { .. }) {
        return Err(format!("{}: kein WebAssembly", b.name));
    }
    if g.programm.len() > MODUL_HOECHSTENS {
        return Err(format!("{}: groesser als {MODUL_HOECHSTENS} Bytes", b.name));
    }
    Ok(())
}

/// Der Lauf im eigenen Faden; endet mit einem [`Ereignis::Beendet`].
fn fahren(wasm: Vec<u8>, gast: Gast, an: mpsc::Receiver<String>) {
    let aus = gast.aus.clone();
    let wecker = gast.wecker;
    let melde = move |grund: String| {
        let _ = aus.send(Ereignis::Beendet(grund));
        if let Some(w) = wecker {
            w();
        }
    };
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = match Module::new(&engine, &wasm) {
        Ok(m) => m,
        Err(f) => return melde(format!("kein gueltiges WebAssembly: {f}")),
    };
    let mut linker = Linker::<Gast>::new(&engine);
    if let Err(f) = funktionen(&mut linker) {
        return melde(f.to_string());
    }
    let mut store = Store::new(&engine, gast);
    store.limiter(|g| &mut g.grenzen);
    if let Err(f) = store.set_fuel(TREIBSTOFF_JE_EREIGNIS) {
        return melde(f.to_string());
    }
    let instanz = match linker.instantiate_and_start(&mut store, &module) {
        Ok(i) => i,
        Err(f) => return melde(format!("startet nicht: {f}")),
    };
    let Some(mem) = instanz.get_memory(&store, "memory") else {
        return melde("das Modul exportiert keinen Speicher".into());
    };
    let platz = match instanz.get_typed_func::<i32, i32>(&store, "myl_speicher") {
        Ok(f) => f,
        Err(f) => return melde(format!("myl_speicher fehlt: {f}")),
    };
    let ereignis = match instanz.get_typed_func::<(i32, i32), ()>(&store, "myl_ereignis") {
        Ok(f) => f,
        Err(f) => return melde(format!("myl_ereignis fehlt: {f}")),
    };
    for zeile in an {
        if store.data().abbruch.load(Ordering::SeqCst) {
            return melde("abgebrochen".into());
        }
        let ende = matches!(nachricht::lesen::<AnModul>(&zeile), Ok(AnModul::Ende));
        {
            let g = store.data_mut();
            g.ausgaben = 0;
            g.ausgabe_bytes = 0;
        }
        if let Err(f) = store.set_fuel(TREIBSTOFF_JE_EREIGNIS) {
            return melde(f.to_string());
        }
        let n = zeile.len() as i32;
        let lauf = platz
            .call(&mut store, n)
            .and_then(|p| {
                mem.write(&mut store, p as u32 as usize, zeile.as_bytes()).map_err(|_| Error::new("Zeiger ausserhalb des Speichers"))?;
                Ok(p)
            })
            .and_then(|p| ereignis.call(&mut store, (p, n)));
        if let Err(f) = lauf {
            return melde(format!("beendet: {f}"));
        }
        if ende {
            return melde("Ende".into());
        }
    }
    melde("Ende".into());
}

/// **Startet ein gepruefes Modul in einem eigenen Faden.** Ausgefuehrt werden
/// genau die gepruefen Bytes aus dem Speicher.
pub fn starten(
    g: &Geprueft,
    ablage: &Path,
    wecker: Option<fn()>,
    an: mpsc::Receiver<String>,
    aus: mpsc::Sender<Ereignis>,
) -> Result<Griff, String> {
    zugelassen(g)?;
    let name = &g.beschreibung.name;
    let ort = modulort(ablage, name);
    let daten = ort.join("daten");
    std::fs::create_dir_all(&daten).map_err(|f| format!("{}: {f}", daten.display()))?;
    let freigabe = genehmigt(ablage, name);
    let erlaubt = g.beschreibung.befugnisse.http.iter().filter(|h| freigabe.contains(h)).cloned().collect();
    let abbruch = Arc::new(AtomicBool::new(false));
    let grenzen = StoreLimitsBuilder::new()
        .memory_size(SPEICHER_HOECHSTENS)
        .memories(1)
        .tables(4)
        .table_elements(100_000)
        .instances(1)
        .trap_on_grow_failure(true)
        .build();
    let gast = Gast {
        grenzen,
        aus,
        wecker,
        abbruch: abbruch.clone(),
        ergebnis: Vec::new(),
        ausgaben: 0,
        ausgabe_bytes: 0,
        protokollzeilen: 0,
        netz: Netz::neu(erlaubt),
        daten: Daten { ordner: daten },
        geheimnisse: ort.join("geheimnisse"),
    };
    let wasm = g.programm.clone();
    let faden = std::thread::Builder::new()
        .name(format!("modul-{name}"))
        .spawn(move || fahren(wasm, gast, an))
        .map_err(|f| format!("{name}: {f}"))?;
    Ok(Griff { abbruch, faden })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ablage::{genehmigen, geheimnis_setzen};
    use crate::beschreibung::Beschreibung;
    use crate::wirt::{Lauf, Start};
    use std::time::Duration;

    /// Ein Ordner fuer eine Probe, der mit ihr verschwindet. ⚠️ Bis zum
    /// 2026-11-13 blieb jeder liegen, je Lauf und Prozess einer mehr.
    struct Ordner(PathBuf);

    impl std::ops::Deref for Ordner {
        type Target = PathBuf;
        fn deref(&self) -> &PathBuf {
            &self.0
        }
    }

    impl AsRef<std::path::Path> for Ordner {
        fn as_ref(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for Ordner {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn ordner(n: &str) -> Ordner {
        let d = std::env::temp_dir().join(format!("myl-wasm-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        Ordner(d)
    }

    fn beschreibung(befugnisse: &str) -> Beschreibung {
        serde_json::from_str(&format!(
            r#"{{"name":"probe","version":"0.1.0","protokoll":1,"was":"Probe","laufzeit":{{"art":"wasm","datei":"m.wasm"}},
                "befehle":[],"befugnisse":{{{befugnisse}}},"dateien":{{}}}}"#
        ))
        .expect("Beschreibung")
    }

    fn geprueft(wasm: Vec<u8>, befugnisse: &str) -> Geprueft {
        Geprueft {
            ordner: std::env::temp_dir(),
            beschreibung: beschreibung(befugnisse),
            signiert_von: "Probe".into(),
            programm_hash: "0".repeat(64),
            programm: wasm,
        }
    }

    /// Ein Text als Datenabschnitt in WAT.
    fn daten(s: &str) -> String {
        s.bytes().map(|b| format!("\\{b:02x}")).collect()
    }

    fn hallo() -> String {
        nachricht::zeile(&VomModul::Hallo { protokoll: crate::PROTOKOLL, name: "probe".into(), version: "0.1.0".into() })
    }

    /// Ein Gast, der auf das erste Ereignis mit dem Handschlag antwortet und
    /// ab dem zweiten `$danach` ausfuehrt.
    fn gast(danach: &str, extra: &str) -> Vec<u8> {
        let h = hallo();
        let wat = format!(
            r#"(module
              (import "myl" "ausgabe" (func $aus (param i32 i32)))
              {extra}
              (memory (export "memory") 2)
              (global $mal (mut i32) (i32.const 0))
              (data (i32.const 0) "{d}")
              (func (export "myl_speicher") (param i32) (result i32) (i32.const 8192))
              (func (export "myl_ereignis") (param $p i32) (param $n i32)
                (if (i32.eqz (global.get $mal))
                  (then (global.set $mal (i32.const 1)) (call $aus (i32.const 0) (i32.const {n})))
                  (else {danach}))))"#,
            d = daten(&h),
            n = h.len()
        );
        wat::parse_str(wat).expect("WAT")
    }

    fn start(ablage: &Path) -> Start<'_> {
        Start { konsole: "probe", sprache: "de", modell: None, breite: 80, hoehe: 24, speicher: ablage, ablage, wecker: None, ansicht: false }
    }

    /// Wartet auf das Ende und gibt den Grund zurueck.
    fn ende(l: &mut Lauf, frist: Duration) -> String {
        let bis = Instant::now() + frist;
        while Instant::now() < bis {
            for e in l.abholen() {
                if let Ereignis::Beendet(g) = e {
                    return g;
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("kein Ende in {frist:?}");
    }

    #[test]
    fn handschlag_und_geordnetes_ende() {
        let d = ordner("ende");
        let mut l = Lauf::starten(&geprueft(gast("(nop)", ""), ""), &start(&d)).expect("startet");
        assert!(l.laeuft());
        l.senden(&AnModul::Ende);
        assert_eq!(ende(&mut l, Duration::from_secs(5)), "Ende");
        std::thread::sleep(Duration::from_millis(50));
        assert!(!l.laeuft());
    }

    /// ⛔️ **Eine Endlosschleife endet am Treibstoff**, und die Konsole wartet nicht.
    #[test]
    fn eine_endlosschleife_endet_am_treibstoff() {
        let d = ordner("schleife");
        let mut l = Lauf::starten(&geprueft(gast("(loop $l (br $l))", ""), ""), &start(&d)).expect("startet");
        let t = Instant::now();
        l.senden(&AnModul::Takt);
        assert!(t.elapsed() < Duration::from_millis(100), "die Konsole hat gewartet");
        let g = ende(&mut l, Duration::from_secs(20));
        assert!(g.contains("fuel"), "{g}");
    }

    /// ⛔️ **Mehr Speicher als erlaubt** beendet das Modul, schon beim Start und im Lauf.
    #[test]
    fn die_speichergrenze_greift() {
        let d = ordner("speicher");
        let zu_gross = wat::parse_str(r#"(module (memory (export "memory") 2000))"#).unwrap();
        let f = Lauf::starten(&geprueft(zu_gross, ""), &start(&d)).err().expect("darf nicht starten");
        assert!(f.contains("startet nicht") || f.contains("beendet vor"), "{f}");
        let mut l = Lauf::starten(&geprueft(gast("(drop (memory.grow (i32.const 2000)))", ""), ""), &start(&d)).expect("startet");
        l.senden(&AnModul::Takt);
        let g = ende(&mut l, Duration::from_secs(5));
        assert!(g.contains("beendet"), "{g}");
    }

    /// ⛔️ **Fremde Zeiger und Laengen** beenden das Modul, nicht die Konsole.
    #[test]
    fn fremde_zeiger_beenden_nur_das_modul() {
        for (name, ruf) in [
            ("ausserhalb", "(call $aus (i32.const 2000000) (i32.const 10))"),
            ("negativ", "(call $aus (i32.const -1) (i32.const 10))"),
            ("zu_lang", "(call $aus (i32.const 0) (i32.const -1))"),
        ] {
            let d = ordner(name);
            let mut l = Lauf::starten(&geprueft(gast(ruf, ""), ""), &start(&d)).expect("startet");
            l.senden(&AnModul::Takt);
            let g = ende(&mut l, Duration::from_secs(5));
            assert!(g.contains("beendet"), "{name}: {g}");
        }
    }

    /// ⛔️ **Wer unbekannte Funktionen braucht, startet nicht**: Es gibt kein WASI.
    #[test]
    fn ohne_wasi_startet_nichts() {
        let d = ordner("wasi");
        let w = wat::parse_str(r#"(module (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32))) (memory (export "memory") 1))"#).unwrap();
        let f = Lauf::starten(&geprueft(w, ""), &start(&d)).err().expect("darf nicht starten");
        assert!(f.contains("startet nicht") || f.contains("beendet vor"), "{f}");
    }

    #[test]
    fn zu_viel_ausgabe_in_einem_ereignis_endet() {
        let d = ordner("flut");
        let schleife = format!("(loop $l (call $aus (i32.const 0) (i32.const {})) (br $l))", hallo().len());
        let mut l = Lauf::starten(&geprueft(gast(&schleife, ""), ""), &start(&d)).expect("startet");
        l.senden(&AnModul::Takt);
        let g = ende(&mut l, Duration::from_secs(10));
        assert!(g.contains("zu viel Ausgabe"), "{g}");
    }

    /// ⛔️ **Werkzeuge starten Programme**; das gibt es abgeschottet nicht.
    #[test]
    fn werkzeuge_und_agent_sind_abgeschottet_nicht_zugelassen() {
        let g = geprueft(gast("(nop)", ""), r#""agent":true"#);
        assert!(zugelassen(&g).unwrap_err().contains("Agentenlauf"));
        let mut g = geprueft(gast("(nop)", ""), "");
        g.beschreibung.kiste = Some("kiste".into());
        assert!(zugelassen(&g).is_err());
        let mut g = geprueft(vec![0; MODUL_HOECHSTENS + 1], "");
        assert!(zugelassen(&g).unwrap_err().contains("groesser"));
        g.programm = vec![0; 8];
        assert!(zugelassen(&g).is_ok(), "nur die Groesse war das Problem");
    }

    #[test]
    fn adressen_werden_streng_gelesen() {
        let a = adresse_lesen("https://API.example.com/pfad?x=1").unwrap();
        assert_eq!((a.https, a.rechner.as_str(), a.port, a.eintrag.as_str()), (true, "api.example.com", 443, "api.example.com"));
        assert_eq!(adresse_lesen("https://example.com:8443/").unwrap().port, 8443);
        for falsch in [
            "ftp://example.com/",
            "https://user@example.com/",
            "https://example.com@boese.example/",
            "https:///x",
            "https://exa mple.com/",
            "https://example.com\\@boese.example/",
            "https://[::1]/",
            "https://example.com:abc/",
            "https://example.com\n/",
        ] {
            assert!(adresse_lesen(falsch).is_err(), "{falsch}");
        }
    }

    #[test]
    fn private_adressen_sind_nicht_oeffentlich() {
        for a in ["127.0.0.1", "10.1.2.3", "172.16.0.1", "192.168.1.1", "169.254.169.254", "100.64.0.1", "0.0.0.0", "198.18.0.1", "240.0.0.1", "224.0.0.1", "::1", "fe80::1", "fd00::1", "::ffff:127.0.0.1", "::ffff:10.0.0.1"] {
            assert!(!oeffentlich(a.parse().unwrap()), "{a}");
        }
        for a in ["1.1.1.1", "93.184.216.34", "2606:4700:4700::1111"] {
            assert!(oeffentlich(a.parse().unwrap()), "{a}");
        }
    }

    /// ⛔️ **Nur, was genannt und genehmigt ist.**
    #[test]
    fn nur_genehmigte_rechner() {
        let n = Netz::neu(vec!["localhost:8080".into(), "api.example.com".into()]);
        assert!(n.pruefen("https://boese.example/").unwrap_err().contains("nicht genehmigt"));
        assert!(n.pruefen("https://api.example.com.boese.example/").unwrap_err().contains("nicht genehmigt"));
        assert!(n.pruefen("http://api.example.com/").unwrap_err().contains("nur https"));
        assert!(n.pruefen("https://api.example.com:8443/").unwrap_err().contains("nicht genehmigt"));
        assert!(n.pruefen("http://localhost:8080/").is_ok());
        let unverschluesselt = Netz::neu(vec!["api.example.com:80".into()]);
        assert!(unverschluesselt.pruefen("http://api.example.com:80/").unwrap_err().contains("nur https"));
        let leer = Netz::neu(vec![]);
        assert!(leer.pruefen("http://localhost:8080/").is_err());
    }

    /// ⛔️ **Ein genehmigter Name, der auf dieses Netz zeigt, wird abgewiesen**,
    /// auch wenn nur eine seiner Adressen das tut.
    #[test]
    fn ein_umgebogener_name_wird_abgewiesen() {
        let frei: IpAddr = "1.1.1.1".parse().unwrap();
        let innen: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(ziel_ip("a.example", false, &[frei]), Ok(frei));
        assert!(ziel_ip("a.example", false, &[innen]).unwrap_err().contains("private Adresse"));
        assert!(ziel_ip("a.example", false, &[frei, innen]).unwrap_err().contains("private Adresse"));
        assert_eq!(ziel_ip("localhost", true, &[innen]), Ok(innen), "nur wer localhost selbst genehmigt hat");
        assert!(ziel_ip("a.example", false, &[]).is_err());
    }

    #[test]
    fn die_konfiguration_laesst_sich_nicht_unterwandern() {
        let z = adresse_lesen("https://api.example.com/").unwrap();
        let a = Httpanfrage { url: "https://api.example.com/".into(), methode: None, kopf: vec![("X-Key".into(), "ge\"heim".into())], koerper: Some("a\"\nurl = \"https://boese.example/".into()) };
        let k = konfiguration(&a, &z, "1.1.1.1".parse().unwrap()).unwrap();
        assert_eq!(k.lines().filter(|l| l.starts_with("url")).count(), 1, "{k}");
        assert!(k.contains("max-redirs = 0") && k.contains(r#"proto = "=https""#) && k.contains("api.example.com:443:1.1.1.1"), "{k}");
        let mut b = a.clone();
        b.kopf = vec![("X".into(), "a\r\nHost: boese".into())];
        assert!(konfiguration(&b, &z, "1.1.1.1".parse().unwrap()).is_err());
        b.kopf = vec![("X Y".into(), "a".into())];
        assert!(konfiguration(&b, &z, "1.1.1.1".parse().unwrap()).is_err());
        b.kopf = vec![];
        b.methode = Some("CONNECT".into());
        assert!(konfiguration(&b, &z, "1.1.1.1".parse().unwrap()).is_err());
    }

    #[test]
    fn dateien_bleiben_im_eigenen_ordner() {
        let d = ordner("daten");
        let daten = Daten { ordner: d.clone() };
        daten.schreiben("unter/a.txt", b"inhalt").unwrap();
        assert_eq!(daten.lesen("unter/a.txt").unwrap(), b"inhalt");
        for falsch in ["../a", "/etc/passwd", "a/../../b", ".versteckt", "unter/.x", "a\\b", ""] {
            assert!(daten.pfad(falsch).is_err(), "{falsch:?}");
        }
        #[cfg(unix)]
        {
            let draussen = ordner("draussen");
            std::fs::write(draussen.join("x"), b"geheim").unwrap();
            std::os::unix::fs::symlink(&draussen, d.join("weg")).unwrap();
            assert!(daten.lesen("weg/x").unwrap_err().contains("Verknuepfung"));
            assert!(daten.schreiben("weg/y", b"z").unwrap_err().contains("Verknuepfung"));
            assert!(!draussen.join("y").exists());
        }
    }

    #[test]
    fn der_datenordner_hat_einen_deckel() {
        let d = ordner("deckel");
        let daten = Daten { ordner: d.clone() };
        assert!(daten.schreiben("gross", &vec![0; DATEI_HOECHSTENS + 1]).is_err());
        let f = std::fs::File::create(d.join("schon")).unwrap();
        f.set_len(DATEN_HOECHSTENS).unwrap();
        assert!(daten.schreiben("noch", b"x").unwrap_err().contains("voll"));
    }

    #[test]
    fn genehmigung_und_geheimnisse() {
        let d = ordner("gen");
        assert!(genehmigt(&d, "m").is_empty());
        genehmigen(&d, "m", &["a.example".into()]).unwrap();
        assert_eq!(genehmigt(&d, "m"), vec!["a.example"]);
        geheimnis_setzen(&d, "m", "key", b"wert").unwrap();
        assert_eq!(std::fs::read(d.join("m/geheimnisse/key")).unwrap(), b"wert");
        assert!(geheimnis_setzen(&d, "m", "../x", b"w").is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(d.join("m/geheimnisse/key")).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    /// ⚑ **Der Weg ueber `curl`, gegen einen oertlichen Server**: Antwort, Kopf,
    /// Koerper, keine Weiterleitung, Groessengrenze und Rate.
    #[test]
    fn ein_genehmigter_rechner_antwortet_und_leitet_nicht_weiter() {
        use std::io::BufRead;
        let horcher = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = horcher.local_addr().unwrap().port();
        let antworten: Vec<Vec<u8>> = vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhallo".to_vec(),
            format!("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/anders\r\nContent-Length: 3\r\nConnection: close\r\n\r\nweg").into_bytes(),
            {
                let mut v = format!("HTTP/1.1 200 OK\r\nContent-Length: {n}\r\nConnection: close\r\n\r\n", n = HTTP_ANTWORT_HOECHSTENS + 100).into_bytes();
                v.extend(vec![b'x'; HTTP_ANTWORT_HOECHSTENS + 100]);
                v
            },
        ];
        let server = std::thread::spawn(move || {
            let mut gesehen = Vec::new();
            for a in antworten {
                let (mut s, _) = horcher.accept().unwrap();
                let mut r = std::io::BufReader::new(s.try_clone().unwrap());
                let (mut kopf, mut laenge) = (String::new(), 0usize);
                loop {
                    let mut z = String::new();
                    r.read_line(&mut z).unwrap();
                    if let Some(w) = z.to_ascii_lowercase().strip_prefix("content-length:") {
                        laenge = w.trim().parse().unwrap();
                    }
                    let leer = z.trim().is_empty();
                    kopf.push_str(&z);
                    if leer {
                        break;
                    }
                }
                let mut k = vec![0; laenge];
                r.read_exact(&mut k).unwrap();
                gesehen.push(format!("{kopf}{}", String::from_utf8_lossy(&k)));
                let _ = s.write_all(&a);
            }
            gesehen
        });
        let mut n = Netz::neu(vec![format!("127.0.0.1:{port}")]);
        let url = format!("http://127.0.0.1:{port}/pfad");
        let a = Httpanfrage { url: url.clone(), methode: Some("POST".into()), kopf: vec![("X-Probe".into(), "wert".into())], koerper: Some("koerper".into()) };
        assert_eq!(n.anfragen(&a).unwrap(), Httpantwort { status: 200, koerper: "hallo".into() });
        let b = Httpanfrage { url: url.clone(), methode: None, kopf: vec![], koerper: None };
        assert_eq!(n.anfragen(&b).unwrap(), Httpantwort { status: 302, koerper: "weg".into() }, "die Weiterleitung wird nicht verfolgt");
        assert!(n.anfragen(&b).unwrap_err().contains("groesser"));
        let gesehen = server.join().unwrap();
        assert!(gesehen[0].starts_with("POST /pfad") && gesehen[0].contains("X-Probe: wert") && gesehen[0].ends_with("koerper"), "{}", gesehen[0]);
        n.minute.1 = HTTP_JE_MINUTE;
        assert!(n.anfragen(&b).unwrap_err().contains("in einer Minute"));
    }

    /// ⚑ **Der ganze Weg mit einem echten Gast**, gebaut aus
    /// `beispiele/wasm-gast`: Handschlag, Zeilen, Datei, Geheimnis, Absage ans Netz.
    #[test]
    fn ein_echter_gast_faehrt_den_ganzen_weg() {
        use sha2::{Digest, Sha256};
        let hier = Path::new(env!("CARGO_MANIFEST_DIR"));
        let quelle = std::fs::read(hier.join("beispiele/wasm-gast/src/lib.rs")).expect("Quelle");
        let vermerkt = std::fs::read_to_string(hier.join("tests/gast.wasm.quelle")).expect("Vermerk");
        assert_eq!(hex::encode(Sha256::digest(&quelle)), vermerkt.trim(), "gast.wasm ist aelter als seine Quelle; beispiele/wasm-gast/bauen.sh");
        let wasm = std::fs::read(hier.join("tests/gast.wasm")).expect("gast.wasm");
        let d = ordner("echt");
        geheimnis_setzen(&d, "beispiel", "schluessel", b"sesam").unwrap();
        let mut g = geprueft(wasm, r#""http":["example.com"]"#);
        g.beschreibung.name = "beispiel".into();
        let mut l = Lauf::starten(&g, &start(&d)).expect("startet");
        l.senden(&AnModul::Befehl { zeile: "/beispiel".into() });
        let mut text = String::new();
        let bis = Instant::now() + Duration::from_secs(10);
        while Instant::now() < bis && !text.contains("netz:") {
            for e in l.abholen() {
                match e {
                    Ereignis::Nachricht(VomModul::Zeilen { text: t, .. }) => text.push_str(&format!("{t}\n")),
                    Ereignis::Beendet(g) => panic!("{g}"),
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(text.contains("datei: gespeichert"), "{text}");
        assert!(text.contains("geheimnis: 5 Bytes"), "{text}");
        assert!(text.contains("netz: nicht genehmigt"), "{text}");
        assert!(text.contains("uhr: ja"), "{text}");
        assert_eq!(std::fs::read(d.join("beispiel/daten/notiz.txt")).unwrap(), b"gespeichert");
        l.senden(&AnModul::Ende);
        assert_eq!(ende(&mut l, Duration::from_secs(5)), "Ende");
    }
}
