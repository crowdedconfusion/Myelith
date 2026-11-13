//! **Signierte Herkunftsangaben nach C2PA** (Content Credentials,
//! Spezifikation 2.2): ein Manifest in jeder erzeugten Tondatei, das sie als
//! KI-erzeugt ausweist und den Ton an eine Signatur bindet.
//!
//! ⚑ **Warum**: Der Verhaltenskodex der Kommission zur Kennzeichnung nach
//! Art. 50 KI-Verordnung (2026-06-10) sieht mindestens zwei Schichten vor:
//! signierte Metadaten und ein unhoerbares Wasserzeichen. Dies ist die
//! erste; das Wasserzeichen steht in [`super`].
//!
//! ⚑ **Ein frischer Schluessel je Datei** (Festlegung des Projektinhabers,
//! 2026-11-11): Ed25519, nur fuer diese Datei erzeugt, mit einem selbst
//! signierten Zertifikat. So bleiben Dateien eines Nutzers unverknuepfbar.
//! 📌 Der Name im Zertifikat traegt eine Organisation (`O`): Ohne sie
//! meldet `c2pa-rs` 0.90 (`c2patool` und die Pruefseite)
//! `CoseX5ChainMissing` und `claimSignature.mismatch`, auch bei einem
//! Zertifikat aus `openssl`; gemessen mit `c2patool` 0.27.23.
//! ⚠️ Ohne ein Zertifikat einer CA auf der C2PA-Vertrauensliste beweist die
//! Signatur nicht, **wer** signiert hat; sie zeigt, dass Ton und Angaben
//! seit dem Signieren unveraendert sind, und Pruefprogramme melden einen
//! unbekannten Signierer.
//!
//! Aufbau, nach Abschnitten der Spezifikation:
//!
//! - Ablage `c2pa` (11.1.4.2) mit einem Manifest `urn:c2pa:<uuid>` (8.1):
//!   Behauptungen `c2pa.assertions`, Anspruch `c2pa.claim.v2`, Signatur
//!   `c2pa.signature` (11.2.1).
//! - Behauptungen: `c2pa.actions.v2` mit `c2pa.created` und dem Quellentyp
//!   `trainedAlgorithmicMedia` (18.14.2); `c2pa.hash.data` ueber die ganze
//!   Datei ausser dem Block `C2PA` (18.5).
//! - Signatur: `COSE_Sign1_Tagged` (13.2.2), `alg` EdDSA und `x5chain` im
//!   geschuetzten Kopf (14.5), Nutzlast abgetrennt; signiert wird
//!   `["Signature1", geschuetzt, h'', Anspruch]` (13.2.3).
//! - `x5chain` als Liste, auch mit nur einem Zertifikat, wie `c2patool`.
//! - In WAV: als Block `C2PA` am Ende der RIFF-Datei (A.3.6).
//!
//! ⚑ **Der ausgenommene Bereich ist der ganze Block `C2PA` mit Kopf**,
//! genau `Laenge + 8`, wie in der Referenzumsetzung `c2pa-rs`
//! (`riff_io.rs`), die ihn beim Pruefen auch genau so verlangt. Die
//! Spezifikation sagt es fuer RIFF nicht.

use sha2::{Digest, Sha256};

/// Der Werkzeugname in den Angaben.
const WERKZEUG: &str = "Myelith";

