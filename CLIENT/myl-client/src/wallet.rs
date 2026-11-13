//! **Testwallets**: Konten, deren Schluessel der Client haelt, und eines
//! davon ausgewaehlt (das eingeloggte). Seit dem 2026-10-26.
//!
//! # ⚑ Was ein Wallet hier ist
//!
//! Ein BLS-Schluessel in einer Datei und die Adresse dazu. Beim Mining traegt
//! der Knoten die Adresse des ausgewaehlten Wallets als **Auszahlungskonto**
//! ueber die Kette ein (`--konto`), und auf der Probekette praegt jede Epoche
//! dorthin. Der Knoten haelt den Schluessel des Wallets nicht: Er arbeitet
//! mit seinem eigenen Konsensschluessel, und nur fuer einen Wechsel bekommt
//! er den Schluessel des bisher eingetragenen Wallets (`--kontowechsel`),
//! denn die Kette nimmt einen Wechsel nur mit dessen Unterschrift an.
//!
//! # ⚑ Wie ein Wallet entsteht (Festlegung des Projektinhabers, 2026-10-26)
//!
//! 1. [`entwerfen`] wuerfelt 128 Bit und zeigt sie als **zwoelf Woerter nach
//!    BIP-39** (`crate::bip39`); [`aus_woertern`] stellt aus ihnen wieder her.
//!    Seed nach BIP-39, Schluessel nach EIP-2333 und EIP-2334: dieselben Woerter geben
//!    immer dasselbe Wallet.
//! 2. [`im_netz_suchen`] fragt einen Knoten, ob die Adresse auf der Kette
//!    schon Spuren hat, damit nichts doppelt angelegt wird; auf dieser
//!    Maschine verhindert [`speichern`] dasselbe Wallet unter zwei Namen.
//! 3. **Gespeichert wird erst nach doppelter Bestaetigung**: „notiert" und
//!    zwei abgefragte Woerter ([`Entwurf::abfrage`], [`Entwurf::bestaetigt`]).
//!    Wer abbricht, hat nichts angelegt.
//!
//! # ⚠️ Spielgeld, und warum das hier noch reicht
//!
//! Ohne Passwort liegt der Schluessel **unverschluesselt** mit Rechten `0600`
//! neben den Einstellungen; mit Passwort als Keystore nach EIP-2335
//! ([`crate::keystore`]), und dann liegt nur noch der Schluessel des Kontos
//! dort, nicht der Seed. Fuer MYL der Probekette genuegt beides. Vor echten
//! MYL fehlen der Schluesselbund des Systems, ein kaltes Auszahlungskonto ohne
//! Schluessel auf dieser Maschine und eine pruefbare Auskunft gegen die
//! Zustandswurzel.
//!
//! # ⚑ Dasselbe Dateiformat wie der Knoten
//!
//! Kopfzeile und Ausgangsmaterial in Hex: 64 Bytes Seed bei einem Wallet aus
//! Woertern, 32 Bytes bei einem aelteren. Der Knoten liest beides
//! (`myl_node::schluessel`), und nur so kann er den Schluessel eines Wallets
//! fuer einen Wechsel lesen.

use std::path::PathBuf;

use myl_types::adresstext;
use myl_types::bls::BlsSecretKey;
/// Die Adresse eines Wallets; herausgereicht, damit Oberflaechen ohne eigene
/// Abhaengigkeit auf `myl-types` mit ihr umgehen koennen.
pub use myl_types::ids::Address;

use crate::bip39;

/// Die Kopfzeile, Wort fuer Wort wie im Knoten.
const DATEIKOPF: &str = "# Myelith-Konsensschluessel (BLS12-381). GEHEIM. Nicht weitergeben.";
/// Welches Wallet ausgewaehlt ist (eine Zeile, der Name).
const GEWAEHLT: &str = "gewaehlt";
/// Welches Wallet die Kette zuletzt als Auszahlungskonto fuehrte.
const EINGETRAGEN: &str = "eingetragen";

/// Ein Wallet: Name und Adresse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wallet {
    pub name: String,
    pub adresse: Address,
}

impl Wallet {
    /// Die Adresse in Hex, wie `--konto` sie nimmt.
    pub fn adresse_hex(&self) -> String {
        hex(self.adresse.as_bytes())
    }

    /// ⚑ Die Adresse, wie ein Mensch sie sieht: mit Prüfsumme und Präfix.
    pub fn adresse_text(&self) -> String {
        adresstext::als_text(&self.adresse, adresstext::PRAEFIX_TESTNETZ)
    }

    /// Wo der Schluessel liegt, unverschluesselt oder als Keystore.
    pub fn schluesselpfad(&self) -> PathBuf {
        let k = keystorepfad(&self.name);
        if k.exists() {
            k
        } else {
            ordner().join(format!("{}.key", self.name))
        }
    }
}

/// **Ein Wallet, das es noch nicht gibt**: Woerter, Seed und Adresse, bis
/// es bestaetigt und gespeichert ist.
///
/// ⚑ `Debug` zeigt **nie** Woerter oder Seed: Ein abgeleitetes schriebe sie in
/// jede Fehlermeldung, die einen Entwurf ausgibt.
pub struct Entwurf {
    woerter: Vec<String>,
    seed: [u8; 64],
    pub adresse: Address,
}

impl std::fmt::Debug for Entwurf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entwurf").field("adresse", &self.adresse_hex()).finish_non_exhaustive()
    }
}

/// ⛔️ **Ein verworfener Entwurf hinterlaesst keine Woerter im Speicher**
/// (CLIENT 7.14): Seed und Woerter werden beim Fallenlassen ueberschrieben.
impl Drop for Entwurf {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.seed.zeroize();
        for w in &mut self.woerter {
            w.zeroize();
        }
    }
}

