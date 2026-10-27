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
//!    Seed nach BIP-39, Schluessel nach EIP-2333: dieselben Woerter geben
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
//! Die Schluessel liegen **unverschluesselt** mit Rechten `0600` neben den
//! Einstellungen. Fuer MYL der Probekette genuegt das. Vor echten MYL fehlen
//! eine verschluesselte Ablage (Keystore nach EIP-2335, dazu der
//! Schluesselbund des Systems), ein kaltes Auszahlungskonto ohne Schluessel
//! auf dieser Maschine und eine pruefbare Auskunft gegen die Zustandswurzel.
//!
//! # ⚑ Dasselbe Dateiformat wie der Knoten
//!
//! Kopfzeile und Ausgangsmaterial in Hex: 64 Bytes Seed bei einem Wallet aus
//! Woertern, 32 Bytes bei einem aelteren. Der Knoten liest beides
//! (`myl_node::schluessel`), und nur so kann er den Schluessel eines Wallets
//! fuer einen Wechsel lesen.

use std::path::PathBuf;

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

    /// Wo der Schluessel liegt.
    pub fn schluesselpfad(&self) -> PathBuf {
        ordner().join(format!("{}.key", self.name))
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
    let mut e = [0u8; bip39::ENTROPIE_BYTES];
    getrandom::fill(&mut e).map_err(|f| format!("keine Zufallszahlen: {f}"))?;
    let woerter = bip39::woerter(&e).join(" ");
    aus_woertern(&woerter)
}

/// **Stellt ein Wallet aus zwoelf Woertern her**, mit Pruefung von Liste
/// und Pruefsumme.
pub fn aus_woertern(text: &str) -> Result<Entwurf, String> {
    let e = bip39::entropie(text)?;
    let woerter: Vec<String> = bip39::woerter(&e).iter().map(|w| w.to_string()).collect();
    let seed = bip39::seed(&woerter.join(" "), "");
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
    name_pruefen(name)?;
    if let Some(da) = liste().into_iter().find(|w| w.adresse == entwurf.adresse) {
        return Err(format!("Dieses Wallet gibt es hier schon: „{}“.", da.name));
    }
    let d = ordner();
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    let pfad = d.join(format!("{name}.key"));
    if pfad.exists() {
        return Err(format!("Ein Wallet „{name}“ gibt es schon."));
    }
    let text = format!("{DATEIKOPF}\n{}\n", hex(&entwurf.seed));
    // ⚑ `0600` ab dem Anlegen, wie im Knoten (Fund 607).
    myl_types::geheimdatei::schreiben(&pfad, text.as_bytes()).map_err(|e| format!("{}: {e}", pfad.display()))?;
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
            let name = p.file_name()?.to_str()?.strip_suffix(".key")?.to_string();
            laden(&name).ok()
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// Liest ein Wallet.
pub fn laden(name: &str) -> Result<Wallet, String> {
    name_pruefen(name)?;
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
    let pfad = ordner().join(format!("{name}.key"));
    let text = std::fs::read_to_string(&pfad).map_err(|e| format!("{}: {e}", pfad.display()))?;
    let wort = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let ikm = ikm_aus_hex(wort).ok_or_else(|| format!("{}: kein Schlüssel", pfad.display()))?;
    BlsSecretKey::key_gen(&ikm).map_err(|_| "Schlüssel nicht ableitbar".to_string())
}

/// ⚑ Mit dem Seed ist das der Hauptschluessel nach EIP-2333 (IETF KeyGen
/// ueber den Seed).
fn adresse_aus_ikm(ikm: &[u8]) -> Result<Address, String> {
    let geheim = BlsSecretKey::key_gen(ikm).map_err(|_| "Schlüssel nicht ableitbar".to_string())?;
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

    /// ⚑ Die Adresse folgt aus dem Seed nach BIP-39 ueber `key_gen`
    /// (EIP-2333, Hauptschluessel), und die Datei traegt den Seed.
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