/// Die UUIDs der JUMBF-Kaesten (11.1.4): vier Zeichen, dann
/// `0011-0010-8000-00AA00389B71`.
fn uuid_von(kennung: &[u8; 4]) -> [u8; 16] {
    let mut u = [0u8; 16];
    u[..4].copy_from_slice(kennung);
    u[4..].copy_from_slice(&[0x00, 0x11, 0x00, 0x10, 0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
    u
}

// ---------------------------------------------------------------- CBOR

/// Ein CBOR-Wert, so weit C2PA ihn braucht.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cbor {
    Zahl(u64),
    Negativ(u64),
    Bytes(Vec<u8>),
    Text(String),
    Liste(Vec<Cbor>),
    Karte(Vec<(Cbor, Cbor)>),
    Marke(u64, Box<Cbor>),
    Nichts,
}

/// So tief darf ein gelesener CBOR-Wert geschachtelt sein.
const CBOR_TIEFE_HOECHSTENS: usize = 64;

fn kopf(aus: &mut Vec<u8>, art: u8, n: u64) {
    let a = art << 5;
    match n {
        0..=23 => aus.push(a | n as u8),
        24..=0xFF => aus.extend([a | 24, n as u8]),
        0x100..=0xFFFF => {
            aus.push(a | 25);
            aus.extend((n as u16).to_be_bytes());
        }
        0x1_0000..=0xFFFF_FFFF => {
            aus.push(a | 26);
            aus.extend((n as u32).to_be_bytes());
        }
        _ => {
            aus.push(a | 27);
            aus.extend(n.to_be_bytes());
        }
    }
}

impl Cbor {
    fn text(t: &str) -> Self {
        Cbor::Text(t.to_string())
    }

    /// Eine Karte aus Textschluesseln.
    fn karte(paare: Vec<(&str, Cbor)>) -> Self {
        Cbor::Karte(paare.into_iter().map(|(k, v)| (Cbor::text(k), v)).collect())
    }

    /// **Deterministisch kodiert** (RFC 8949, 4.2.1): kuerzeste Koepfe,
    /// Schluessel einer Karte nach ihren Bytes geordnet.
    pub(crate) fn kodieren(&self) -> Vec<u8> {
        let mut aus = Vec::new();
        self.schreiben(&mut aus);
        aus
    }

    fn schreiben(&self, aus: &mut Vec<u8>) {
        match self {
            Cbor::Zahl(n) => kopf(aus, 0, *n),
            Cbor::Negativ(n) => kopf(aus, 1, *n),
            Cbor::Bytes(b) => {
                kopf(aus, 2, b.len() as u64);
                aus.extend(b);
            }
            Cbor::Text(t) => {
                kopf(aus, 3, t.len() as u64);
                aus.extend(t.as_bytes());
            }
            Cbor::Liste(l) => {
                kopf(aus, 4, l.len() as u64);
                l.iter().for_each(|w| w.schreiben(aus));
            }
            Cbor::Karte(m) => {
                let mut paare: Vec<(Vec<u8>, Vec<u8>)> = m.iter().map(|(k, v)| (k.kodieren(), v.kodieren())).collect();
                paare.sort();
                kopf(aus, 5, paare.len() as u64);
                for (k, v) in paare {
                    aus.extend(k);
                    aus.extend(v);
                }
            }
            Cbor::Marke(t, w) => {
                kopf(aus, 6, *t);
                w.schreiben(aus);
            }
            Cbor::Nichts => aus.push(0xF6),
        }
    }

    /// Liest einen Wert ab `*i`; nur feste Laengen.
    pub(crate) fn lesen(b: &[u8], i: &mut usize) -> Result<Self, String> {
        Self::lesen_in(b, i, 0)
    }

    /// ⛔️ **Mit Tiefengrenze** (2026-11-13): Jede Liste, Karte und Marke
    /// ruft sich selbst. 📌 Ohne Grenze sprengte eine Datei mit einem
    /// Block `C2PA` aus lauter `0x81` (Liste mit einem Element) den Stapel,
    /// und `myl kennzeichen` brach den Prozess ab, statt die Datei
    /// abzulehnen. Ein Manifest nach C2PA ist sechs Ebenen tief.
    fn lesen_in(b: &[u8], i: &mut usize, tiefe: usize) -> Result<Self, String> {
        if tiefe > CBOR_TIEFE_HOECHSTENS {
            return Err(format!("CBOR: tiefer als {CBOR_TIEFE_HOECHSTENS} Ebenen"));
        }
        let erstes = *b.get(*i).ok_or("CBOR zu kurz")?;
        *i += 1;
        let (art, zusatz) = (erstes >> 5, erstes & 0x1F);
        let mut lang = |n: usize| -> Result<u64, String> {
            let s = b.get(*i..*i + n).ok_or("CBOR zu kurz")?;
            *i += n;
            Ok(s.iter().fold(0u64, |a, &x| (a << 8) | x as u64))
        };
        let n = match zusatz {
            0..=23 => zusatz as u64,
            24 => lang(1)?,
            25 => lang(2)?,
            26 => lang(4)?,
            27 => lang(8)?,
            _ if art == 7 && zusatz == 22 => 0,
            _ => return Err(format!("CBOR: Kopf {erstes:#x} wird nicht gelesen")),
        };
        let mut stueck = |n: u64| -> Result<Vec<u8>, String> {
            let ende = usize::try_from(n).ok().and_then(|n| i.checked_add(n)).ok_or("CBOR zu kurz")?;
            let s = b.get(*i..ende).ok_or("CBOR zu kurz")?.to_vec();
            *i = ende;
            Ok(s)
        };
        Ok(match art {
            0 => Cbor::Zahl(n),
            1 => Cbor::Negativ(n),
            2 => Cbor::Bytes(stueck(n)?),
            3 => Cbor::Text(String::from_utf8(stueck(n)?).map_err(|_| "CBOR: Text ist kein UTF-8")?),
            4 => Cbor::Liste((0..n).map(|_| Cbor::lesen_in(b, i, tiefe + 1)).collect::<Result<_, _>>()?),
            5 => Cbor::Karte((0..n).map(|_| Ok((Cbor::lesen_in(b, i, tiefe + 1)?, Cbor::lesen_in(b, i, tiefe + 1)?))).collect::<Result<_, String>>()?),
            6 => Cbor::Marke(n, Box::new(Cbor::lesen_in(b, i, tiefe + 1)?)),
            7 if zusatz == 22 => Cbor::Nichts,
            _ => return Err(format!("CBOR: Art {art} wird nicht gelesen")),
        })
    }

    fn feld(&self, schluessel: &str) -> Option<&Cbor> {
        let Cbor::Karte(m) = self else { return None };
        m.iter().find(|(k, _)| matches!(k, Cbor::Text(t) if t == schluessel)).map(|(_, v)| v)
    }
}

// ---------------------------------------------------------------- JUMBF

/// Ein Kasten: Laenge (4 Byte, gross zuerst), Art, Inhalt (ISO 19566-5).
fn kasten(art: &[u8; 4], inhalt: &[u8]) -> Vec<u8> {
    let mut aus = ((inhalt.len() + 8) as u32).to_be_bytes().to_vec();
    aus.extend(art);
    aus.extend(inhalt);
    aus
}

/// Der Inhalt eines Ueberkastens `jumb` ohne seinen eigenen Kopf: die
/// Beschreibung `jumd` (Art, Schalter `0x03` fuer abrufbar und benannt,
/// Name mit Nullbyte), dann die Inhalte.
fn ueberinhalt(art: &[u8; 4], name: &str, inhalte: &[Vec<u8>]) -> Vec<u8> {
    let mut d = uuid_von(art).to_vec();
    d.push(0x03);
    d.extend(name.as_bytes());
    d.push(0);
    let mut aus = kasten(b"jumd", &d);
    inhalte.iter().for_each(|k| aus.extend(k));
    aus
}

fn ueberkasten(art: &[u8; 4], name: &str, inhalte: &[Vec<u8>]) -> Vec<u8> {
    kasten(b"jumb", &ueberinhalt(art, name, inhalte))
}

/// Ein Kasten: Art und Inhalt ohne Kopf.
type Kasten<'a> = ([u8; 4], &'a [u8]);