impl Entwurf {
    /// Die zwoelf Woerter, zum Notieren.
    pub fn woerter(&self) -> &[String] {
        &self.woerter
    }

    pub fn adresse_hex(&self) -> String {
        hex(self.adresse.as_bytes())
    }

    /// **Welche zwei Woerter abgefragt werden**, zufaellig und verschieden,
    /// gezaehlt ab 1 und aufsteigend.
    pub fn abfrage(&self) -> (usize, usize) {
        let mut z = [0u8; 2];
        let _ = getrandom::fill(&mut z);
        let a = z[0] as usize % bip39::WOERTER;
        let b = (a + 1 + z[1] as usize % (bip39::WOERTER - 1)) % bip39::WOERTER;
        (a.min(b) + 1, a.max(b) + 1)
    }

    /// Ob die abgefragten Woerter stimmen (Position ab 1); Gross- und
    /// Kleinschreibung und Leerraum zaehlen nicht.
    pub fn bestaetigt(&self, antworten: &[(usize, &str)]) -> bool {
        !antworten.is_empty()
            && antworten.iter().all(|(n, w)| {
                n.checked_sub(1).and_then(|i| self.woerter.get(i)).is_some_and(|x| *x == w.trim().to_lowercase())
            })
    }
}

/// **Wuerfelt ein neues Wallet**: 128 Bit Zufall, zwoelf Woerter.
pub fn entwerfen() -> Result<Entwurf, String> {
    let mut e = zeroize::Zeroizing::new([0u8; bip39::ENTROPIE_BYTES]);
    getrandom::fill(&mut e[..]).map_err(|f| format!("keine Zufallszahlen: {f}"))?;
    let woerter = zeroize::Zeroizing::new(bip39::woerter(&e).join(" "));
    aus_woertern(&woerter)
}

/// **Stellt ein Wallet aus zwoelf Woertern her**, mit Pruefung von Liste
/// und Pruefsumme.
pub fn aus_woertern(text: &str) -> Result<Entwurf, String> {
    let e = zeroize::Zeroizing::new(bip39::entropie(text)?);
    let woerter: Vec<String> = bip39::woerter(&e).iter().map(|w| w.to_string()).collect();
    let seed = bip39::seed(&zeroize::Zeroizing::new(woerter.join(" ")), "");
    let adresse = adresse_aus_ikm(&seed)?;
    Ok(Entwurf { woerter, seed, adresse })
}

/// Was ein Knoten ueber eine Adresse weiss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Netzbefund {
    /// Ohne Spur auf der Kette, auf dieser Hoehe.
    Unbekannt { hoehe: u64 },
    /// Mit Spur: Guthaben, Nonce, als Miner oder als Auszahlungskonto.
    Bekannt { hoehe: u64, guthaben: u64, nonce: u64, miner: bool, auszahlung_fuer: u64 },
    /// Nicht gefragt oder keine Antwort; der Grund.
    NichtGeprueft(String),
}

impl Netzbefund {
    /// Ein Satz fuer die Oberflaeche.
    pub fn satz(&self) -> String {
        match self {
            Netzbefund::Unbekannt { hoehe } => format!("Im Netz noch nicht vorhanden (geprüft auf Höhe {hoehe})."),
            Netzbefund::Bekannt { guthaben, nonce, miner, auszahlung_fuer, .. } => {
                let mut teile = vec![format!("{} MYL", crate::mining::myl_text(*guthaben))];
                if *nonce > 0 {
                    teile.push(format!("{nonce} Transaktionen"));
                }
                if *miner {
                    teile.push("als Miner angemeldet".into());
                }
                if *auszahlung_fuer > 0 {
                    teile.push(format!("Auszahlungskonto für {auszahlung_fuer} Miner"));
                }
                format!("Im Netz schon vorhanden: {}.", teile.join(", "))
            }
            Netzbefund::NichtGeprueft(grund) => format!("Nicht im Netz geprüft: {grund}"),
        }
    }
}

/// **Sucht die Adresse im Netz**: beim laufenden Knoten des Minings, sonst
/// beim Abfrageort des Einstiegs (`beobachtung …` in `einstieg.txt`).
///
/// ⚠️ Eine Auskunft eines Knotens, kein Beweis gegen die Zustandswurzel: Vor
/// echten MYL gehoert ein Merkle-Pfad zum Konto dazu, gegen einen bestaetigten
/// Blockkopf geprueft, und mehr als ein befragter Knoten.
pub fn im_netz_suchen(adresse: &Address) -> Netzbefund {
    let Some(ort) = crate::mining::abfrageort() else {
        return Netzbefund::NichtGeprueft(
            "kein Knoten erreichbar (Mining läuft nicht, und der Einstieg nennt keinen Abfrageort).".into(),
        );
    };
    match crate::mining::holen(&ort, &format!("/konto/{}", hex(adresse.as_bytes()))) {
        Ok(text) => befund_aus(&text, &ort),
        Err(f) => Netzbefund::NichtGeprueft(f),
    }
}

/// Eine Zeile des Verlaufs (`myl wallet verlauf`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verlaufszeile {
    pub hoehe: u64,
    pub art: String,
    pub ausgehend: bool,
    /// Die Gegenseite einer Überweisung, als Hex.
    pub gegen: String,
    pub betrag: u64,
}

/// **Was unter jedem Verlauf steht**, in `myl`, Konsole und Fenster gleich.
pub const VERLAUF_GRENZE: &str =
    "Aus den gehaltenen Blöcken des Knotens. Im Block heißt nicht gelungen; Prägungen stehen hier nicht.";