/// Die Kaesten eines Inhalts.
fn kaesten(b: &[u8]) -> Result<Vec<Kasten<'_>>, String> {
    let mut aus = Vec::new();
    let mut i = 0;
    while i + 8 <= b.len() {
        let laenge = u32::from_be_bytes(b[i..i + 4].try_into().expect("vier")) as usize;
        if laenge < 8 || i + laenge > b.len() {
            return Err("JUMBF: Kasten ueber das Ende".into());
        }
        aus.push((b[i + 4..i + 8].try_into().expect("vier"), &b[i + 8..i + laenge]));
        i += laenge;
    }
    Ok(aus)
}

/// Name eines Ueberkastens und seine Inhaltskaesten.
fn zerlegen(inhalt: &[u8]) -> Result<(String, Vec<Kasten<'_>>), String> {
    let k = kaesten(inhalt)?;
    let (art, d) = k.first().ok_or("JUMBF: leer")?;
    if art != b"jumd" || d.len() < 18 {
        return Err("JUMBF: keine Beschreibung".into());
    }
    let ende = d[17..].iter().position(|&x| x == 0).ok_or("JUMBF: Name ohne Ende")?;
    Ok((String::from_utf8_lossy(&d[17..17 + ende]).into_owned(), k[1..].to_vec()))
}

// ---------------------------------------------------------------- DER

fn der(tag: u8, inhalt: &[u8]) -> Vec<u8> {
    let mut aus = vec![tag];
    let n = inhalt.len();
    if n < 0x80 {
        aus.push(n as u8);
    } else {
        let b = (n as u32).to_be_bytes();
        let erste = b.iter().position(|&x| x != 0).unwrap_or(3);
        aus.push(0x80 | (4 - erste) as u8);
        aus.extend(&b[erste..]);
    }
    aus.extend(inhalt);
    aus
}

fn folge(teile: &[Vec<u8>]) -> Vec<u8> {
    der(0x30, &teile.concat())
}

/// Eine OID aus ihren Zahlen.
fn oid(zahlen: &[u64]) -> Vec<u8> {
    let mut b = vec![(zahlen[0] * 40 + zahlen[1]) as u8];
    for &z in &zahlen[2..] {
        let mut stellen = vec![(z & 0x7F) as u8];
        let mut r = z >> 7;
        while r > 0 {
            stellen.push(0x80 | (r & 0x7F) as u8);
            r >>= 7;
        }
        stellen.reverse();
        b.extend(stellen);
    }
    der(0x06, &b)
}

const ED25519: [u64; 4] = [1, 3, 101, 112];