impl Verlaufszeile {
    /// **Die Zeile, wie `myl`, Konsole und Fenster sie zeigen**: Höhe, Art,
    /// Betrag und die Gegenseite als Adresstext.
    pub fn text(&self) -> String {
        use myl_types::adresstext::{aus_text, als_text, PRAEFIX_TESTNETZ};
        let gegen = aus_text(&self.gegen, PRAEFIX_TESTNETZ).map_or(String::new(), |a| {
            format!(" {} {}", if self.ausgehend { "an" } else { "von" }, als_text(&a, PRAEFIX_TESTNETZ))
        });
        let betrag = if self.betrag > 0 { format!(" {} MYL", crate::mining::myl_text(self.betrag)) } else { String::new() };
        format!("Höhe {:>8}  {:<13}{betrag}{gegen}", self.hoehe, self.art)
    }
}

/// **Wie viele Transaktionen ein Konto gesendet hat**, aus seiner Nonce.
///
/// ⚑ Ein Konto, das in Epoche `e` entsteht, beginnt bei `e · 2³²` (der
/// Nonce-Boden der Kette, gegen Wiederholung nach einem geleerten Konto);
/// jede Transaktion zaehlt eins hoch. 📌 In der Probe am Probenetz vom
/// 2026-11-12 stand fuer ein unbenutztes Konto „Nonce 4294967296“: richtig,
/// aber fuer niemanden lesbar. Gezeigt wird deshalb, was ueber dem Boden liegt.
pub fn gesendet(nonce: u64) -> u64 {
    nonce & 0xffff_ffff
}

/// Höchstens so viele Seiten holt `myl wallet verlauf`.
pub const VERLAUF_SEITEN: usize = 20;

/// **Der Verlauf einer Adresse** aus den gehaltenen Blöcken des erreichbaren
/// Knotens, neueste zuerst. ⚠️ Eine Auskunft wie `/konto`, und im Block heisst
/// nicht gelungen.
pub fn verlauf(adresse: &Address) -> Result<Vec<Verlaufszeile>, String> {
    let ort = crate::mining::abfrageort().ok_or("kein Knoten erreichbar (Mining läuft nicht, und der Einstieg nennt keinen Abfrageort).")?;
    let basis = format!("/verlauf/{}", hex(adresse.as_bytes()));
    let mut zeilen = Vec::new();
    let mut bis: Option<u64> = None;
    for _ in 0..VERLAUF_SEITEN {
        let weg = bis.map_or(basis.clone(), |h| format!("{basis}?bis={h}"));
        let (mut seite, weiter) = verlauf_aus(&seite_holen(&weg, |w| crate::mining::holen(&ort, w))?)?;
        zeilen.append(&mut seite);
        match weiter {
            Some(h) => bis = Some(h),
            None => break,
        }
        // Der Knoten liefert höchstens eine Seite je Takt.
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    Ok(zeilen)
}

/// So oft fragt der Client nach einer gedrosselten Seite, je
/// [`VERLAUF_WARTEN_MS`] auseinander.
const VERLAUF_VERSUCHE: usize = 5;
const VERLAUF_WARTEN_MS: u64 = 150;

/// **Eine Seite, auch wenn der Knoten gerade drosselt.** Er liefert hoechstens
/// eine Seite je 100 ms, an alle Fragenden zusammen, und antwortet sonst mit
/// 503. 📌 Gefunden am 2026-11-12 in der Probe von Ende zu Ende: Der Verlauf
/// eines zweiten Wallets direkt nach dem ersten endete mit der nackten Zeile
/// `HTTP/1.1 503 Service Unavailable`. Dasselbe trifft Konsole und Fenster am
/// selben Knoten.
fn seite_holen(weg: &str, holen: impl Fn(&str) -> Result<String, String>) -> Result<String, String> {
    let mut letzter = String::new();
    for versuch in 0..VERLAUF_VERSUCHE {
        if versuch > 0 {
            std::thread::sleep(std::time::Duration::from_millis(VERLAUF_WARTEN_MS));
        }
        match holen(weg) {
            Err(f) if f.starts_with("HTTP/1.1 503") => letzter = f,
            sonst => return sonst,
        }
    }
    Err(format!("der Knoten drosselt oder antwortet nicht ({letzter}); bitte gleich noch einmal"))
}

/// Eine Seite aus der Antwort von `/verlauf/…`.
fn verlauf_aus(text: &str) -> Result<(Vec<Verlaufszeile>, Option<u64>), String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("unverständliche Antwort: {e}"))?;
    let eintraege = v.get("eintraege").and_then(|e| e.as_array()).ok_or("unverständliche Antwort: keine Einträge")?;
    let zeilen = eintraege
        .iter()
        .map(|e| Verlaufszeile {
            hoehe: e["hoehe"].as_u64().unwrap_or(0),
            art: e["art"].as_str().unwrap_or("").to_string(),
            ausgehend: e["ausgehend"].as_bool().unwrap_or(false),
            gegen: e["gegen"].as_str().unwrap_or("").to_string(),
            betrag: e["betrag"].as_u64().unwrap_or(0),
        })
        .collect();
    Ok((zeilen, v["weiter_bei"].as_u64()))
}