/// Datum aus Tagen seit 1970 (Hinnant, `civil_from_days`).
fn datum(tage: i64) -> (i64, u32, u32) {
    let z = tage + 719_468;
    let ara = z.div_euclid(146_097);
    let t = z.rem_euclid(146_097);
    let j = (t - t / 1460 + t / 36_524 - t / 146_096) / 365;
    let y = j + ara * 400;
    let tj = t - (365 * j + j / 4 - j / 100);
    let m = (5 * tj + 2) / 153;
    let d = (tj - (153 * m + 2) / 5 + 1) as u32;
    let m = if m < 10 { m + 3 } else { m - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `UTCTime` jetzt (bis 2049 gueltig, RFC 5280 4.1.2.5).
fn jetzt_utc() -> Vec<u8> {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (y, m, d) = datum(s.div_euclid(86_400));
    let r = s.rem_euclid(86_400);
    let t = format!("{:02}{m:02}{d:02}{:02}{:02}{:02}Z", y % 100, r / 3600, r / 60 % 60, r % 60);
    der(0x17, t.as_bytes())
}

/// **Das Zertifikat des Schluessels**, selbst signiert (14.5.1.1): v3,
/// Ed25519, Name `CN=Myelith, O=Myelith`, gueltig bis 9999-12-31
/// (RFC 5280 4.1.2.5), Schluesselgebrauch `digitalSignature` (kritisch),
/// erweitert `c2pa-kp-claimSigning` und `emailProtection`, kein CA,
/// Schluessel-ID und gleiche Aussteller-Schluessel-ID.
fn zertifikat(schluessel: &ed25519_dalek::SigningKey, seriennummer: &[u8; 16]) -> Vec<u8> {
    use ed25519_dalek::Signer;
    let algo = folge(&[oid(&ED25519)]);
    let attribut = |id: u64| der(0x31, &folge(&[oid(&[2, 5, 4, id]), der(0x0C, WERKZEUG.as_bytes())]));
    let name = folge(&[attribut(3), attribut(10)]);
    let oeffentlich = schluessel.verifying_key().to_bytes();
    let spki = folge(&[algo.clone(), der(0x03, &[&[0u8][..], &oeffentlich].concat())]);
    let erweiterung = |id: &[u64], kritisch: bool, wert: Vec<u8>| {
        let mut t = vec![oid(id)];
        if kritisch {
            t.push(der(0x01, &[0xFF]));
        }
        t.push(der(0x04, &wert));
        folge(&t)
    };
    let kennung = &Sha256::digest(oeffentlich)[..20];
    let erweiterungen = der(
        0xA3,
        &folge(&[
            erweiterung(&[2, 5, 29, 15], true, der(0x03, &[0x07, 0x80])),
            erweiterung(&[2, 5, 29, 37], false, folge(&[oid(&[1, 3, 6, 1, 4, 1, 62558, 2, 1]), oid(&[1, 3, 6, 1, 5, 5, 7, 3, 4])])),
            erweiterung(&[2, 5, 29, 19], true, folge(&[])),
            erweiterung(&[2, 5, 29, 14], false, der(0x04, kennung)),
            // 📌 Authority Key Identifier, bei selbst signiert gleich der
            // Schluessel-ID: Die Spezifikation verlangt ihn nur bei fremd
            // signierten Zertifikaten, `c2pa-rs` (`certificate_profile.rs`)
            // bei jedem, sonst `signingCredential.invalid`.
            erweiterung(&[2, 5, 29, 35], false, folge(&[der(0x80, kennung)])),
        ]),
    );
    let tbs = folge(&[
        der(0xA0, &der(0x02, &[2])),
        der(0x02, seriennummer),
        algo.clone(),
        name.clone(),
        folge(&[jetzt_utc(), der(0x18, b"99991231235959Z")]),
        name,
        spki,
        erweiterungen,
    ]);
    let unterschrift = schluessel.sign(&tbs).to_bytes();
    folge(&[tbs, algo, der(0x03, &[&[0u8][..], &unterschrift].concat())])
}

/// Der oeffentliche Schluessel aus einem Ed25519-Zertifikat: die Bitfolge
/// nach der OID `1.3.101.112` in den Angaben zum Schluessel.
fn oeffentlicher_schluessel(zert: &[u8]) -> Option<[u8; 32]> {
    let muster = [&oid(&ED25519)[..], &[0x03, 0x21, 0x00]].concat();
    let i = zert.windows(muster.len()).position(|w| w == muster)?;
    zert.get(i + muster.len()..i + muster.len() + 32)?.try_into().ok()
}

// ---------------------------------------------------------------- Manifest

fn zufall<const N: usize>() -> Result<[u8; N], String> {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).map_err(|f| format!("kein Zufall fuer die Signatur: {f}"))?;
    Ok(b)
}

fn uuid4(b: [u8; 16]) -> String {
    let mut b = b;
    b[6] = (b[6] & 0x0F) | 0x40;
    b[8] = (b[8] & 0x3F) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// Was beim Bauen fest ist: Schluessel, Zertifikat, Namen.
struct Bauplan {
    schluessel: ed25519_dalek::SigningKey,
    zertifikat: Vec<u8>,
    manifest: String,
    instanz: String,
}

fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Das Manifest mit diesem Hash ueber die Datei und diesem ausgenommenen
/// Bereich.
fn manifest(p: &Bauplan, hash: &[u8], start: u64, laenge: u64, fuellung: usize) -> Vec<u8> {
    use ed25519_dalek::Signer;
    let werkzeug = || Cbor::karte(vec![("name", Cbor::text(WERKZEUG)), ("version", Cbor::Text(version()))]);
    let aktionen = Cbor::karte(vec![(
        "actions",
        Cbor::Liste(vec![Cbor::karte(vec![
            ("action", Cbor::text("c2pa.created")),
            ("digitalSourceType", Cbor::text(super::QUELLENTYP)),
            ("softwareAgent", werkzeug()),
        ])]),
    )]);
    let bindung = Cbor::karte(vec![
        ("exclusions", Cbor::Liste(vec![Cbor::karte(vec![("start", Cbor::Zahl(start)), ("length", Cbor::Zahl(laenge))])])),
        ("name", Cbor::text("jumbf manifest")),
        ("alg", Cbor::text("sha256")),
        ("hash", Cbor::Bytes(hash.to_vec())),
        ("pad", Cbor::Bytes(vec![0; fuellung])),
    ]);
    let behauptungen: Vec<(&str, Vec<u8>)> = vec![("c2pa.actions.v2", aktionen.kodieren()), ("c2pa.hash.data", bindung.kodieren())];
    let mut bezuege = Vec::new();
    let mut kaesten_b = Vec::new();
    for (name, cbor) in &behauptungen {
        let inhalt = ueberinhalt(b"cbor", name, &[kasten(b"cbor", cbor)]);
        // ⚑ Gehasht wird der Inhalt des Ueberkastens ohne seinen Kopf (8.4.2.3).
        bezuege.push(Cbor::karte(vec![("url", Cbor::Text(format!("self#jumbf=c2pa.assertions/{name}"))), ("hash", Cbor::Bytes(Sha256::digest(&inhalt).to_vec()))]));
        kaesten_b.push(kasten(b"jumb", &inhalt));
    }
    let anspruch = Cbor::karte(vec![
        ("instanceID", Cbor::Text(p.instanz.clone())),
        ("claim_generator_info", werkzeug()),
        ("signature", Cbor::text("self#jumbf=c2pa.signature")),
        ("created_assertions", Cbor::Liste(bezuege)),
        ("alg", Cbor::text("sha256")),
    ])
    .kodieren();
    let geschuetzt = Cbor::Karte(vec![(Cbor::Zahl(1), Cbor::Negativ(7)), (Cbor::Zahl(33), Cbor::Liste(vec![Cbor::Bytes(p.zertifikat.clone())]))]).kodieren();
    let zu_signieren = Cbor::Liste(vec![Cbor::text("Signature1"), Cbor::Bytes(geschuetzt.clone()), Cbor::Bytes(Vec::new()), Cbor::Bytes(anspruch.clone())]).kodieren();
    let signatur = p.schluessel.sign(&zu_signieren).to_bytes();
    let cose = Cbor::Marke(18, Box::new(Cbor::Liste(vec![Cbor::Bytes(geschuetzt), Cbor::Karte(Vec::new()), Cbor::Nichts, Cbor::Bytes(signatur.to_vec())]))).kodieren();
    let manifest = ueberkasten(
        b"c2ma",
        &p.manifest,
        &[
            ueberkasten(b"c2as", "c2pa.assertions", &kaesten_b),
            ueberkasten(b"c2cl", "c2pa.claim.v2", &[kasten(b"cbor", &anspruch)]),
            ueberkasten(b"c2cs", "c2pa.signature", &[kasten(b"cbor", &cose)]),
        ],
    );
    ueberkasten(b"c2pa", "c2pa", &[manifest])
}

/// Die Datei mit einem Block `C2PA` aus `ablage` am Ende, die RIFF-Laenge
/// angepasst.
fn mit_block(wav: &[u8], ablage: &[u8]) -> Vec<u8> {
    let mut aus = wav.to_vec();
    aus.extend(b"C2PA");
    aus.extend((ablage.len() as u32).to_le_bytes());
    aus.extend(ablage);
    if ablage.len() % 2 == 1 {
        aus.push(0);
    }
    let riff = (aus.len() - 8) as u32;
    aus[4..8].copy_from_slice(&riff.to_le_bytes());
    aus
}

/// SHA-256 ueber die Datei ohne den Bereich `[start, start + laenge)`.
fn hash_ohne(datei: &[u8], start: usize, laenge: usize) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(&datei[..start]);
    h.update(&datei[(start + laenge).min(datei.len())..]);
    h.finalize().to_vec()
}

/// **Signiert ein WAV**: haengt das Manifest als Block `C2PA` an.
///
/// ⚑ Zwei Durchgaenge mit fester Groesse: erst mit einem Hash aus Nullen,
/// um Lage und Laenge des Blocks zu kennen, dann mit dem echten Hash ueber
/// die fertige Datei. Hash (32 Byte), Signatur (64 Byte) und Zertifikat
/// (feste Seriennummernlaenge) aendern die Groesse nicht. 📌 Die Laenge des
/// Blocks steht selbst im Manifest und waechst als CBOR-Zahl mit ihrem Wert;
/// sie wird gerechnet, bis sie sich bestaetigt.
///
/// ⚑ **Das Manifest ist immer gerade lang**, ueber das Feld `pad` der
/// Bindung: Dann braucht der Block kein Fuellbyte, und der ausgenommene
/// Bereich ist eindeutig Kopf und Inhalt (`Laenge + 8`). 📌 Die erste
/// Fassung nahm bei ungerader Laenge das Fuellbyte von RIFF mit in den
/// Bereich; die Referenzumsetzung `c2pa-rs` verlangt genau `Laenge + 8` und
/// hasht das Fuellbyte mit, und die Pruefseite der Content Authenticity
/// Initiative meldete die Datei als „moeglicherweise manipuliert“.
pub(crate) fn signieren(wav: Vec<u8>) -> Result<Vec<u8>, String> {
    if wav.len() < 12 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err("C2PA: kein WAV".into());
    }
    if bloecke(&wav).iter().any(|(id, _, _)| id == b"C2PA") {
        return Err("C2PA: die Datei traegt schon ein Manifest".into());
    }
    let mut seriennummer = zufall::<16>()?;
    seriennummer[0] = (seriennummer[0] & 0x7F) | 0x01;
    let schluessel = ed25519_dalek::SigningKey::from_bytes(&zufall::<32>()?);
    let p = Bauplan {
        zertifikat: zertifikat(&schluessel, &seriennummer),
        schluessel,
        manifest: format!("urn:c2pa:{}", uuid4(zufall::<16>()?)),
        instanz: format!("xmp:iid:{}", uuid4(zufall::<16>()?)),
    };
    let start = wav.len() as u64;
    // Die Laenge des Blocks steht selbst im Manifest und waechst als
    // CBOR-Zahl mit ihrem Wert; und das Manifest soll gerade lang sein
    // (siehe unten). Gerechnet, bis beides steht.
    let (mut laenge, mut fuellung) = (0u64, 0usize);
    let mut probe = manifest(&p, &[0u8; 32], start, laenge, fuellung);
    for _ in 0..8 {
        if probe.len() % 2 == 1 {
            fuellung += 1;
        } else if (8 + probe.len()) as u64 == laenge {
            break;
        } else {
            laenge = (8 + probe.len()) as u64;
        }
        probe = manifest(&p, &[0u8; 32], start, laenge, fuellung);
    }
    if probe.len() % 2 == 1 || (8 + probe.len()) as u64 != laenge {
        return Err("C2PA: die Laenge des Blocks fand keinen festen Wert".into());
    }
    let vorlaeufig = mit_block(&wav, &probe);
    let hash = hash_ohne(&vorlaeufig, start as usize, laenge as usize);
    let fertig = manifest(&p, &hash, start, laenge, fuellung);
    if fertig.len() != probe.len() {
        return Err("C2PA: das Manifest aenderte beim zweiten Durchgang seine Groesse".into());
    }
    Ok(mit_block(&wav, &fertig))
}

/// Die Bloecke einer RIFF-Datei: (Kennung, Lage des Inhalts, Laenge).
fn bloecke(b: &[u8]) -> Vec<([u8; 4], usize, usize)> {
    let mut aus = Vec::new();
    let mut o = 12;
    while o + 8 <= b.len() {
        let n = u32::from_le_bytes(b[o + 4..o + 8].try_into().expect("vier")) as usize;
        aus.push((b[o..o + 4].try_into().expect("vier"), o + 8, n.min(b.len().saturating_sub(o + 8))));
        o += 8 + n + (n & 1);
    }
    aus
}

/// Was die Pruefung des Manifests ergibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Herkunft {
    /// Kein Block `C2PA`.
    Fehlt,
    /// Signatur, Bezuege und Hash stimmen; KI-erzeugt laut Aktion.
    Gueltig,
    /// Ein Manifest liegt, aber etwas stimmt nicht.
    Ungueltig(String),
}

/// **Prueft das Manifest einer Datei**: Signatur ueber den Anspruch mit dem
/// Schluessel aus dem Zertifikat, die Hashes der Behauptungen, der Hash
/// ueber die Datei, und dass die Aktion `c2pa.created` mit dem Quellentyp
/// KI-erzeugt lautet. ⚠️ Ob dem Signierer zu trauen ist, prueft sie nicht.
pub(crate) fn pruefen(datei: &[u8]) -> Herkunft {
    match pruefen_mit_grund(datei) {
        Ok(None) => Herkunft::Fehlt,
        Ok(Some(())) => Herkunft::Gueltig,
        Err(f) => Herkunft::Ungueltig(f),
    }
}