/// Der Befund aus der Antwort von `/konto/…`.
fn befund_aus(text: &str, ort: &str) -> Netzbefund {
    let zahl = |name: &str| -> Option<u64> {
        let ab = text.find(&format!("\"{name}\":"))? + name.len() + 3;
        text[ab..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
    };
    let wahr = |name: &str| text.contains(&format!("\"{name}\":true"));
    let Some(hoehe) = zahl("hoehe") else {
        return Netzbefund::NichtGeprueft(format!("unverständliche Antwort von {ort}"));
    };
    if !wahr("bekannt") {
        return Netzbefund::Unbekannt { hoehe };
    }
    Netzbefund::Bekannt {
        hoehe,
        guthaben: zahl("guthaben").unwrap_or(0),
        nonce: zahl("nonce").unwrap_or(0),
        miner: wahr("miner"),
        auszahlung_fuer: zahl("auszahlung_fuer").unwrap_or(0),
    }
}

#[cfg(test)]
mod verlauf_tests {
    use super::*;

    #[test]
    fn gesendet_zaehlt_ueber_dem_boden() {
        assert_eq!(gesendet(0), 0);
        assert_eq!(gesendet(4_294_967_296), 0, "Epoche 1, nichts gesendet");
        assert_eq!(gesendet(3 * (1 << 32) + 5), 5);
    }

    /// ⛔️ Eine gedrosselte Seite wird nachgefragt; bleibt der Knoten dabei,
    /// steht ein Satz da und keine nackte Statuszeile. Ein anderer Fehler
    /// wird nicht wiederholt.
    #[test]
    fn eine_gedrosselte_seite_wird_nachgefragt() {
        let zaehler = std::cell::Cell::new(0);
        let zweimal_gedrosselt = |_: &str| {
            zaehler.set(zaehler.get() + 1);
            if zaehler.get() <= 2 { Err("HTTP/1.1 503 Service Unavailable".to_string()) } else { Ok("seite".to_string()) }
        };
        assert_eq!(seite_holen("/verlauf/x", zweimal_gedrosselt), Ok("seite".to_string()));
        assert_eq!(zaehler.get(), 3);

        let f = seite_holen("/verlauf/x", |_| Err("HTTP/1.1 503 Service Unavailable".to_string())).expect_err("bleibt gedrosselt");
        assert!(f.starts_with("der Knoten drosselt"), "{f}");

        zaehler.set(0);
        let anders = |_: &str| {
            zaehler.set(zaehler.get() + 1);
            Err::<String, String>("HTTP/1.1 404 Not Found".to_string())
        };
        assert!(seite_holen("/verlauf/x", anders).is_err());
        assert_eq!(zaehler.get(), 1, "ein anderer Fehler wird nicht wiederholt");
    }

    #[test]
    fn eine_seite_des_verlaufs_wird_gelesen() {
        let t = r#"{"weiter_bei":7,"eintraege":[{"hoehe":9,"art":"burn","ausgehend":true,"gegen":"","betrag":5}]}"#;
        let (z, w) = verlauf_aus(t).expect("lesbar");
        assert_eq!(w, Some(7));
        assert_eq!(z, vec![Verlaufszeile { hoehe: 9, art: "burn".into(), ausgehend: true, gegen: String::new(), betrag: 5 }]);
        assert_eq!(verlauf_aus(r#"{"weiter_bei":null,"eintraege":[]}"#).expect("leer").1, None);
        assert!(verlauf_aus("kein json").is_err(), "Gegenprobe: Unsinn");
    }

    /// ⚑ **Eine Zeile nennt Richtung und Gegenseite**, und ohne Betrag und
    /// Gegenseite steht keines von beiden da.
    #[test]
    fn die_zeile_nennt_richtung_und_gegenseite() {
        use myl_types::adresstext::{als_text, PRAEFIX_TESTNETZ};
        let gegen = myl_types::ids::Address::new([3; 32]);
        let hex: String = gegen.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        let mut z = Verlaufszeile { hoehe: 12, art: "ueberweisung".into(), ausgehend: true, gegen: hex, betrag: 2_000_000 };
        let an = z.text();
        assert!(an.starts_with("Höhe       12  ueberweisung "), "{an}");
        assert!(an.contains(&format!("{} MYL", crate::mining::myl_text(2_000_000))), "{an}");
        assert!(an.ends_with(&format!(" an {}", als_text(&gegen, PRAEFIX_TESTNETZ))), "{an}");
        z.ausgehend = false;
        assert!(z.text().contains(" von "), "eingehend");
        let leer = Verlaufszeile { hoehe: 1, art: "sitzung".into(), ausgehend: true, gegen: String::new(), betrag: 0 };
        assert!(!leer.text().contains("MYL") && !leer.text().contains(" an "), "{}", leer.text());
    }
}

/// Der Ordner der Wallets: neben den Einstellungen, oder wo `MYL_WALLETS`
/// hinzeigt (fuer Proben).
pub fn ordner() -> PathBuf {
    if let Some(p) = std::env::var_os("MYL_WALLETS").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let e = crate::Einstellungen::vorgabepfad();
    e.parent().map(|p| p.join("wallets")).unwrap_or_else(|| PathBuf::from("wallets"))
}

/// Ein Name taugt, wenn er als Dateiname nichts anrichten kann.
pub fn name_pruefen(name: &str) -> Result<(), String> {
    let gut = !name.is_empty()
        && name.len() <= 40
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if gut {
        Ok(())
    } else {
        Err(format!("„{name}“ taugt nicht als Name: Buchstaben, Ziffern, - und _, höchstens 40 Zeichen."))
    }
}

/// **Speichert einen bestaetigten Entwurf.** Ist noch keines ausgewaehlt,
/// wird es das.
///
/// ⛔️ **Dasselbe Wallet nicht zweimal**: Liegt seine Adresse hier schon
/// unter einem anderen Namen, wird nichts angelegt.
pub fn speichern(name: &str, entwurf: &Entwurf) -> Result<Wallet, String> {
    speichern_mit(name, entwurf, None)
}

/// [`speichern`], mit Passwort als Keystore.
pub fn speichern_mit(name: &str, entwurf: &Entwurf, passwort: Option<&str>) -> Result<Wallet, String> {
    name_pruefen(name)?;
    if let Some(da) = liste().into_iter().find(|w| w.adresse == entwurf.adresse) {
        return Err(format!("Dieses Wallet gibt es hier schon: „{}“.", da.name));
    }
    let d = ordner();
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    let pfad = d.join(format!("{name}.key"));
    if pfad.exists() || keystorepfad(name).exists() {
        return Err(format!("Ein Wallet „{name}“ gibt es schon."));
    }
    match passwort {
        // ⚑ Nie ueber eine Klartextdatei: Was einmal auf der Platte lag, ist
        //   mit dem Loeschen nicht sicher weg.
        Some(p) => keystore_schreiben(name, &BlsSecretKey::aus_ausgangsmaterial(&entwurf.seed).map_err(|_| "Schlüssel nicht ableitbar".to_string())?, p)?,
        None => {
            let text = zeroize::Zeroizing::new(format!("{DATEIKOPF}\n{}\n", hex(&entwurf.seed)));
            // ⚑ `0600` ab dem Anlegen, wie im Knoten (Fund 607).
            myl_types::geheimdatei::schreiben(&pfad, text.as_bytes()).map_err(|e| format!("{}: {e}", pfad.display()))?;
        }
    }
    let w = Wallet { name: name.to_string(), adresse: entwurf.adresse };
    if gewaehlt().is_none() {
        waehlen(name)?;
    }
    Ok(w)
}

/// Alle Wallets, nach Namen.
pub fn liste() -> Vec<Wallet> {
    let mut v: Vec<Wallet> = std::fs::read_dir(ordner())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let datei = p.file_name()?.to_str()?;
            let name = datei.strip_suffix(".key").or_else(|| datei.strip_suffix(KEYSTORE_ENDUNG))?.to_string();
            laden(&name).ok()
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v.dedup_by(|a, b| a.name == b.name);
    v
}

/// Liest ein Wallet.
pub fn laden(name: &str) -> Result<Wallet, String> {
    name_pruefen(name)?;
    if let Some(k) = keystore_lesen(name)? {
        let b: [u8; myl_types::bls::BLS_PK_LEN] =
            ikm_aus_beliebigem_hex(&k.pubkey).and_then(|v| v.try_into().ok()).ok_or("Keystore: kein öffentlicher Schlüssel")?;
        return Ok(Wallet { name: name.to_string(), adresse: Address::aus_schluessel(&myl_types::bls::BlsPublicKey(b)) });
    }
    let pfad = ordner().join(format!("{name}.key"));
    let text = std::fs::read_to_string(&pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
    let wort = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let ikm = ikm_aus_hex(wort).ok_or_else(|| format!("{}: kein Schlüssel", pfad.display()))?;
    Ok(Wallet { name: name.to_string(), adresse: adresse_aus_ikm(&ikm)? })
}

/// Das ausgewaehlte Wallet, falls es eines gibt und es noch existiert.
pub fn gewaehlt() -> Option<Wallet> {
    let name = std::fs::read_to_string(ordner().join(GEWAEHLT)).ok()?;
    laden(name.trim()).ok()
}

/// **Waehlt ein Wallet aus.** Laeuft das Mining, gilt es ab dem naechsten
/// Start; der Knoten traegt es dann ein.
pub fn waehlen(name: &str) -> Result<Wallet, String> {
    let w = laden(name)?;
    std::fs::write(ordner().join(GEWAEHLT), format!("{name}\n")).map_err(|e| e.to_string())?;
    Ok(w)
}

/// Das Wallet, das die Kette zuletzt als Auszahlungskonto fuehrte, soweit
/// der Client es gesehen hat.
pub fn eingetragen() -> Option<Wallet> {
    let name = std::fs::read_to_string(ordner().join(EINGETRAGEN)).ok()?;
    laden(name.trim()).ok()
}

/// Vermerkt, dass die Kette dieses Wallet als Auszahlungskonto fuehrt.
pub fn eingetragen_setzen(name: &str) -> Result<(), String> {
    std::fs::write(ordner().join(EINGETRAGEN), format!("{name}\n")).map_err(|e| e.to_string())
}

/// **Der geheime Schluessel eines Wallets**, zum Unterschreiben. Er
/// verlaesst diese Kiste nur als Unterschrift (`crate::bezahlen`).
pub(crate) fn geheimschluessel(name: &str) -> Result<BlsSecretKey, String> {
    name_pruefen(name)?;
    if keystorepfad(name).exists() {
        let offen = OFFEN.lock().map_err(|_| "Sperre vergiftet".to_string())?;
        let s = offen.get(name).ok_or_else(|| format!("{GESPERRT}: {name}"))?;
        return BlsSecretKey::aus_geheimen_bytes(**s).map_err(|_| "Schlüssel ungültig".to_string());
    }
    let pfad = ordner().join(format!("{name}.key"));
    let text = std::fs::read_to_string(&pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
    let wort = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let ikm = ikm_aus_hex(wort).ok_or_else(|| format!("{}: kein Schlüssel", pfad.display()))?;
    BlsSecretKey::aus_ausgangsmaterial(&ikm).map_err(|_| "Schlüssel nicht ableitbar".to_string())
}

/// Die Endung eines Keystores neben den `.key`-Dateien.
const KEYSTORE_ENDUNG: &str = ".keystore.json";

/// Was ein Aufruf meldet, solange das Wallet nicht entsperrt ist.
pub const GESPERRT: &str = "Das Wallet ist verschlüsselt und gesperrt";

/// ⚑ **Nur fuer die Lebensdauer dieses Prozesses**, und beim Beenden genullt.
static OFFEN: std::sync::Mutex<std::collections::BTreeMap<String, zeroize::Zeroizing<[u8; 32]>>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

fn keystorepfad(name: &str) -> PathBuf {
    ordner().join(format!("{name}{KEYSTORE_ENDUNG}"))
}

fn keystore_lesen(name: &str) -> Result<Option<crate::keystore::Keystore>, String> {
    let p = keystorepfad(name);
    match std::fs::read(&p) {
        Ok(b) => serde_json::from_slice(&b).map(Some).map_err(|e| format!("{}: {e}", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

fn keystore_schreiben(name: &str, geheim: &BlsSecretKey, passwort: &str) -> Result<(), String> {
    let oeffentlich = geheim.public_key().map_err(|_| "öffentlicher Teil nicht ableitbar".to_string())?;
    let pfad_text = format!("m/12381/{}/0/0", adresstext::COIN_NUMMER);
    let k = crate::keystore::verschluesseln(&geheim.geheime_bytes(), &oeffentlich.0, &pfad_text, "Myelith-Wallet", passwort)?;
    // ⛔️ Erst lesen, dann gilt es: Ein Keystore, der sich nicht oeffnen laesst,
    //    waere ein verlorener Schluessel.
    if *crate::keystore::entschluesseln(&k, passwort)? != geheim.geheime_bytes() {
        return Err("Der Keystore lässt sich nicht wieder öffnen.".into());
    }
    let p = keystorepfad(name);
    let text = serde_json::to_vec_pretty(&k).map_err(|e| e.to_string())?;
    myl_types::geheimdatei::schreiben(&p, &text).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(())
}

/// Ob dieses Wallet als Keystore liegt.
pub fn ist_verschluesselt(name: &str) -> bool {
    keystorepfad(name).exists()
}

/// Ob ein verschluesseltes Wallet in diesem Prozess offen ist.
pub fn ist_entsperrt(name: &str) -> bool {
    OFFEN.lock().is_ok_and(|o| o.contains_key(name))
}

/// **Oeffnet einen Keystore fuer diesen Prozess.**
pub fn entsperren(name: &str, passwort: &str) -> Result<(), String> {
    let k = keystore_lesen(name)?.ok_or_else(|| format!("„{name}“ ist nicht verschlüsselt."))?;
    let klar = crate::keystore::entschluesseln(&k, passwort)?;
    // ⛔️ **Der Schluessel muss zum oeffentlichen Teil passen** (CLIENT 7.15,
    //    gefunden mit der Verstuemmelungsprobe des Keystores): Die Pruefsumme
    //    nach EIP-2335 deckt den IV nicht. Ein veraenderter IV oeffnet den
    //    Keystore ohne Fehler und ergibt ein anderes Geheimnis; die Norm
    //    verweist dafuer auf das Feld `pubkey`. Ohne Passwort laesst sich das
    //    falsche Geheimnis nicht kennen, also auch kein passendes `pubkey`.
    let oeffentlich = BlsSecretKey::aus_geheimen_bytes(*klar)
        .and_then(|s| s.public_key())
        .map_err(|_| "Der Keystore ist beschädigt: kein gültiger Schlüssel.".to_string())?;
    if hex(&oeffentlich.0) != k.pubkey.to_ascii_lowercase() {
        return Err("Der Keystore ist beschädigt: Der Schlüssel passt nicht zum öffentlichen Teil.".into());
    }
    OFFEN.lock().map_err(|_| "Sperre vergiftet".to_string())?.insert(name.to_string(), klar);
    Ok(())
}

/// Der Schluessel eines entsperrten Wallets als Hex, fuer den Knoten ueber
/// eine Roehre (`--kontowechsel -`). `None`, wenn es gesperrt ist.
pub fn geheimhex_entsperrt(name: &str) -> Option<zeroize::Zeroizing<String>> {
    let o = OFFEN.lock().ok()?;
    o.get(name).map(|s| zeroize::Zeroizing::new(hex(s.as_ref())))
}

/// **Verschluesselt ein vorhandenes Wallet** und entfernt die Klartextdatei.
///
/// ⚠️ **Auf einer SSD oder in einer Sicherung kann die alte Datei
/// wiederherstellbar bleiben.** Wer das ausschliessen will, legt das Wallet
/// neu an und zieht um; die Meldung des Klienten sagt das.
pub fn verschluesseln(name: &str, passwort: &str) -> Result<(), String> {
    name_pruefen(name)?;
    if ist_verschluesselt(name) {
        return Err(format!("„{name}“ ist schon verschlüsselt."));
    }
    let geheim = geheimschluessel(name)?;
    keystore_schreiben(name, &geheim, passwort)?;
    let klar = ordner().join(format!("{name}.key"));
    // Einmal ueberschreiben kostet nichts, auch wenn es nichts garantiert.
    if let Ok(m) = std::fs::metadata(&klar) {
        let _ = std::fs::write(&klar, vec![0u8; m.len() as usize]);
    }
    std::fs::remove_file(&klar).map_err(|e| format!("{}: {e}", klar.display()))
}

/// ⚑ Mit dem Seed ist das das erste Konto nach EIP-2334
/// (`m/12381/<coin>/0/0`), nicht mehr der Hauptschluessel.
fn adresse_aus_ikm(ikm: &[u8]) -> Result<Address, String> {
    let geheim = BlsSecretKey::aus_ausgangsmaterial(ikm).map_err(|_| "Schlüssel nicht ableitbar".to_string())?;
    let oeffentlich = geheim.public_key().map_err(|_| "öffentlicher Teil nicht ableitbar".to_string())?;
    Ok(Address::aus_schluessel(&oeffentlich))
}

/// 32 Bytes (aeltere Wallets) oder 64 Bytes (Seed).
fn ikm_aus_hex(wort: &str) -> Option<Vec<u8>> {
    if wort.len() != 64 && wort.len() != 128 {
        return None;
    }
    (0..wort.len() / 2).map(|i| u8::from_str_radix(wort.get(i * 2..i * 2 + 2)?, 16).ok()).collect()
}

fn ikm_aus_beliebigem_hex(wort: &str) -> Option<Vec<u8>> {
    if !wort.len().is_multiple_of(2) {
        return None;
    }
    (0..wort.len() / 2).map(|i| u8::from_str_radix(wort.get(i * 2..i * 2 + 2)?, 16).ok()).collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod proben {
    use super::*;

    /// Die Proben teilen sich `MYL_WALLETS`; eine nach der anderen.
    static REIHE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn mit_ordner(f: impl FnOnce()) {
        let _g = REIHE.lock().unwrap_or_else(|e| e.into_inner());
        let d = tempfile::tempdir().expect("Ordner");
        std::env::set_var("MYL_WALLETS", d.path());
        f();
        std::env::remove_var("MYL_WALLETS");
    }

    const WOERTER: &str = "legal winner thank year wave sausage worth useful legal winner thank yellow";

    #[test]
    fn das_erste_wallet_wird_ausgewaehlt_und_die_adresse_bleibt() {
        mit_ordner(|| {
            let a = speichern("alpha", &entwerfen().expect("Entwurf")).expect("alpha");
            let b = speichern("beta", &entwerfen().expect("Entwurf")).expect("beta");
            assert_ne!(a.adresse, b.adresse);
            assert_eq!(gewaehlt().map(|w| w.name), Some("alpha".to_string()));
            waehlen("beta").expect("waehlen");
            assert_eq!(gewaehlt(), Some(b.clone()));
            assert_eq!(laden("alpha").expect("laden"), a);
            assert_eq!(liste().iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), ["alpha", "beta"]);
            assert!(speichern("alpha", &entwerfen().expect("Entwurf")).is_err(), "ein Name zweimal");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let rechte = std::fs::metadata(a.schluesselpfad()).expect("da").permissions().mode() & 0o777;
                assert_eq!(rechte, 0o600, "der Schlüssel ist für andere lesbar");
            }
        });
    }

    /// ⚑ **Dieselben Woerter, dasselbe Wallet**, und hier nur einmal.
    #[test]
    fn dieselben_woerter_geben_dasselbe_wallet_und_nur_einmal() {
        mit_ordner(|| {
            let e1 = aus_woertern(WOERTER).expect("Woerter");
            let e2 = aus_woertern(&WOERTER.to_uppercase()).expect("gross");
            assert_eq!(e1.adresse, e2.adresse);
            assert_eq!(e1.woerter().join(" "), WOERTER);
            let w = speichern("haupt", &e1).expect("speichern");
            let f = speichern("zweit", &e2).unwrap_err();
            assert!(f.contains("„haupt“"), "{f}");
            // Neu gelesen aus der Datei: dieselbe Adresse wie aus den Woertern.
            assert_eq!(laden("haupt").expect("laden").adresse, w.adresse);
        });
    }

    /// ⚑ Die Adresse folgt aus dem Seed nach BIP-39 ueber den Pfad nach
    /// EIP-2334, und die Datei traegt den Seed.
    #[test]
    fn die_datei_traegt_den_seed_im_format_des_knotens() {
        mit_ordner(|| {
            let e = aus_woertern(WOERTER).expect("Woerter");
            let w = speichern("format", &e).expect("speichern");
            let text = std::fs::read_to_string(w.schluesselpfad()).expect("lesen");
            let mut z = text.lines();
            assert_eq!(z.next(), Some(DATEIKOPF));
            let seed = ikm_aus_hex(z.next().expect("Zeile")).expect("Hex");
            assert_eq!(seed, bip39::seed(WOERTER, "").to_vec());
            assert_eq!(adresse_aus_ikm(&seed).expect("Adresse"), w.adresse);
        });
    }

    /// ⛔️ **Ohne die richtigen Woerter keine Bestaetigung**, und gefragt
    /// werden zwei verschiedene Stellen.
    /// ⚑ **Dasselbe Wallet, verschluesselt**: gleiche Adresse, gleiche
    /// Unterschrift, kein Klartext mehr auf der Platte, gesperrt bis zum Passwort.
    #[test]
    fn ein_verschluesseltes_wallet_unterschreibt_erst_nach_dem_passwort() {
        mit_ordner(|| {
            let e = aus_woertern(WOERTER).unwrap();
            let w = speichern("klar", &e).unwrap();
            let vorher = geheimschluessel("klar").unwrap().geheime_bytes();
            verschluesseln("klar", "ein gutes Passwort").unwrap();
            assert!(!ordner().join("klar.key").exists(), "die Klartextdatei ist weg");
            let text = std::fs::read_to_string(ordner().join("klar.keystore.json")).unwrap();
            assert!(!text.contains(&hex(&vorher)), "der Schluessel steht nicht im Klartext");
            assert!(!text.contains(&hex(&e.seed)));
            assert_eq!(laden("klar").unwrap().adresse, w.adresse, "die Adresse kommt ohne Passwort");
            assert_eq!(liste().len(), 1);
            assert!(geheimschluessel("klar").err().unwrap().contains(GESPERRT));
            assert_eq!(entsperren("klar", "falsch").unwrap_err(), crate::keystore::FALSCHES_PASSWORT);
            assert!(!ist_entsperrt("klar"));
            entsperren("klar", "ein gutes Passwort").unwrap();
            assert_eq!(geheimschluessel("klar").unwrap().geheime_bytes(), vorher);
            assert_eq!(geheimhex_entsperrt("klar").unwrap().as_str(), hex(&vorher));
            assert!(verschluesseln("klar", "noch ein Passwort").unwrap_err().contains("schon"));
        });
    }

    /// ⛔️ **Ein veraenderter IV wird beim Entsperren erkannt** (CLIENT 7.15):
    /// Der Keystore oeffnet sich, aber das Geheimnis passt nicht zum
    /// oeffentlichen Teil. Gegenprobe: derselbe Keystore unveraendert
    /// entsperrt.
    #[test]
    fn ein_veraenderter_iv_wird_beim_entsperren_erkannt() {
        mit_ordner(|| {
            let e = aus_woertern(WOERTER).unwrap();
            speichern_mit("iv", &e, Some("ein gutes Passwort")).unwrap();
            let pfad = ordner().join("iv.keystore.json");
            let echt = std::fs::read_to_string(&pfad).unwrap();
            let mut k: crate::keystore::Keystore = serde_json::from_str(&echt).unwrap();
            let erstes = if k.crypto.cipher.params.iv.starts_with('0') { "1" } else { "0" };
            k.crypto.cipher.params.iv.replace_range(0..1, erstes);
            std::fs::write(&pfad, serde_json::to_vec_pretty(&k).unwrap()).unwrap();
            assert!(crate::keystore::entschluesseln(&k, "ein gutes Passwort").is_ok(), "die Norm prueft den IV nicht");
            assert!(entsperren("iv", "ein gutes Passwort").unwrap_err().contains("beschädigt"));
            assert!(!ist_entsperrt("iv"));
            std::fs::write(&pfad, &echt).unwrap();
            entsperren("iv", "ein gutes Passwort").unwrap();
            assert!(ist_entsperrt("iv"));
        });
    }

    /// ⛔️ **Mit Passwort entsteht nie eine Klartextdatei.**
    #[test]
    fn mit_passwort_gibt_es_keine_klartextdatei() {
        mit_ordner(|| {
            let e = aus_woertern(WOERTER).unwrap();
            let w = speichern_mit("sicher", &e, Some("ein gutes Passwort")).unwrap();
            assert!(!ordner().join("sicher.key").exists());
            assert!(ist_verschluesselt("sicher"));
            assert_eq!(gewaehlt().unwrap().name, "sicher");
            assert_eq!(laden("sicher").unwrap().adresse, w.adresse);
            assert!(speichern("sicher", &aus_woertern(WOERTER).unwrap()).is_err());
            let k = keystore_lesen("sicher").unwrap().unwrap();
            assert_eq!(k.path, format!("m/12381/{}/0/0", adresstext::COIN_NUMMER));
            assert!(speichern_mit("kurz", &entwerfen().unwrap(), Some("kurz")).unwrap_err().contains("mindestens"));
            assert!(!ordner().join("kurz.key").exists() && !ist_verschluesselt("kurz"));
        });
    }

    #[test]
    fn bestaetigt_nur_mit_den_richtigen_woertern() {
        let e = aus_woertern(WOERTER).expect("Woerter");
        for _ in 0..50 {
            let (a, b) = e.abfrage();
            assert!(1 <= a && a < b && b <= 12, "{a} {b}");
        }
        assert!(e.bestaetigt(&[(3, "thank"), (8, " Useful ")]));
        assert!(!e.bestaetigt(&[(3, "thank"), (8, "worth")]));
        assert!(!e.bestaetigt(&[(13, "thank")]));
        assert!(!e.bestaetigt(&[]));
        assert!(!format!("{e:?}").contains("legal"), "Debug zeigt die Woerter");
    }

    /// Die Antwort des Probenetzes fuer ein Testkonto mit Guthaben, und
    /// fuer eine unbenutzte Adresse.
    #[test]
    fn der_befund_aus_der_antwort_des_knotens() {
        let bekannt = r#"{"hoehe":25045,"bekannt":true,"guthaben":130000000,"nonce":0,"miner":false,"auszahlung_fuer":1}"#;
        let b = befund_aus(bekannt, "x");
        assert_eq!(b, Netzbefund::Bekannt { hoehe: 25045, guthaben: 130_000_000, nonce: 0, miner: false, auszahlung_fuer: 1 });
        assert!(b.satz().contains("schon vorhanden: 130 MYL, Auszahlungskonto für 1 Miner"), "{}", b.satz());
        let leer = r#"{"hoehe":7,"bekannt":false,"guthaben":0,"nonce":0,"miner":false,"auszahlung_fuer":0}"#;
        assert_eq!(befund_aus(leer, "x"), Netzbefund::Unbekannt { hoehe: 7 });
        assert!(matches!(befund_aus("Unsinn", "x"), Netzbefund::NichtGeprueft(_)));
    }

    #[test]
    fn ein_name_mit_pfad_wird_abgewiesen() {
        for name in ["../weg", "a/b", "", "mit leerzeichen"] {
            assert!(name_pruefen(name).is_err(), "{name:?} wurde angenommen");
        }
    }
}