fn pruefen_mit_grund(datei: &[u8]) -> Result<Option<()>, String> {
    use ed25519_dalek::Verifier;
    let Some(&(_, ab, n)) = bloecke(datei).iter().find(|(id, _, _)| id == b"C2PA") else { return Ok(None) };
    let ablage = &datei[ab..ab + n];
    let (_, store) = kaesten(ablage)?.into_iter().next().ok_or("C2PA: leere Ablage")?;
    let (name, inhalte) = zerlegen(store)?;
    if name != "c2pa" {
        return Err(format!("C2PA: Ablage heisst {name}"));
    }
    let (_, manifest) = inhalte.into_iter().find(|(a, _)| a == b"jumb").ok_or("C2PA: kein Manifest")?;
    let (_, teile) = zerlegen(manifest)?;
    let mut behauptungen = std::collections::BTreeMap::new();
    let (mut anspruch, mut cose) = (None, None);
    for (_, t) in teile.iter().filter(|(a, _)| a == b"jumb") {
        let (name, innen) = zerlegen(t)?;
        let cbor = || innen.iter().find(|(a, _)| a == b"cbor").map(|(_, c)| *c);
        match name.as_str() {
            "c2pa.assertions" => {
                for (_, b) in innen.iter().filter(|(a, _)| a == b"jumb") {
                    let (bname, binnen) = zerlegen(b)?;
                    let c = binnen.iter().find(|(a, _)| a == b"cbor").map(|(_, c)| *c).ok_or("C2PA: Behauptung ohne CBOR")?;
                    behauptungen.insert(bname, (Sha256::digest(b).to_vec(), Cbor::lesen(c, &mut 0)?));
                }
            }
            "c2pa.claim.v2" => anspruch = cbor(),
            "c2pa.signature" => cose = cbor(),
            _ => {}
        }
    }
    let anspruch_bytes = anspruch.ok_or("C2PA: kein Anspruch")?;
    let Cbor::Marke(18, liste) = Cbor::lesen(cose.ok_or("C2PA: keine Signatur")?, &mut 0)? else { return Err("C2PA: keine COSE_Sign1".into()) };
    let Cbor::Liste(l) = *liste else { return Err("C2PA: COSE ohne Liste".into()) };
    let [Cbor::Bytes(geschuetzt), _, Cbor::Nichts, Cbor::Bytes(signatur)] = l.as_slice() else { return Err("C2PA: COSE falsch geformt".into()) };
    let kopf = Cbor::lesen(geschuetzt, &mut 0)?;
    let Cbor::Karte(k) = &kopf else { return Err("C2PA: geschuetzter Kopf keine Karte".into()) };
    if !k.iter().any(|(a, b)| *a == Cbor::Zahl(1) && *b == Cbor::Negativ(7)) {
        return Err("C2PA: nicht EdDSA".into());
    }
    let zert = k.iter().find(|(a, _)| *a == Cbor::Zahl(33)).map(|(_, b)| b).ok_or("C2PA: kein x5chain")?;
    let zert = match zert {
        Cbor::Bytes(z) => z.clone(),
        Cbor::Liste(z) => match z.first() {
            Some(Cbor::Bytes(z)) => z.clone(),
            _ => return Err("C2PA: x5chain leer".into()),
        },
        _ => return Err("C2PA: x5chain falsch geformt".into()),
    };
    let schluessel = ed25519_dalek::VerifyingKey::from_bytes(&oeffentlicher_schluessel(&zert).ok_or("C2PA: kein Ed25519-Schluessel im Zertifikat")?).map_err(|_| "C2PA: Schluessel ungueltig")?;
    let zu_pruefen = Cbor::Liste(vec![Cbor::text("Signature1"), Cbor::Bytes(geschuetzt.clone()), Cbor::Bytes(Vec::new()), Cbor::Bytes(anspruch_bytes.to_vec())]).kodieren();
    let sig = ed25519_dalek::Signature::from_slice(signatur).map_err(|_| "C2PA: Signatur falsch lang")?;
    schluessel.verify(&zu_pruefen, &sig).map_err(|_| "C2PA: Signatur stimmt nicht".to_string())?;
    // Die Bezuege des Anspruchs.
    let anspruch = Cbor::lesen(anspruch_bytes, &mut 0)?;
    let Some(Cbor::Liste(bezuege)) = anspruch.feld("created_assertions") else { return Err("C2PA: keine Behauptungen im Anspruch".into()) };
    for b in bezuege {
        let (Some(Cbor::Text(url)), Some(Cbor::Bytes(h))) = (b.feld("url"), b.feld("hash")) else { return Err("C2PA: Bezug falsch geformt".into()) };
        let name = url.rsplit('/').next().unwrap_or("");
        let (eigen, _) = behauptungen.get(name).ok_or(format!("C2PA: Behauptung {name} fehlt"))?;
        if eigen != h {
            return Err(format!("C2PA: Behauptung {name} veraendert"));
        }
    }
    // Der Hash ueber die Datei.
    let (_, bindung) = behauptungen.get("c2pa.hash.data").ok_or("C2PA: keine Bindung an den Ton")?;
    let Some(Cbor::Bytes(soll)) = bindung.feld("hash") else { return Err("C2PA: Bindung ohne Hash".into()) };
    let Some(Cbor::Liste(aus)) = bindung.feld("exclusions") else { return Err("C2PA: Bindung ohne Ausnahme".into()) };
    let (Some(Cbor::Zahl(s)), Some(Cbor::Zahl(l))) = (aus.first().and_then(|a| a.feld("start")), aus.first().and_then(|a| a.feld("length"))) else { return Err("C2PA: Ausnahme falsch geformt".into()) };
    // ⚑ Wie `c2pa-rs`: Die Ausnahme ist genau der Block mit Kopf, nicht mehr
    //   und nicht weniger (auch kein Fuellbyte).
    if (*s as usize, *l as usize) != (ab - 8, n + 8) {
        return Err(format!("C2PA: die Ausnahme ({s}, {l}) ist nicht der Block ({}, {})", ab - 8, n + 8));
    }
    if (*s + *l) as usize > datei.len() || hash_ohne(datei, *s as usize, *l as usize) != *soll {
        return Err("C2PA: der Ton passt nicht zum Manifest".into());
    }
    // KI-erzeugt laut Aktion.
    let (_, aktionen) = behauptungen.get("c2pa.actions.v2").ok_or("C2PA: keine Aktionen")?;
    let ki = matches!(aktionen.feld("actions"), Some(Cbor::Liste(a)) if a.iter().any(|x| x.feld("action") == Some(&Cbor::text("c2pa.created")) && x.feld("digitalSourceType") == Some(&Cbor::text(super::QUELLENTYP))));
    if !ki {
        return Err("C2PA: keine Aktion `c2pa.created` mit KI-Quellentyp".into());
    }
    Ok(Some(()))
}

#[cfg(test)]
mod proben {
    use super::*;

    fn wav(n: usize) -> Vec<u8> {
        let proben: Vec<i16> = (0..n).map(|i| ((i % 100) as i16 - 50) * 100).collect();
        super::super::schreiben(24_000, 1, &proben)
    }

    /// ⚑ **Signiert ist gueltig, und jede Aenderung faellt auf**: ein
    /// geaenderter Abtastwert, ein geaendertes Byte im Anspruch, eine
    /// fremde Signatur. Gegenprobe: ohne Manifest `Fehlt`.
    #[test]
    fn signiert_ist_gueltig_und_aenderungen_fallen_auf() {
        let roh = wav(4_800);
        assert_eq!(pruefen(&roh), Herkunft::Fehlt);
        let s = signieren(roh.clone()).expect("signiert");
        assert_eq!(pruefen(&s), Herkunft::Gueltig, "frisch signiert");
        // Ein Abtastwert geaendert (im Datenblock, vor dem Manifest).
        let mut t = s.clone();
        let i = t.windows(4).position(|w| w == b"data").expect("data") + 100;
        t[i] ^= 1;
        assert!(matches!(pruefen(&t), Herkunft::Ungueltig(f) if f.contains("Ton")), "Ton geaendert");
        // Ein Byte im Anspruch geaendert: im Wert von `instanceID`, das nur
        // dort steht.
        let mut t = s.clone();
        let i = t.windows(10).position(|w| w == b"instanceID").expect("Anspruch") + 20;
        t[i] ^= 1;
        assert!(matches!(pruefen(&t), Herkunft::Ungueltig(f) if f.contains("Signatur")), "Anspruch geaendert");
        // Zweimal signiert wird abgelehnt.
        assert!(signieren(s.clone()).is_err());
    }

    /// ⛔️ **Ein praepariertes Manifest wird abgelehnt, nicht abgestuerzt**:
    /// 100 000 geschachtelte Listen in einer Behauptung (vorher Abbruch des
    /// Prozesses am Stapel) und eine Bytefolge mit der Laenge 2⁶⁴ − 1
    /// (vorher Ueberlauf). 64 Ebenen lesen sich weiter.
    #[test]
    fn ein_praepariertes_manifest_wird_abgelehnt() {
        let mut bombe = vec![0x81u8; 100_000];
        bombe.push(0);
        assert!(matches!(Cbor::lesen(&bombe, &mut 0), Err(f) if f.contains("tiefer")));
        let mut tief = vec![0x81u8; CBOR_TIEFE_HOECHSTENS];
        tief.push(0);
        assert!(Cbor::lesen(&tief, &mut 0).is_ok(), "{CBOR_TIEFE_HOECHSTENS} Ebenen");
        let mut lang = vec![0x5Bu8];
        lang.extend([0xFF; 8]);
        assert!(matches!(Cbor::lesen(&lang, &mut 0), Err(f) if f.contains("zu kurz")));

        let behauptung = ueberkasten(b"cbor", "c2pa.hash.data", &[kasten(b"cbor", &bombe)]);
        let behauptungen = ueberkasten(b"c2as", "c2pa.assertions", &[behauptung]);
        let manifest = ueberkasten(b"c2ma", "urn:c2pa:x", &[behauptungen]);
        let ablage = ueberkasten(b"c2pa", "c2pa", &[manifest]);
        let datei = mit_block(&wav(100), &ablage);
        assert!(matches!(pruefen(&datei), Herkunft::Ungueltig(f) if f.contains("tiefer")));
    }

    /// ⚑ **Der Block ist gerade lang und der ausgenommene Bereich genau Kopf
    /// und Inhalt**, bei verschiedenen Laengen der Datei (die Lage aendert
    /// die Groesse ihrer CBOR-Zahl). Gegenprobe: Die Groessen des Manifests
    /// waren dabei auch ungerade, sonst saehe die Probe nichts.
    #[test]
    fn der_block_ist_gerade_und_genau_ausgenommen() {
        let mut ungerade_ohne_fuellung = false;
        for n in [10, 101, 4_800, 40_000, 70_001] {
            let s = signieren(wav(n)).expect("signiert");
            let (_, ab, laenge) = *bloecke(&s).iter().find(|(id, _, _)| id == b"C2PA").expect("Block");
            assert_eq!(laenge % 2, 0, "{n}: ungerader Block");
            assert_eq!(ab + laenge, s.len(), "{n}: etwas nach dem Block");
            let Herkunft::Gueltig = pruefen(&s) else { panic!("{n}: ungueltig") };
            let ablage = &s[ab..];
            let (_, store) = kaesten(ablage).unwrap().into_iter().next().unwrap();
            let roh = String::from_utf8_lossy(store);
            assert!(roh.contains("jumbf manifest"));
            // Ohne Fuellung waere das Manifest bei manchen Laengen ungerade.
            for z in [300, 301] {
                let p = Bauplan { schluessel: ed25519_dalek::SigningKey::from_bytes(&[7; 32]), zertifikat: vec![0; z], manifest: "urn:c2pa:x".into(), instanz: "x".into() };
                ungerade_ohne_fuellung |= manifest(&p, &[0; 32], (ab - 8) as u64, (laenge + 8) as u64, 0).len() % 2 == 1;
            }
        }
        assert!(ungerade_ohne_fuellung, "nie ungerade: die Fuellung wurde nicht geprueft");
    }

    /// ⚑ **Je Datei ein frischer Schluessel**: zwei Signaturen derselben
    /// Datei tragen verschiedene Zertifikate, also verknuepft nichts sie.
    #[test]
    fn je_datei_ein_frischer_schluessel() {
        let roh = wav(2_400);
        let (a, b) = (signieren(roh.clone()).unwrap(), signieren(roh).unwrap());
        let zert = |d: &[u8]| oeffentlicher_schluessel(&d[d.windows(4).position(|w| w == b"C2PA").unwrap()..]).unwrap();
        assert_ne!(zert(&a), zert(&b));
        assert_eq!(pruefen(&a), Herkunft::Gueltig);
        assert_eq!(pruefen(&b), Herkunft::Gueltig);
    }

    /// ⚑ **Das Zertifikat besteht das Profil von `c2pa-rs`**, soweit es
    /// sich ohne X.509-Leser pruefen laesst: Version 3, Ed25519 ohne
    /// Parameter, Schluesselgebrauch, erweiterte Verwendung mit
    /// `c2pa-kp-claimSigning`, Schluessel-ID **und Aussteller-Schluessel-ID**
    /// (`2.5.29.35`), **Organisation im Namen** (`2.5.4.10`; ohne sie
    /// `CoseX5ChainMissing` in `c2pa-rs` 0.90), selbst signiert. Gegenprobe:
    /// Die Unterschrift stimmt mit dem eigenen Schluessel, mit einem anderen
    /// nicht.
    #[test]
    fn das_zertifikat_traegt_was_c2pa_rs_verlangt() {
        use ed25519_dalek::Verifier;
        let schluessel = ed25519_dalek::SigningKey::from_bytes(&[9; 32]);
        let fremd = ed25519_dalek::SigningKey::from_bytes(&[8; 32]);
        let z = zertifikat(&schluessel, &[1; 16]);
        let enthaelt = |teil: &[u8]| z.windows(teil.len()).any(|w| w == teil);
        assert!(enthaelt(&der(0xA0, &der(0x02, &[2]))), "Version 3");
        assert!(enthaelt(&folge(&[oid(&ED25519)])), "Ed25519 ohne Parameter");
        for id in [&[2u64, 5, 29, 15][..], &[2, 5, 29, 37], &[2, 5, 29, 14], &[2, 5, 29, 35], &[1, 3, 6, 1, 4, 1, 62558, 2, 1]] {
            assert!(enthaelt(&oid(id)), "fehlt: {id:?}");
        }
        let kennung = |k: &ed25519_dalek::SigningKey| Sha256::digest(k.verifying_key().to_bytes())[..20].to_vec();
        assert!(enthaelt(&der(0x04, &kennung(&schluessel))), "Schluessel-ID");
        assert!(enthaelt(&folge(&[der(0x80, &kennung(&schluessel))])), "Aussteller-Schluessel-ID gleich der Schluessel-ID");
        let organisation = der(0x31, &folge(&[oid(&[2, 5, 4, 10]), der(0x0C, b"Myelith")]));
        assert_eq!(z.windows(organisation.len()).filter(|w| *w == organisation.as_slice()).count(), 2, "O in Aussteller und Inhaber");
        assert_eq!(oeffentlicher_schluessel(&z), Some(schluessel.verifying_key().to_bytes()));
        // Zertifikat = Folge aus tbs, Verfahren, Unterschrift; alles ueber
        // 255 Byte, also Koepfe zu vier Byte.
        let tbs = &z[4..4 + 4 + u16::from_be_bytes([z[6], z[7]]) as usize];
        let unterschrift = ed25519_dalek::Signature::from_slice(&z[z.len() - 64..]).unwrap();
        assert!(schluessel.verifying_key().verify(tbs, &unterschrift).is_ok(), "selbst signiert");
        assert!(fremd.verifying_key().verify(tbs, &unterschrift).is_err(), "fremder Schluessel");
    }

    /// ⚑ **`x5chain` ist eine Liste**, auch mit einem Zertifikat: Schluessel
    /// 33, dann ein Feld mit einem Eintrag (`0x81`). Gegenprobe: Die
    /// einzelne Form (Schluessel 33, gleich die Bytes), an der `c2pa-rs` 0.90
    /// scheiterte, kommt nicht vor.
    #[test]
    fn die_kette_ist_eine_liste() {
        let s = signieren(wav(2_400)).expect("signiert");
        let enthaelt = |teil: &[u8]| s.windows(teil.len()).any(|w| w == teil);
        assert!(enthaelt(&[0x18, 0x21, 0x81, 0x59]), "x5chain als Liste");
        assert!(!enthaelt(&[0x18, 0x21, 0x59]), "x5chain einzeln");
        assert_eq!(pruefen(&s), Herkunft::Gueltig);
    }

    /// CBOR ist deterministisch: Schluessel nach Bytes geordnet, kuerzeste
    /// Koepfe; und liest zurueck, was es schreibt.
    #[test]
    fn cbor_deterministisch_und_umkehrbar() {
        let w = Cbor::karte(vec![("zz", Cbor::Zahl(1)), ("a", Cbor::Zahl(500)), ("bb", Cbor::Negativ(7))]);
        let k = w.kodieren();
        assert_eq!(k, vec![0xA3, 0x61, b'a', 0x19, 0x01, 0xF4, 0x62, b'b', b'b', 0x27, 0x62, b'z', b'z', 0x01]);
        let zurueck = Cbor::lesen(&k, &mut 0).unwrap();
        assert_eq!(zurueck.kodieren(), k);
        assert_eq!(datum(0), (1970, 1, 1));
        assert_eq!(datum(20_768), (2026, 11, 11));
    }
}
