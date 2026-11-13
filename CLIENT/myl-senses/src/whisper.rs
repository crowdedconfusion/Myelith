//! **whisper large-v3-turbo**, ohne fremde Laufzeit (CLIENT 8.2): aus dem
//! Log-Mel ([`crate::spektrum::log_mel`]) ueber Encoder und Decoder zum Text.
//!
//! ⚑ **Zwei Arten von Gewichten, ein Rechenweg.** Ausgeliefert wird
//! [`Gewichte::laden`]: Die Matrizen rechnen in int8 ueber die Kerne des
//! Sprachmodells (`integer_llm_kernels`), auf macOS gebuendelt mit Metal.
//! [`Gewichte::laden_genau`] behaelt `f32` und ist die Referenz, die die
//! Probe Stufe fuer Stufe mit `transformers` vergleicht. Alles ausser den
//! Matrizen ist fuer beide dieselbe Zeile Code. ⚑ **Erst richtig, dann
//! schnell**: Der genaue Weg kam zuerst, der ganzzahlige wird an ihm
//! gemessen.
//!
//! # Was gerechnet wird (whisper large-v3-turbo, Vorlage `transformers`)
//!
//! 1. Faltung 128 → 1280, Kern 3, Rand 1, GELU; Faltung 1280 → 1280, Kern 3,
//!    Rand 1, Schritt 2, GELU: aus 3000 Fenstern werden 1500.
//! 2. Dazu die gelernte Lageeinbettung.
//! 3. 32 Ebenen: `x += Aufmerksamkeit(LN(x))`, `x += fc2(GELU(fc1(LN(x))))`,
//!    20 Koepfe zu 64, `k` ohne Verschiebung.
//! 4. LayerNorm am Ende.
//!
//! Die Gewichte kommen aus der Safetensors-Datei des Originals (f16 oder
//! f32); der Leser hier liest nur, was Encoder und Decoder brauchen. Die
//! Masse stehen in `config.json` daneben ([`Masse`]), die Sondertoken in
//! `added_tokens.json` ([`Sondertoken`]): Ein Weg rechnet jede Groesse von
//! whisper, von tiny bis large-v3-turbo.

use std::collections::BTreeMap;
use std::path::Path;

/// **Die Masse eines whisper**, aus `config.json`.
///
/// ⚑ Bis 0.23.0 standen die Masse von large-v3-turbo fest im Code. Fuer
/// schwache Hardware braucht es die kleinen Groessen (Festlegung des
/// Projektinhabers, 2026-11-11: die Sinne nur lokal, auch auf 4 Kernen mit
/// 4 GB), und die unterscheiden sich in jedem Mass ausser der Kopfbreite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Masse {
    /// Breite von Encoder und Decoder (`d_model`).
    pub breite: usize,
    /// Koepfe der Aufmerksamkeit.
    pub koepfe: usize,
    /// Ebenen des Encoders.
    pub ebenen_enc: usize,
    /// Ebenen des Decoders.
    pub ebenen_dec: usize,
    /// Mel-Baender am Eingang (80, ab large-v3 128).
    pub baender: usize,
    /// Fenster nach der zweiten Faltung.
    pub fenster: usize,
    /// Stellen des Decoders, Vorspann eingeschlossen.
    pub stellen: usize,
}

impl Masse {
    /// Liest `config.json`.
    pub fn laden(datei: &Path) -> Result<Self, String> {
        let roh = std::fs::read(datei).map_err(|f| format!("{}: {f}", datei.display()))?;
        let Json::Objekt(o) = json(&roh, &mut 0)? else { return Err("config.json ist kein Objekt".into()) };
        let zahl = |k: &str| match o.get(k) {
            Some(Json::Zahl(z)) if *z >= 1.0 => Ok(*z as usize),
            _ => Err(format!("config.json: {k} fehlt")),
        };
        let m = Self {
            breite: zahl("d_model")?,
            koepfe: zahl("encoder_attention_heads")?,
            ebenen_enc: zahl("encoder_layers")?,
            ebenen_dec: zahl("decoder_layers")?,
            baender: zahl("num_mel_bins")?,
            fenster: zahl("max_source_positions")?,
            stellen: zahl("max_target_positions")?,
        };
        if m.breite != m.koepfe * KOPFBREITE || zahl("decoder_attention_heads")? != m.koepfe {
            return Err(format!("{} Breite bei {} Koepfen: dieser Weg rechnet nur Koepfe zu {KOPFBREITE}", m.breite, m.koepfe));
        }
        Ok(m)
    }
}

/// **Die Sondertoken**, aus `added_tokens.json`. ⚠️ Sie liegen nicht ueberall
/// gleich: large-v3 kennt eine Sprache mehr als die aelteren Groessen, und
/// alles hinter den Sprachen ruecken um eins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sondertoken {
    pub uebersetzen: u32,
    pub mitschreiben: u32,
    pub ohne_zeiten: u32,
}

impl Sondertoken {
    pub fn laden(datei: &Path) -> Result<Self, String> {
        let roh = std::fs::read(datei).map_err(|f| format!("{}: {f}", datei.display()))?;
        let Json::Objekt(o) = json(&roh, &mut 0)? else { return Err("added_tokens.json ist kein Objekt".into()) };
        let token = |k: &str| match o.get(k) {
            Some(Json::Zahl(z)) => Ok(*z as u32),
            _ => Err(format!("added_tokens.json: {k} fehlt")),
        };
        // Die aelteren Groessen fuehren `<|endoftext|>` nicht hier, sondern im
        // Woerterbuch; steht es hier, muss es an seiner Stelle stehen.
        let ende_passt = token("<|endoftext|>").map_or(true, |e| e == ENDE);
        if !ende_passt || token("<|startoftranscript|>")? != ANFANG {
            return Err("added_tokens.json: Ende und Anfang liegen anders als bei whisper".into());
        }
        Ok(Self { uebersetzen: token("<|translate|>")?, mitschreiben: token("<|transcribe|>")?, ohne_zeiten: token("<|notimestamps|>")? })
    }
}

// ---- Safetensors ---------------------------------------------------------

/// Ein Tensor aus der Datei: Form und Werte in `f32`.
#[derive(Debug, Clone)]
pub struct Tensor {
    pub form: Vec<usize>,
    pub werte: Vec<f32>,
}

/// Ein Wert des JSON-Kopfs, so weit Safetensors ihn braucht.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Text(String),
    Zahl(f64),
    Liste(Vec<Json>),
    Objekt(BTreeMap<String, Json>),
    Sonst,
}

/// So tief darf JSON hier verschachtelt sein. Echte Dateien (Safetensors-Kopf,
/// `config.json`, `tokenizer.json`) liegen bei hoechstens fuenf Ebenen.
const JSON_TIEFE_HOECHSTENS: usize = 128;

/// Liest einen JSON-Wert ab `i`.
///
/// 📌 Gefunden am 2026-11-12: Ohne Tiefengrenze sprengten 100 000 `[` den
/// Stapel, und das ist keine Panik, sondern ein Abbruch des ganzen
/// Prozesses. Gelesen werden auch Dateien aus eigenen Gewichtsordnern, die
/// keine Pruefsumme kennt.
pub(crate) fn json(b: &[u8], i: &mut usize) -> Result<Json, String> {
    json_in(b, i, 0)
}

fn json_in(b: &[u8], i: &mut usize, tiefe: usize) -> Result<Json, String> {
    if tiefe > JSON_TIEFE_HOECHSTENS {
        return Err(format!("JSON tiefer als {JSON_TIEFE_HOECHSTENS} Ebenen"));
    }
    let leer = |i: &mut usize| {
        while *i < b.len() && b[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    leer(i);
    let c = *b.get(*i).ok_or("JSON endet zu frueh")?;
    match c {
        b'{' => {
            *i += 1;
            let mut o = BTreeMap::new();
            loop {
                leer(i);
                if b.get(*i) == Some(&b'}') {
                    *i += 1;
                    return Ok(Json::Objekt(o));
                }
                let Json::Text(k) = json_in(b, i, tiefe + 1)? else { return Err("Schluessel ist kein Text".into()) };
                leer(i);
                if b.get(*i) != Some(&b':') {
                    return Err("':' fehlt".into());
                }
                *i += 1;
                let v = json_in(b, i, tiefe + 1)?;
                o.insert(k, v);
                leer(i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b'}') => {}
                    _ => return Err("',' oder '}' fehlt".into()),
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut l = Vec::new();
            loop {
                leer(i);
                if b.get(*i) == Some(&b']') {
                    *i += 1;
                    return Ok(Json::Liste(l));
                }
                l.push(json_in(b, i, tiefe + 1)?);
                leer(i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b']') => {}
                    _ => return Err("',' oder ']' fehlt".into()),
                }
            }
        }
        b'"' => {
            *i += 1;
            let mut s = Vec::new();
            while *i < b.len() && b[*i] != b'"' {
                if b[*i] == b'\\' {
                    *i += 1;
                    match b.get(*i) {
                        Some(b'u') => {
                            let hex = b.get(*i + 1..*i + 5).ok_or("\\u endet zu frueh")?;
                            let z = u32::from_str_radix(std::str::from_utf8(hex).map_err(|_| "\\u")?, 16).map_err(|_| "\\u")?;
                            let mut puffer = [0u8; 4];
                            s.extend_from_slice(char::from_u32(z).unwrap_or('\u{FFFD}').encode_utf8(&mut puffer).as_bytes());
                            *i += 5;
                            continue;
                        }
                        Some(b'n') => s.push(b'\n'),
                        Some(b't') => s.push(b'\t'),
                        Some(b'r') => s.push(b'\r'),
                        Some(&c) => s.push(c),
                        None => return Err("Text endet zu frueh".into()),
                    }
                    *i += 1;
                    continue;
                }
                s.push(b[*i]);
                *i += 1;
            }
            *i += 1;
            Ok(Json::Text(String::from_utf8_lossy(&s).into_owned()))
        }
        b'-' | b'0'..=b'9' => {
            let a = *i;
            while *i < b.len() && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                *i += 1;
            }
            std::str::from_utf8(&b[a..*i]).ok().and_then(|t| t.parse().ok()).map(Json::Zahl).ok_or_else(|| "keine Zahl".into())
        }
        _ => {
            while *i < b.len() && b[*i].is_ascii_alphabetic() {
                *i += 1;
            }
            Ok(Json::Sonst)
        }
    }
}

/// f16 nach f32, genau.
pub fn halb_zu_f32(h: u16) -> f32 {
    let vorzeichen = u32::from(h >> 15) << 31;
    let exp = u32::from((h >> 10) & 0x1F);
    let mant = u32::from(h & 0x3FF);
    let bits = match (exp, mant) {
        (0, 0) => vorzeichen,
        (0, m) => {
            // Subnormal: normalisieren.
            let mut e = 127 - 15 + 1;
            let mut m = m;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            vorzeichen | (e << 23) | ((m & 0x3FF) << 13)
        }
        (0x1F, m) => vorzeichen | 0x7F80_0000 | (m << 13),
        (e, m) => vorzeichen | ((e + 127 - 15) << 23) | (m << 13),
    };
    f32::from_bits(bits)
}

/// **Eine Matrix in zwei int8-Stellen**, je Zeile eine Zweierpotenz als
/// Skala: dieselbe Form wie die Gewichte des Sprachmodells, damit dieselben
/// Kerne sie rechnen (`integer_llm_kernels::linear`, auf macOS ab 16
/// Eingaben mit Metal).
///
/// ⚑ **Warum zwei Stellen.** Eine Stelle rundet jedes Gewicht auf 2⁻⁷ bis
/// 2⁻⁸ seines Zeilenbetrags; gemessen wich der Encoder damit um 4 Prozent
/// von `transformers` ab, bei gleichen Token. Die zweite Stelle traegt den
/// Rest der ersten mal 256 (`w = (h + t/256) · 2⁻ˢ`), derselbe Kniff, mit dem
/// der Metal-Weg der Kernkiste int16 in zwei int8 zerlegt, und bringt den
/// ganzzahligen Weg an die geprüfte Referenz heran.
///
/// ⚑ **Beide Stellen in einer Matrix, untereinander** (Zeilen `0..aus` die
/// erste, `aus..2·aus` der Rest mit Verschiebung `s + 8`): Verschiebung und
/// Ausgabebits gelten ohnehin je Zeile. 📌 Als zwei Aufrufe zerlegte die
/// Kernkiste jede Eingabe zweimal auf einem Kern in zwei int8 und schickte
/// zwei Befehlspuffer; gemessen 2,26 s Kernzeit je Encoder.
pub(crate) struct Ganzmatrix {
    pub(crate) aus: usize,
    ein: usize,
    stellen: Stelle,
    /// Die Ausgabebits fuer W8A8, das nur die erste Stelle rechnet.
    bits_a8: Ausgabebits,
}

/// Die Stellen: int8 je Gewicht, eine Zweierpotenz je Zeile.
struct Stelle {
    w: Vec<i8>,
    schiebe: Vec<u8>,
    bits: Ausgabebits,
}

/// Nachkommabits der Ausgabe je Kanal beim letzten Aufruf; der naechste
/// beginnt dort (siehe [`einstellen`]).
struct Ausgabebits {
    je_kanal: std::sync::Mutex<Vec<u8>>,
    /// Wurde schon einmal gerechnet?
    eingestellt: std::sync::atomic::AtomicBool,
}

impl Ausgabebits {
    fn neu(kanaele: usize) -> Self {
        Self { je_kanal: std::sync::Mutex::new(vec![EINGABE_BITS; kanaele]), eingestellt: false.into() }
    }
}

/// Groesste Zweierpotenz `s` mit `hoechst · 2^s ≤ grenze`, hoechstens `deckel`.
fn bits_fuer(hoechst: f32, grenze: f32, deckel: u8) -> u8 {
    let mut s = 0u8;
    while s < deckel && hoechst * 2f32.powi(i32::from(s) + 1) <= grenze {
        s += 1;
    }
    s
}

impl Ganzmatrix {
    pub(crate) fn aus_f32(werte: &[f32], aus: usize) -> Self {
        let ein = werte.len() / aus;
        let (mut hoch, mut tief) = (Vec::with_capacity(2 * werte.len()), Vec::with_capacity(werte.len()));
        let mut schiebe = Vec::with_capacity(2 * aus);
        for zeile in werte.chunks(ein) {
            let hoechst = zeile.iter().fold(0f32, |m, v| m.max(v.abs()));
            assert!(hoechst <= 127.0, "Gewicht {hoechst} passt nicht in int8 ohne Teiler");
            let s = bits_fuer(hoechst, 127.0, 24);
            let f = 2f64.powi(i32::from(s));
            for v in zeile {
                let genau = f64::from(*v) * f;
                let h = genau.round().clamp(-127.0, 127.0);
                hoch.push(h as i8);
                tief.push(((genau - h) * 256.0).round().clamp(-127.0, 127.0) as i8);
            }
            schiebe.push(s);
        }
        let tiefe: Vec<u8> = schiebe.iter().map(|s| s + 8).collect();
        schiebe.extend(tiefe);
        hoch.extend(tief);
        let stellen = Stelle { w: hoch, schiebe, bits: Ausgabebits::neu(2 * aus) };
        Self { aus, ein, stellen, bits_a8: Ausgabebits::neu(aus) }
    }

    /// **Nur die erste Stelle**: int8 je Gewicht, eine Zweierpotenz je
    /// Zeile, wie die Gewichte des Sprachmodells (W8A16). Halb so viel
    /// Speicher und Bandbreite wie zwei Stellen; fuer den Sprecher von
    /// Qwen3-TTS, der Bild fuer Bild die ganze Matrix liest.
    pub(crate) fn eine_stelle(werte: &[f32], aus: usize) -> Self {
        let mut m = Self::aus_f32(werte, aus);
        m.stellen.w.truncate(aus * m.ein);
        m.stellen.w.shrink_to_fit();
        m.stellen.schiebe.truncate(aus);
        m.stellen.bits = Ausgabebits::neu(aus);
        m
    }

    /// Traegt die Matrix die zweite Stelle?
    fn zwei(&self) -> bool {
        self.stellen.schiebe.len() == 2 * self.aus
    }
}

/// Ein Tensor im Kopf der Safetensors-Datei: Name ohne `model.`, Form, Ort
/// der Bytes in der Datei, Bytes je Wert.
#[derive(Clone, Debug)]
pub(crate) struct Eintrag {
    pub(crate) name: String,
    pub(crate) form: Vec<usize>,
    pub(crate) ab: u64,
    pub(crate) laenge: usize,
    pub(crate) breit: usize,
    /// bfloat16 statt IEEE-Halbgenauigkeit (die Gewichte von Qwen3-TTS).
    pub(crate) bf16: bool,
}

/// **Nur den Kopf lesen**; die Tensoren holt [`tensor_lesen`] einzeln.
///
/// 📌 Bis 0.23.0 lag die ganze Datei im Speicher, daneben jeder Tensor in
/// `f32` und dann die int8-Matrizen: gemessen 2,9 GB Spitze fuer whisper
/// small auf 4 Kernen mit 4 GB, large-v3-turbo brach dort ab. Einzeln
/// gelesen liegt je Faden nur ein Tensor in `f32`.
pub(crate) fn kopf_lesen(datei: &Path, behalten: impl Fn(&str) -> Option<String>) -> Result<Vec<Eintrag>, String> {
    use std::io::Read;
    let fehler = |f: std::io::Error| format!("{}: {f}", datei.display());
    let mut f = std::fs::File::open(datei).map_err(fehler)?;
    let groesse = f.metadata().map_err(fehler)?.len();
    let mut acht = [0u8; 8];
    f.read_exact(&mut acht).map_err(|_| "keine Safetensors-Datei".to_string())?;
    let n = u64::from_le_bytes(acht);
    if n > 100 << 20 || 8 + n > groesse {
        return Err("Kopf ragt ueber die Datei".into());
    }
    let mut kopf = vec![0u8; n as usize];
    f.read_exact(&mut kopf).map_err(fehler)?;
    let Json::Objekt(o) = json(&kopf, &mut 0)? else { return Err("Kopf ist kein Objekt".into()) };
    let mut eintraege = Vec::new();
    for (name, v) in o {
        let Some(gemerkt) = behalten(&name) else { continue };
        let Json::Objekt(t) = v else { continue };
        let (Some(Json::Text(art)), Some(Json::Liste(form)), Some(Json::Liste(lage))) =
            (t.get("dtype"), t.get("shape"), t.get("data_offsets"))
        else {
            return Err(format!("{name}: unvollstaendiger Eintrag"));
        };
        let (breit, bf16): (usize, bool) = match art.as_str() {
            "F16" => (2, false),
            "BF16" => (2, true),
            "F32" => (4, false),
            _ => return Err(format!("{name}: {art} statt F16, BF16 oder F32")),
        };
        // ⛔️ **Jede Zahl aus dem Kopf ist fremd** (2026-11-13): ganz, nicht
        //   negativ und hoechstens so gross wie die Datei, und die Form
        //   multipliziert ohne Ueberlauf. 📌 Vorher brachte `data_offsets`
        //   mit einem Eintrag eine Panik am Index, und eine Lage von 1e30
        //   wurde `usize::MAX`: `8 + n + e` lief ueber, im Release-Bau
        //   unbemerkt, und `tensor_lesen` haette fast 2^64 Bytes verlangt.
        let zahl = |j: &Json| match j {
            Json::Zahl(z) if *z >= 0.0 && z.fract() == 0.0 && *z <= groesse as f64 => Some(*z as usize),
            _ => None,
        };
        let form: Vec<usize> = form.iter().map(zahl).collect::<Option<_>>().ok_or(format!("{name}: Form unlesbar"))?;
        let [a, e] = lage.as_slice() else { return Err(format!("{name}: data_offsets braucht zwei Zahlen")) };
        let (Some(a), Some(e)) = (zahl(a), zahl(e)) else { return Err(format!("{name}: data_offsets unlesbar")) };
        if e < a || (8 + n).checked_add(e as u64).is_none_or(|ende| ende > groesse) {
            return Err(format!("{name}: Daten ausserhalb"));
        }
        if form.iter().try_fold(breit, |p, &d| p.checked_mul(d)) != Some(e - a) {
            return Err(format!("{name}: Groesse passt nicht zur Form"));
        }
        eintraege.push(Eintrag { name: gemerkt, form, ab: 8 + n + a as u64, laenge: e - a, breit, bf16 });
    }
    Ok(eintraege)
}

/// Encoder und Decoder von whisper, abgelegt ohne das `model.`.
fn whisper_name(name: &str) -> Option<String> {
    (name.starts_with("model.encoder.") || name.starts_with("model.decoder.")).then(|| name.trim_start_matches("model.").to_string())
}

/// Einen Tensor von der Platte, nach `f32`.
pub(crate) fn tensor_lesen(datei: &Path, e: &Eintrag) -> Result<Tensor, String> {
    use std::io::{Read, Seek};
    let fehler = |f: std::io::Error| format!("{}: {}: {f}", datei.display(), e.name);
    let mut f = std::fs::File::open(datei).map_err(fehler)?;
    f.seek(std::io::SeekFrom::Start(e.ab)).map_err(fehler)?;
    let mut bytes = vec![0u8; e.laenge];
    f.read_exact(&mut bytes).map_err(fehler)?;
    let werte: Vec<f32> = if e.bf16 {
        // bfloat16 sind die oberen 16 Bit eines f32, also genau.
        bytes.as_chunks::<2>().0.iter().map(|p| f32::from_bits(u32::from(u16::from_le_bytes(*p)) << 16)).collect()
    } else if e.breit == 2 {
        bytes.as_chunks::<2>().0.iter().map(|p| halb_zu_f32(u16::from_le_bytes(*p))).collect()
    } else {
        bytes.as_chunks::<4>().0.iter().map(|p| f32::from_le_bytes(*p)).collect()
    };
    Ok(Tensor { form: e.form.clone(), werte })
}

/// **Eine Zeile eines Tensors** `[zeilen][breite]` von der Platte, ohne den
/// Rest zu lesen. Fuer Einbettungen, von denen je Satz nur wenige Zeilen
/// gebraucht werden.
pub(crate) fn zeile_lesen(datei: &Path, e: &Eintrag, zeile: usize) -> Result<Vec<f32>, String> {
    let Some((&zeilen, rest)) = e.form.split_first() else { return Err(format!("{}: kein Tensor mit Zeilen", e.name)) };
    let breite: usize = rest.iter().product();
    if zeile >= zeilen {
        return Err(format!("{}: Zeile {zeile} von {}", e.name, e.form[0]));
    }
    let teil = Eintrag { form: vec![1, breite], ab: e.ab + (zeile * breite * e.breit) as u64, laenge: breite * e.breit, ..e.clone() };
    tensor_lesen(datei, &teil).map(|t| t.werte)
}

/// **Die Gewichte von whisper**, aus der Safetensors-Datei gelesen.
///
/// ⚑ **Zwei Arten, ein Rechenweg drumherum.** [`Gewichte::laden`] legt jede
/// Matrix in int8 ([`Ganzmatrix`]) und rechnet sie ueber die Kerne des
/// Sprachmodells; [`Gewichte::laden_genau`] behaelt `f32` und ist die
/// Referenz, gegen die die Probe `transformers` vergleicht. Alles ausser den
/// Matrizen (LayerNorm, GELU, Softmax) ist fuer beide dieselbe Zeile Code.
pub struct Gewichte {
    tensoren: BTreeMap<String, Tensor>,
    ganz: BTreeMap<String, Ganzmatrix>,
    masse: Masse,
    zeichen: Sondertoken,
    /// Rechnen die Buendel W8A8 ([`Gewichte::laden`]) statt W8A16?
    acht: bool,
}

/// **`f` ueber alle Kerne**, je Faden das naechste freie Stueck; die
/// Reihenfolge des Ergebnisses ist die der Eingabe.
///
/// 📌 Auf einem Kern kostete das Laden 1,6 s je Aufruf von [`text`]: jeder
/// Tensor von f16 nach f32, jede Matrix in zwei Stellen.
pub(crate) fn alle_kerne<T: Send, R: Send>(stuecke: Vec<T>, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let n = stuecke.len();
    let eingang = std::sync::Mutex::new(stuecke.into_iter().enumerate());
    let mut aus: Vec<(usize, R)> = std::thread::scope(|s| {
        let faeden: Vec<_> = (0..kerne().min(n.max(1)))
            .map(|_| {
                s.spawn(|| {
                    let mut eigen = Vec::new();
                    loop {
                        let naechstes = eingang.lock().expect("Eingang").next();
                        let Some((i, x)) = naechstes else { break };
                        eigen.push((i, f(x)));
                    }
                    eigen
                })
            })
            .collect();
        faeden.into_iter().flat_map(|f| f.join().expect("Faden")).collect()
    });
    aus.sort_by_key(|(i, _)| *i);
    aus.into_iter().map(|(_, r)| r).collect()
}

/// Matrizen, die [`linear`] rechnet: Projektionen, Vorwaertsschicht, Faltungen.
fn ist_matrix(name: &str) -> bool {
    name.ends_with("proj.weight")
        || name.ends_with("fc1.weight")
        || name.ends_with("fc2.weight")
        || name.ends_with("conv1.weight")
        || name.ends_with("conv2.weight")
}

/// Der Kopf des Decoders ist die Tokeneinbettung (geteilte Gewichte).
const KOPF: &str = "decoder.embed_tokens.weight";

impl Gewichte {
    /// Liest die Gewichte und legt die Matrizen in int8; die Buendel
    /// rechnen W8A8 (Vorgabe, siehe [`linear_a8`]).
    pub fn laden(datei: &Path) -> Result<Self, String> {
        let mut g = Self::laden_w8a16(datei)?;
        g.acht = true;
        Ok(g)
    }

    /// Wie [`Self::laden`], die Buendel aber in W8A16: naeher an der
    /// Referenz (die Probe gegen `transformers` haelt ihn bei 10⁻³), mit der
    /// doppelten Arbeit auf der Grafikeinheit.
    pub fn laden_w8a16(datei: &Path) -> Result<Self, String> {
        enum Teil {
            Genau(String, Tensor),
            Ganz(String, Ganzmatrix),
            Beides(String, Tensor, Ganzmatrix),
        }
        let teile = alle_kerne(kopf_lesen(datei, whisper_name)?, |e| -> Result<Teil, String> {
            let t = tensor_lesen(datei, &e)?;
            Ok(if e.name == KOPF {
                let m = Ganzmatrix::aus_f32(&t.werte, t.form[0]);
                Teil::Beides(e.name, t, m)
            } else if ist_matrix(&e.name) {
                // Das `f32` der Matrix faellt hier weg, sobald sie zerlegt ist.
                Teil::Ganz(e.name, Ganzmatrix::aus_f32(&t.werte, t.form[0]))
            } else {
                Teil::Genau(e.name, t)
            })
        });
        let (mut tensoren, mut ganz) = (BTreeMap::new(), BTreeMap::new());
        for teil in teile {
            match teil? {
                Teil::Genau(n, t) => {
                    tensoren.insert(n, t);
                }
                Teil::Ganz(n, m) => {
                    ganz.insert(n, m);
                }
                Teil::Beides(n, t, m) => {
                    tensoren.insert(n.clone(), t);
                    ganz.insert(n, m);
                }
            }
        }
        Self::zusammen(datei, tensoren, ganz)
    }

    /// Liest alle Tensoren von Encoder und Decoder (`model.encoder.…`,
    /// `model.decoder.…`), abgelegt ohne das `model.`, und behaelt `f32`.
    pub fn laden_genau(datei: &Path) -> Result<Self, String> {
        let tensoren = alle_kerne(kopf_lesen(datei, whisper_name)?, |e| tensor_lesen(datei, &e).map(|t| (e.name, t)))
            .into_iter()
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        Self::zusammen(datei, tensoren, BTreeMap::new())
    }

    fn zusammen(datei: &Path, tensoren: BTreeMap<String, Tensor>, ganz: BTreeMap<String, Ganzmatrix>) -> Result<Self, String> {
        let ordner = datei.parent().unwrap_or(Path::new("."));
        let masse = Masse::laden(&ordner.join("config.json"))?;
        let zeichen = Sondertoken::laden(&ordner.join("added_tokens.json"))?;
        Ok(Self { tensoren, ganz, masse, zeichen, acht: false })
    }

    /// Die Masse dieses whisper.
    pub fn masse(&self) -> &Masse {
        &self.masse
    }

    /// Die Sondertoken dieses whisper.
    pub fn zeichen(&self) -> Sondertoken {
        self.zeichen
    }

    /// Rechnet dieser Satz Gewichte in int8?
    pub fn ganzzahlig(&self) -> bool {
        !self.ganz.is_empty()
    }

    fn t(&self, name: &str) -> &[f32] {
        &self.tensoren.get(name).unwrap_or_else(|| panic!("Gewicht {name} fehlt")).werte
    }
}

// ---- Rechnen -------------------------------------------------------------

/// erf nach Abramowitz und Stegun 7.1.26 in `f64` (Fehler unter 1,5·10⁻⁷).
fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t + 0.254_829_592)
            * t
            * (-x * x).exp();
    if x >= 0.0 {
        y
    } else {
        -y
    }
}

pub(crate) fn gelu(x: f32) -> f32 {
    let x = f64::from(x);
    (0.5 * x * (1.0 + erf(x / std::f64::consts::SQRT_2))) as f32
}

/// Skalarprodukt mit 16 unabhaengigen Summen: Eine einzige Kette liesse
/// sich nicht vektorisieren, denn `f32` ist nicht assoziativ.
pub(crate) fn skalar(a: &[f32], b: &[f32]) -> f32 {
    let mut s = [0f32; 16];
    let (ka, ra) = a.as_chunks::<16>();
    let (kb, rb) = b.as_chunks::<16>();
    for (x, y) in ka.iter().zip(kb) {
        for l in 0..16 {
            s[l] += x[l] * y[l];
        }
    }
    let rest: f32 = ra.iter().zip(rb).map(|(x, y)| x * y).sum();
    s.iter().sum::<f32>() + rest
}

/// GELU ueber alle Kerne.
pub(crate) fn gelu_alle(x: &mut [f32]) {
    let je = x.len().div_ceil(kerne());
    std::thread::scope(|s| {
        for teil in x.chunks_mut(je) {
            s.spawn(move || teil.iter_mut().for_each(|v| *v = gelu(*v)));
        }
    });
}

pub(crate) fn kerne() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

/// `y[t] = W · x[t] + b` fuer die Matrix `w` (zeilenweise `[aus][ein]`) und
/// die Verschiebung `b`, je nach Art der Gewichte in int8 oder `f32`.
fn linear(x: &[f32], zeilen: usize, g: &Gewichte, w: &str, b: Option<&str>) -> Vec<f32> {
    let (mut y, aus) = match g.ganz.get(w) {
        Some(m) if g.acht && zeilen >= GEBUENDELT_AB => (linear_a8(x, zeilen, m), m.aus),
        Some(m) => (linear_ganz(x, zeilen, m), m.aus),
        None => {
            let t = g.tensoren.get(w).unwrap_or_else(|| panic!("Gewicht {w} fehlt"));
            (linear_genau(x, zeilen, &t.werte, t.form[0]), t.form[0])
        }
    };
    if let Some(b) = b {
        let b = g.t(b);
        y.chunks_mut(aus).for_each(|z| z.iter_mut().zip(b).for_each(|(v, c)| *v += c));
    }
    y
}

/// **Die Matrix in int8 ueber die Kerne des Sprachmodells.**
///
/// ⚑ **Skalen dort, wo die Werte streuen.** Jedes Fenster geht mit seiner
/// eigenen Zweierpotenz nach int16, so fein, wie sein groesster Betrag es
/// erlaubt; die Ausgabe traegt je Kanal eigene Nachkommabits. Gemessen in
/// der ersten Ebene: Mit je einer Skala fuer alles kostete die Eingabe von
/// `fc2` (GELU bis 10 bei einem Effektivwert von 0,23) und die Ausgabe der
/// ersten Faltung je 5·10⁻⁴ bis 9·10⁻⁴ relativ.
///
/// Wie viele Nachkommabits ein Kanal braucht, weiss vorher niemand.
/// Begonnen wird mit denen des letzten Aufrufs; ist ein Wert geklemmt, wird
/// mit weniger neu gerechnet, nutzt ein Kanal zu wenig des Bereichs, mit
/// mehr (beim ersten Aufruf unter einem Viertel, danach unter einem
/// Sechzehntel). So traegt jede Ausgabe mindestens 11 Bit ihres Kanals, ohne
/// Kalibrierung, und bei aehnlichen Eingaben genuegt ein Durchgang.
pub(crate) fn linear_ganz(x: &[f32], zeilen: usize, m: &Ganzmatrix) -> Vec<f32> {
    let mut xq = vec![0i16; zeilen * m.ein];
    let mut bits = Vec::with_capacity(zeilen);
    for (xz, qz) in x.chunks(m.ein).zip(xq.chunks_mut(m.ein)) {
        let hoechst = xz.iter().fold(0f32, |a, v| a.max(v.abs()));
        // ⚑ Auch negative Exponenten: Im Sprecher von Qwen3-TTS liegen
        // einzelne Aktivierungen ueber 10⁵ (gemessen 124 883 vor einer
        // Projektion). Fuer whisper blieb jede Eingabe darunter, dort
        // aendert sich nichts.
        let mut a = i32::from(bits_fuer(hoechst, 32_767.0, EINGABE_BITS));
        while a > -i32::from(EINGABE_BITS) && hoechst * 2f32.powi(a) > 32_767.0 {
            a -= 1;
        }
        let f = 2f32.powi(a);
        qz.iter_mut().zip(xz).for_each(|(q, v)| *q = (v * f).round().clamp(-32_767.0, 32_767.0) as i16);
        bits.push(a);
    }
    if zeilen < GEBUENDELT_AB {
        return linear_ganz_einzeln(&xq, &bits, zeilen, m);
    }
    let eingaben: Vec<&[i16]> = xq.chunks(m.ein).take(zeilen).collect();
    let (teil, o) = stelle_mal(&eingaben, m.ein, &m.stellen);
    let kanal: Vec<f32> = o.iter().map(|&oz| 2f32.powi(-i32::from(oz))).collect();
    let mut y = vec![0f32; zeilen * m.aus];
    for ((yz, tz), &a) in y.chunks_mut(m.aus).zip(&teil).zip(&bits) {
        // Fenster `b` kam mit 2^a statt 2^EINGABE_BITS herein.
        let fenster = 2f32.powi(i32::from(EINGABE_BITS) - a);
        if m.zwei() {
            let (k_hoch, k_tief) = kanal.split_at(m.aus);
            let (hoch, tief) = tz.split_at(m.aus);
            for ((((z, &h), &l), kh), kl) in yz.iter_mut().zip(hoch).zip(tief).zip(k_hoch).zip(k_tief) {
                *z = (f32::from(h) * kh + f32::from(l) * kl) * fenster;
            }
        } else {
            for ((z, &h), kh) in yz.iter_mut().zip(tz).zip(&kanal) {
                *z = f32::from(h) * kh * fenster;
            }
        }
    }
    y
}

/// **W8A8 fuer die Buendel**: jedes Fenster mit seiner Zweierpotenz nach
/// int8, mal der ersten Stelle der Gewichte, ein `matmul2d` statt der
/// vier von W8A16 mit zwei Stellen (`integer_llm_kernels::linear::linear_w8a8_pc_stapel`).
///
/// ⚑ **Entschieden vom Projektinhaber am 2026-11-11, nach einem Versuch.**
/// Gegen `transformers` weicht der Encoder so um 10⁻¹ ab statt 5·10⁻⁴, der
/// erste Logit des Decoders um 7 Prozent; die Token und die Hoerprobe (16
/// Aufnahmen, kein Wortfehler) blieben gleich. Eine zweite Stelle brachte
/// bei 8-Bit-Eingaben nichts (1,1·10⁻¹), Skalen je 128er-Gruppe nur einen
/// Faktor zwei. Der Decoder rechnet Token fuer Token weiter exakt
/// ([`linear_ganz_einzeln`]).
pub(crate) fn linear_a8(x: &[f32], zeilen: usize, m: &Ganzmatrix) -> Vec<f32> {
    let mut xq = vec![0i8; zeilen * m.ein];
    let mut bits = vec![0i32; zeilen];
    // Je Zeile unabhaengig, also ueber die Kerne (der Rueckweg von Qwen3-TTS
    // rundet hier 67 200 Zeilen je Faltung).
    je_kern(&mut bits, 1, |r0, teil| {
        for (dr, a) in teil.iter_mut().enumerate() {
            // ⚑ Auch negative Exponenten: Anders als bei int16 liegen viele
            // Aktivierungen ueber 127.
            let hoechst = x[(r0 + dr) * m.ein..][..m.ein].iter().fold(0f32, |a, v| a.max(v.abs()));
            *a = i32::from(EINGABE_BITS);
            while *a > -i32::from(EINGABE_BITS) && hoechst * 2f32.powi(*a) > 127.0 {
                *a -= 1;
            }
        }
    });
    je_kern(&mut xq, m.ein, |r0, teil| {
        for (dr, qz) in teil.chunks_mut(m.ein).enumerate() {
            let f = 2f32.powi(bits[r0 + dr]);
            qz.iter_mut().zip(&x[(r0 + dr) * m.ein..][..m.ein]).for_each(|(q, v)| *q = (v * f).round().clamp(-127.0, 127.0) as i8);
        }
    });
    let eingaben: Vec<&[i8]> = xq.chunks(m.ein).take(zeilen).collect();
    let st = &m.stellen;
    let (w, schiebe) = (&st.w[..m.aus * m.ein], &st.schiebe[..m.aus]);
    let (teil, o) = einstellen(&m.bits_a8, |o| {
        integer_llm_kernels::linear::linear_w8a8_pc_stapel(&eingaben, w, m.ein, schiebe, EINGABE_BITS, o)
    });
    let kanal: Vec<f32> = o.iter().map(|&oz| 2f32.powi(-i32::from(oz))).collect();
    let mut y = vec![0f32; zeilen * m.aus];
    je_kern(&mut y, m.aus, |r0, stueck| {
        for (dr, yz) in stueck.chunks_mut(m.aus).enumerate() {
            let fenster = 2f32.powi(i32::from(EINGABE_BITS) - bits[r0 + dr]);
            for ((z, &v), k) in yz.iter_mut().zip(&teil[r0 + dr]).zip(&kanal) {
                *z = f32::from(v) * k * fenster;
            }
        }
    });
    y
}

/// **`f` ueber alle Kerne, je Faden ein zusammenhaengender Teil**: `daten`
/// in Stuecke zu `stueck` Werten; `f` bekommt den Index des ersten Stuecks
/// seines Teils und den Teil. Fuer Schritte, die je Zeile oder je Kanal
/// unabhaengig sind; das Ergebnis haengt nicht von der Aufteilung ab.
/// Unter 2¹⁶ Werten rechnet ein Kern.
pub(crate) fn je_kern<T: Send>(daten: &mut [T], stueck: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    let stueck = stueck.max(1);
    let stuecke = daten.len() / stueck;
    if daten.len() < 1 << 16 || stuecke < 2 {
        f(0, daten);
        return;
    }
    let je = stuecke.div_ceil(kerne());
    std::thread::scope(|s| {
        for (i, teil) in daten.chunks_mut(je * stueck).enumerate() {
            let f = &f;
            s.spawn(move || f(i * je, teil));
        }
    });
}

/// Ab so vielen Eingaben rechnet der Stapel (auf macOS ab hier mit Metal,
/// wie `integer_llm_kernels::ressourcen::metal::VORGABE_AB`).
pub(crate) const GEBUENDELT_AB: usize = 16;

/// **Wenige Eingaben** (der Decoder, Token fuer Token): je Zeile das
/// Skalarprodukt der Kernkiste, exakt in i64, beide Stellen in `f64`
/// zusammengesetzt. Ohne int16 am Ausgang gibt es keine Nachkommabits zu
/// suchen; eine Anpassung je Kanal wuerde bei einem einzelnen Token
/// dauernd nachrechnen.
fn linear_ganz_einzeln(xq: &[i16], bits: &[i32], zeilen: usize, m: &Ganzmatrix) -> Vec<f32> {
    use integer_llm_kernels::dot::dot_i8_i16;
    // Bloecke von Kanaelen ueber den Fadenpool der Kernkiste; jeder rechnet
    // seine Kanaele fuer alle Zeilen. 📌 Hier stand ein `std::thread::scope`
    // je Aufruf; der Sprecher ruft je Bild ueber 200 Matrizen.
    const BLOCK: usize = 64;
    let st = &m.stellen;
    let zwei = m.zwei();
    let teile = integer_llm_kernels::fadenpool::verteilen(m.aus.div_ceil(BLOCK), kerne(), |b| {
        let (von, bis) = (b * BLOCK, ((b + 1) * BLOCK).min(m.aus));
        let mut aus = Vec::with_capacity(zeilen * (bis - von));
        for (xz, &a) in xq.chunks(m.ein).take(zeilen).zip(bits) {
            for z in von..bis {
                let h = dot_i8_i16(&st.w[z * m.ein..][..m.ein], xz) as f64;
                let l = if zwei { dot_i8_i16(&st.w[(m.aus + z) * m.ein..][..m.ein], xz) as f64 } else { 0.0 };
                let skala = 2f64.powi(-i32::from(st.schiebe[z]) - a);
                aus.push(((h + l / 256.0) * skala) as f32);
            }
        }
        aus
    });
    let mut y = vec![0f32; zeilen * m.aus];
    for (b, teil) in teile.iter().enumerate() {
        let breite = teil.len() / zeilen.max(1);
        for (r, stueck) in teil.chunks(breite.max(1)).enumerate().take(zeilen) {
            y[r * m.aus + b * BLOCK..][..breite].copy_from_slice(stueck);
        }
    }
    y
}

/// Die Nachkommabits, mit denen die Kerne jede Eingabe lesen; jedes Fenster
/// kommt mit hoechstens so vielen herein.
const EINGABE_BITS: u8 = 30;

fn stelle_mal(eingaben: &[&[i16]], ein: usize, st: &Stelle) -> (Vec<Vec<i16>>, Vec<u8>) {
    einstellen(&st.bits, |o| integer_llm_kernels::linear::linear_w8a16_pc_stapel(eingaben, &st.w, ein, &st.schiebe, EINGABE_BITS, o))
}

/// **Rechnet und stellt die Ausgabebits je Kanal ein** (siehe
/// [`linear_ganz`]); `rechnen` bekommt die Bits und liefert die int16.
fn einstellen(bits: &Ausgabebits, rechnen: impl Fn(&[u8]) -> Vec<Vec<i16>>) -> (Vec<Vec<i16>>, Vec<u8>) {
    const OBEN: u8 = 62;
    let mut o = bits.je_kanal.lock().expect("Bits").clone();
    // Beim ersten Aufruf ganz einstellen, danach traege (siehe unten).
    let grenze: u16 = if bits.eingestellt.swap(true, std::sync::atomic::Ordering::Relaxed) { 2_048 } else { 8_192 };
    let mut runden = 0;
    loop {
        let y = rechnen(&o);
        let mut groesst = vec![0u16; o.len()];
        for z in &y {
            groesst.iter_mut().zip(z).for_each(|(g, v)| *g = (*g).max(v.unsigned_abs()));
        }
        // Je Kanal die Bits, die den Bereich zur Haelfte bis ganz fuellen.
        let mut nochmal = false;
        let neu: Vec<u8> = o
            .iter()
            .zip(&groesst)
            .map(|(&oz, &g)| {
                if g >= 32_767 {
                    nochmal = true;
                    oz.saturating_sub(4)
                } else if g == 0 {
                    // Alles auf null gerundet: weit nach oben, die naechste
                    // Runde misst nach.
                    nochmal |= oz < OBEN;
                    (oz + 16).min(OBEN)
                } else {
                    // ⚑ Neu gerechnet wird beim ersten Aufruf unter einem
                    // Viertel des Bereichs, danach erst unter einem
                    // Sechzehntel (11 Bit); darueber merkt sich der Kanal die
                    // feineren Bits fuer den naechsten Aufruf. Sonst rechnete
                    // jede neue Aufnahme ihre Matrizen doppelt, gemessen 5,4 s
                    // statt 4,6 s je Satz. Nur traege von Anfang an kostete
                    // die erste Aufnahme Genauigkeit (3·10⁻³ statt 6·10⁻⁴).
                    nochmal |= g < grenze;
                    oz + bits_fuer(f32::from(g), 32_767.0, OBEN - oz)
                }
            })
            .collect();
        runden += 1;
        if !nochmal || runden == 8 {
            *bits.je_kanal.lock().expect("Bits") = neu;
            return (y, o);
        }
        o = neu;
    }
}

/// `y[t] = W · x[t]` in `f32`, `W` zeilenweise `[aus][ein]`, verteilt auf die Kerne.
///
/// ⚑ **Wenige Eingaben verteilen die Ausgaben, viele die Eingaben.** Bei
/// einem einzelnen Token (der Sprecher, Bild fuer Bild) rechnete sonst ein
/// Kern die ganze Matrix: gemessen 9 GB/s, 34 ms je Schritt der
/// Code-Vorhersage. Jede Ausgabe bleibt dieselbe Summe in derselben
/// Reihenfolge ([`summe_f32`]), das Ergebnis ist in beiden Wegen bitgleich.
pub(crate) fn linear_genau(x: &[f32], zeilen: usize, w: &[f32], aus: usize) -> Vec<f32> {
    let ein = w.len() / aus;
    let mut y = vec![0f32; zeilen * aus];
    if zeilen < kerne() {
        // Bloecke von Ausgaben ueber den Fadenpool der Kernkiste.
        const BLOCK: usize = 64;
        let teile = integer_llm_kernels::fadenpool::verteilen(aus.div_ceil(BLOCK), kerne(), |b| {
            let (von, bis) = (b * BLOCK, ((b + 1) * BLOCK).min(aus));
            let mut t = Vec::with_capacity(zeilen * (bis - von));
            for xz in x.chunks(ein).take(zeilen) {
                t.extend((von..bis).map(|o| summe_f32(xz, &w[o * ein..][..ein])));
            }
            t
        });
        for (b, t) in teile.iter().enumerate() {
            let breite = t.len() / zeilen.max(1);
            for (r, stueck) in t.chunks(breite.max(1)).enumerate().take(zeilen) {
                y[r * aus + b * BLOCK..][..breite].copy_from_slice(stueck);
            }
        }
        return y;
    }
    let je = zeilen.div_ceil(kerne());
    std::thread::scope(|s| {
        for (teil, yteil) in y.chunks_mut(je * aus).enumerate() {
            s.spawn(move || {
                for (r, yz) in yteil.chunks_mut(aus).enumerate() {
                    let xz = &x[(teil * je + r) * ein..][..ein];
                    for (o, ziel) in yz.iter_mut().enumerate() {
                        *ziel = summe_f32(xz, &w[o * ein..][..ein]);
                    }
                }
            });
        }
    });
    y
}

/// Das Skalarprodukt von [`linear_genau`], eine Kette von vorn nach hinten.
#[inline]
fn summe_f32(x: &[f32], w: &[f32]) -> f32 {
    let mut summe = 0f32;
    for (a, c) in x.iter().zip(w) {
        summe += a * c;
    }
    summe
}

fn layernorm(x: &[f32], g: &[f32], b: &[f32]) -> Vec<f32> {
    let breite = g.len();
    let mut y = vec![0f32; x.len()];
    for (xz, yz) in x.chunks(breite).zip(y.chunks_mut(breite)) {
        let mittel = xz.iter().map(|v| f64::from(*v)).sum::<f64>() / breite as f64;
        let streu = xz.iter().map(|v| (f64::from(*v) - mittel).powi(2)).sum::<f64>() / breite as f64;
        let n = 1.0 / (streu + 1e-5).sqrt();
        for i in 0..breite {
            yz[i] = ((f64::from(xz[i]) - mittel) * n) as f32 * g[i] + b[i];
        }
    }
    y
}

/// Faltung in einer Dimension, Kern 3, Rand 1; Eingang `[ein][t]`,
/// Ausgang zeilenweise je Fenster `[t'][aus]`.
fn faltung(x: &[f32], ein: usize, t_ein: usize, g: &Gewichte, w: &str, b: &str, schritt: usize) -> Vec<f32> {
    let t_aus = t_ein.div_ceil(schritt);
    // Umgeordnet: je Ausgangsfenster die drei Spalten des Eingangs als Zeile.
    let mut spalten = vec![0f32; t_aus * ein * 3];
    for t in 0..t_aus {
        for c in 0..ein {
            for k in 0..3 {
                let quelle = (t * schritt + k) as isize - 1;
                if (0..t_ein as isize).contains(&quelle) {
                    spalten[(t * ein + c) * 3 + k] = x[c * t_ein + quelle as usize];
                }
            }
        }
    }
    let mut y = linear(&spalten, t_aus, g, w, Some(b));
    gelu_alle(&mut y);
    y
}

/// Breite eines Kopfes, bei jeder Groesse von whisper 64 ([`Masse::laden`] prueft es).
const KOPFBREITE: usize = 64;
/// So viele Anfragen teilen sich einen Durchgang ueber Schluessel und Werte.
///
/// 📌 Einzeln las jede Anfrage die 768 KB ihres Kopfes neu, 23 GB je Ebene,
/// und die Aufmerksamkeit brauchte 3,8 s von 5,5; die Rechnung selbst war
/// nicht das Hindernis.
const BLOCK: usize = 16;

/// Registerkacheln: so viele Anfragen, Schluessel und Stellen eines Kopfes
/// rechnen zusammen in Registern.
///
/// 📌 Die Bewertungen liefen vorher als Zeile mal Zeile durch den Speicher
/// (1,25 s je Encoder). In der Kachel bleibt jede Summe in einem Register,
/// und jede geladene Zahl dient vier Anfragen. **Die Reihenfolge jeder
/// Summe ist dieselbe** (ab null, die Stellen aufsteigend), die Ergebnisse
/// also bitgleich.
const KACHEL_ANFRAGEN: usize = 4;
const KACHEL_SCHLUESSEL: usize = 8;
const KACHEL_STELLEN: usize = 16;

/// `w[r][j] = Σ_c qb[r][c] · kt[c][j]` fuer `b` Anfragen; `qb` ist schon
/// skaliert, `kt` quer (`[stelle][fenster]`).
fn bewerten(qb: &[f32], b: usize, kt: &[f32], n: usize, w: &mut [f32]) {
    const RQ: usize = KACHEL_ANFRAGEN;
    const RK: usize = KACHEL_SCHLUESSEL;
    for r0 in (0..b).step_by(RQ) {
        let rq = RQ.min(b - r0);
        // Fehlen Anfragen am Ende, rechnet die Kachel die letzte doppelt
        // und schreibt sie nur einmal.
        let qr: [&[f32; KOPFBREITE]; RQ] =
            std::array::from_fn(|r| qb[(r0 + r.min(rq - 1)) * KOPFBREITE..][..KOPFBREITE].try_into().expect("Kopfbreite"));
        let mut j0 = 0;
        while j0 + RK <= n {
            let mut acc = [[0f32; RK]; RQ];
            for c in 0..KOPFBREITE {
                let k: &[f32; RK] = kt[c * n + j0..][..RK].try_into().expect("Kachel");
                for r in 0..RQ {
                    let f = qr[r][c];
                    for l in 0..RK {
                        acc[r][l] += f * k[l];
                    }
                }
            }
            for (r, a) in acc.iter().enumerate().take(rq) {
                w[(r0 + r) * n + j0..][..RK].copy_from_slice(a);
            }
            j0 += RK;
        }
        for j in j0..n {
            for (r, q) in qr.iter().enumerate().take(rq) {
                let mut s = 0f32;
                for c in 0..KOPFBREITE {
                    s += q[c] * kt[c * n + j];
                }
                w[(r0 + r) * n + j] = s;
            }
        }
    }
}

/// `ziel[r][d] = Σ_j w[r][j] · v[j][d]` fuer `b` Anfragen; `ziel` beginnt an
/// der ersten Stelle des Kopfes, Zeilen im Abstand `breite`.
fn mischen(w: &[f32], b: usize, n: usize, vh: &[f32], ziel: &mut [f32], breite: usize) {
    const RQ: usize = KACHEL_ANFRAGEN;
    const RD: usize = KACHEL_STELLEN;
    let werte = &vh.as_chunks::<KOPFBREITE>().0[..n];
    for r0 in (0..b).step_by(RQ) {
        let rq = RQ.min(b - r0);
        let wr: [&[f32]; RQ] = std::array::from_fn(|r| &w[(r0 + r.min(rq - 1)) * n..][..n]);
        for d0 in (0..KOPFBREITE).step_by(RD) {
            let mut acc = [[0f32; RD]; RQ];
            for (j, vj) in werte.iter().enumerate() {
                let v: &[f32; RD] = vj[d0..][..RD].try_into().expect("Kachel");
                let f: [f32; RQ] = std::array::from_fn(|r| wr[r][j]);
                for r in 0..RQ {
                    for l in 0..RD {
                        acc[r][l] += f[r] * v[l];
                    }
                }
            }
            for (r, a) in acc.iter().enumerate().take(rq) {
                ziel[(r0 + r) * breite + d0..][..RD].copy_from_slice(a);
            }
        }
    }
}

/// Je Kopf zusammenhaengend: `[kopf][fenster][KOPFBREITE]`.
fn je_kopf(x: &[f32], n: usize) -> Vec<f32> {
    let breite = x.len() / n;
    let mut y = vec![0f32; x.len()];
    for (j, zeile) in x.chunks_exact(breite).enumerate() {
        for (h, teil) in zeile.as_chunks::<KOPFBREITE>().0.iter().enumerate() {
            y[(h * n + j) * KOPFBREITE..][..KOPFBREITE].copy_from_slice(teil);
        }
    }
    y
}

/// Je Kopf quer: `[kopf][stelle][fenster]`.
fn quer_je_kopf(x: &[f32], n: usize) -> Vec<f32> {
    let breite = x.len() / n;
    let mut y = vec![0f32; x.len()];
    for (j, zeile) in x.chunks_exact(breite).enumerate() {
        for (i, v) in zeile.iter().enumerate() {
            y[i * n + j] = *v;
        }
    }
    y
}

fn aufmerksamkeit(x: &[f32], g: &Gewichte, p: &str) -> Vec<f32> {
    let (breite, koepfe) = (g.masse.breite, g.masse.koepfe);
    let n = x.len() / breite;
    let q = linear(x, n, g, &format!("{p}.q_proj.weight"), Some(&format!("{p}.q_proj.bias")));
    let k = linear(x, n, g, &format!("{p}.k_proj.weight"), None);
    let v = linear(x, n, g, &format!("{p}.v_proj.weight"), Some(&format!("{p}.v_proj.bias")));
    let (kk, vv) = (quer_je_kopf(&k, n), je_kopf(&v, n));
    let skala = 1.0 / (KOPFBREITE as f32).sqrt();
    let mut aus = vec![0f32; n * breite];
    let je = n.div_ceil(kerne());
    std::thread::scope(|s| {
        for (teil, ateil) in aus.chunks_mut(je * breite).enumerate() {
            let (q, kk, vv) = (&q, &kk, &vv);
            s.spawn(move || {
                let (lo, hi) = (teil * je, teil * je + ateil.len() / breite);
                let mut w = vec![0f32; BLOCK * n];
                let mut qb = vec![0f32; BLOCK * KOPFBREITE];
                for h in 0..koepfe {
                    let kh = &kk[h * n * KOPFBREITE..][..n * KOPFBREITE];
                    let vh = &vv[h * n * KOPFBREITE..][..n * KOPFBREITE];
                    for i0 in (lo..hi).step_by(BLOCK) {
                        let b = BLOCK.min(hi - i0);
                        for r in 0..b {
                            let quelle = &q[(i0 + r) * breite + h * KOPFBREITE..][..KOPFBREITE];
                            qb[r * KOPFBREITE..][..KOPFBREITE].iter_mut().zip(quelle).for_each(|(z, v)| *z = v * skala);
                        }
                        bewerten(&qb, b, kh, n, &mut w);
                        for wr in w.chunks_mut(n).take(b) {
                            let hoechst = wr.iter().fold(f32::NEG_INFINITY, |m, v| m.max(*v));
                            let mut summe = 0f32;
                            wr.iter_mut().for_each(|wj| {
                                *wj = (*wj - hoechst).exp();
                                summe += *wj;
                            });
                            wr.iter_mut().for_each(|wj| *wj /= summe);
                        }
                        let zeilen = &mut ateil[(i0 - lo) * breite..];
                        mischen(&w, b, n, vh, &mut zeilen[h * KOPFBREITE..], breite);
                    }
                }
            });
        }
    });
    linear(&aus, n, g, &format!("{p}.out_proj.weight"), Some(&format!("{p}.out_proj.bias")))
}

/// Eine Ebene des Encoders, `x` zeilenweise `[fenster][breite]`.
pub fn ebene(x: &mut [f32], g: &Gewichte, nr: usize) {
    let p = format!("encoder.layers.{nr}");
    let a = layernorm(x, g.t(&format!("{p}.self_attn_layer_norm.weight")), g.t(&format!("{p}.self_attn_layer_norm.bias")));
    let a = aufmerksamkeit(&a, g, &format!("{p}.self_attn"));
    x.iter_mut().zip(&a).for_each(|(v, d)| *v += d);
    let n = x.len() / g.masse.breite;
    let f = layernorm(x, g.t(&format!("{p}.final_layer_norm.weight")), g.t(&format!("{p}.final_layer_norm.bias")));
    let mut f = linear(&f, n, g, &format!("{p}.fc1.weight"), Some(&format!("{p}.fc1.bias")));
    gelu_alle(&mut f);
    let f = linear(&f, n, g, &format!("{p}.fc2.weight"), Some(&format!("{p}.fc2.bias")));
    x.iter_mut().zip(&f).for_each(|(v, d)| *v += d);
}

/// Die Einbettung: beide Faltungen und die Lage, zeilenweise je Fenster.
pub fn einbetten(mel: &crate::spektrum::LogMel, g: &Gewichte) -> Vec<f32> {
    let (breite, baender) = (g.masse.breite, g.masse.baender);
    assert_eq!(mel.baender, baender, "dieses whisper braucht {baender} Baender");
    let t = crate::spektrum::FENSTER_JE_STUECK;
    let x1 = faltung(&mel.werte, baender, t, g, "encoder.conv1.weight", "encoder.conv1.bias", 1);
    // Fuer die zweite Faltung zurueck auf [kanal][zeit].
    let mut quer = vec![0f32; breite * t];
    for z in 0..t {
        for c in 0..breite {
            quer[c * t + z] = x1[z * breite + c];
        }
    }
    let mut x = faltung(&quer, breite, t, g, "encoder.conv2.weight", "encoder.conv2.bias", 2);
    x.iter_mut().zip(g.t("encoder.embed_positions.weight")).for_each(|(v, p)| *v += p);
    x
}

/// **Der ganze Encoder**: Einbettung, `ebenen` Ebenen, LayerNorm am Ende.
pub fn kodieren(mel: &crate::spektrum::LogMel, g: &Gewichte, ebenen: usize) -> Vec<f32> {
    let mut x = einbetten(mel, g);
    for nr in 0..ebenen {
        ebene(&mut x, g, nr);
    }
    layernorm(&x, g.t("encoder.layer_norm.weight"), g.t("encoder.layer_norm.bias"))
}

// ---- Decoder -------------------------------------------------------------

/// Sondertoken von whisper large-v3.
pub const ENDE: u32 = 50_257;
pub const ANFANG: u32 = 50_258;
pub const DEUTSCH: u32 = 50_261;
pub const ENGLISCH: u32 = 50_259;

/// Aufmerksamkeit fuer den Decoder: `q` ueber `kv`. Mit `kausal = Some(v)`
/// steht die erste Anfrage an Stelle `v` und sieht nur bis zu sich selbst.
fn aufmerksamkeit_dec(q: &[f32], k: &[f32], v: &[f32], kausal: Option<usize>, koepfe: usize) -> Vec<f32> {
    let breite = koepfe * KOPFBREITE;
    let (nq, nk) = (q.len() / breite, k.len() / breite);
    let skala = 1.0 / (KOPFBREITE as f32).sqrt();
    let mut aus = vec![0f32; nq * breite];
    let mut w = vec![0f32; nk];
    for i in 0..nq {
        let bis = kausal.map_or(nk, |v| v + i + 1);
        for h in 0..koepfe {
            let qi = &q[i * breite + h * KOPFBREITE..][..KOPFBREITE];
            let mut hoechst = f32::NEG_INFINITY;
            for j in 0..bis {
                w[j] = skalar(qi, &k[j * breite + h * KOPFBREITE..][..KOPFBREITE]) * skala;
                hoechst = hoechst.max(w[j]);
            }
            let mut summe = 0f32;
            for wj in &mut w[..bis] {
                *wj = (*wj - hoechst).exp();
                summe += *wj;
            }
            let ziel = &mut aus[i * breite + h * KOPFBREITE..][..KOPFBREITE];
            for j in 0..bis {
                let f = w[j] / summe;
                for (z, vv) in ziel.iter_mut().zip(&v[j * breite + h * KOPFBREITE..][..KOPFBREITE]) {
                    *z += f * vv;
                }
            }
        }
    }
    aus
}

/// Was der Decoder aus dem Encoder braucht: je Ebene Schluessel und Werte
/// der Kreuzaufmerksamkeit, einmal gerechnet.
pub struct Gedaechtnis {
    kv: Vec<(Vec<f32>, Vec<f32>)>,
}

/// Rechnet Schluessel und Werte der Kreuzaufmerksamkeit fuer alle Ebenen.
pub fn gedaechtnis(enc: &[f32], g: &Gewichte) -> Gedaechtnis {
    let n = enc.len() / g.masse.breite;
    let kv = (0..g.masse.ebenen_dec)
        .map(|nr| {
            let p = format!("decoder.layers.{nr}.encoder_attn");
            let k = linear(enc, n, g, &format!("{p}.k_proj.weight"), None);
            let v = linear(enc, n, g, &format!("{p}.v_proj.weight"), Some(&format!("{p}.v_proj.bias")));
            (k, v)
        })
        .collect();
    Gedaechtnis { kv }
}

/// **Der Zwischenspeicher der Selbstaufmerksamkeit**: je Ebene Schluessel
/// und Werte der Token, die der Decoder schon gelesen hat.
///
/// ⚑ Bis 0.19.0 rechnete jeder Schritt die ganze Folge neu; bei 448 Stellen
/// waere das quadratisch in der Laenge eines Stuecks.
#[derive(Default)]
pub struct Verlauf {
    kv: Vec<(Vec<f32>, Vec<f32>)>,
    gelesen: usize,
}

/// **Liest die Token `neu`** hinter dem `verlauf` und gibt die Logits nach
/// dem letzten zurueck.
pub fn weiter(neu: &[u32], verlauf: &mut Verlauf, m: &Gedaechtnis, g: &Gewichte) -> Vec<f32> {
    let (n, v0) = (neu.len(), verlauf.gelesen);
    let (breite, stellen) = (g.masse.breite, g.masse.stellen);
    assert!(n > 0 && v0 + n <= stellen, "der Decoder hat {stellen} Stellen");
    if verlauf.kv.is_empty() {
        verlauf.kv = vec![(Vec::new(), Vec::new()); m.kv.len()];
    }
    let einb = g.t(KOPF);
    let lage = g.t("decoder.embed_positions.weight");
    let mut x = vec![0f32; n * breite];
    for (i, &id) in neu.iter().enumerate() {
        for c in 0..breite {
            x[i * breite + c] = einb[id as usize * breite + c] + lage[(v0 + i) * breite + c];
        }
    }
    for (nr, ((ck, cv), (sk, sv))) in m.kv.iter().zip(&mut verlauf.kv).enumerate() {
        let p = format!("decoder.layers.{nr}");
        let a = layernorm(&x, g.t(&format!("{p}.self_attn_layer_norm.weight")), g.t(&format!("{p}.self_attn_layer_norm.bias")));
        let s = format!("{p}.self_attn");
        let q = linear(&a, n, g, &format!("{s}.q_proj.weight"), Some(&format!("{s}.q_proj.bias")));
        sk.extend(linear(&a, n, g, &format!("{s}.k_proj.weight"), None));
        sv.extend(linear(&a, n, g, &format!("{s}.v_proj.weight"), Some(&format!("{s}.v_proj.bias"))));
        let o = aufmerksamkeit_dec(&q, sk, sv, Some(v0), g.masse.koepfe);
        let o = linear(&o, n, g, &format!("{s}.out_proj.weight"), Some(&format!("{s}.out_proj.bias")));
        x.iter_mut().zip(&o).for_each(|(v, d)| *v += d);
        let a = layernorm(&x, g.t(&format!("{p}.encoder_attn_layer_norm.weight")), g.t(&format!("{p}.encoder_attn_layer_norm.bias")));
        let e = format!("{p}.encoder_attn");
        let q = linear(&a, n, g, &format!("{e}.q_proj.weight"), Some(&format!("{e}.q_proj.bias")));
        let o = aufmerksamkeit_dec(&q, ck, cv, None, g.masse.koepfe);
        let o = linear(&o, n, g, &format!("{e}.out_proj.weight"), Some(&format!("{e}.out_proj.bias")));
        x.iter_mut().zip(&o).for_each(|(v, d)| *v += d);
        let f = layernorm(&x, g.t(&format!("{p}.final_layer_norm.weight")), g.t(&format!("{p}.final_layer_norm.bias")));
        let mut f = linear(&f, n, g, &format!("{p}.fc1.weight"), Some(&format!("{p}.fc1.bias")));
        gelu_alle(&mut f);
        let f = linear(&f, n, g, &format!("{p}.fc2.weight"), Some(&format!("{p}.fc2.bias")));
        x.iter_mut().zip(&f).for_each(|(v, d)| *v += d);
    }
    verlauf.gelesen += n;
    let letzte = layernorm(&x[(n - 1) * breite..], g.t("decoder.layer_norm.weight"), g.t("decoder.layer_norm.bias"));
    // Der Kopf ist die Einbettung selbst (geteilte Gewichte).
    linear(&letzte, 1, g, KOPF, None)
}

/// **Gierig mitschreiben**: Vorspann, dann `argmax` mit den unterdrueckten
/// Token, bis zum Ende oder bis die Stellen des Decoders voll sind. Gibt die ganze
/// Folge und je Schritt den Logit des gewaehlten Tokens zurueck.
pub fn mitschreiben(
    mel: &crate::spektrum::LogMel,
    g: &Gewichte,
    sprache: u32,
    unterdrueckt: &[u32],
    am_anfang: &[u32],
) -> (Vec<u32>, Vec<f32>) {
    let s = mitschreiben_aus(&gedaechtnis(&kodieren(mel, g, g.masse.ebenen_enc), g), g, sprache, unterdrueckt, am_anfang);
    (s.ids, s.logits)
}

/// Eine Mitschrift: die Token, je Schritt der Logit des gewaehlten Tokens
/// und das Mittel seiner Log-Wahrscheinlichkeiten.
pub struct Mitschrift {
    pub ids: Vec<u32>,
    pub logits: Vec<f32>,
    pub mittel_log: f32,
}

/// [`mitschreiben`] aus dem schon gerechneten [`Gedaechtnis`].
pub fn mitschreiben_aus(m: &Gedaechtnis, g: &Gewichte, sprache: u32, unterdrueckt: &[u32], am_anfang: &[u32]) -> Mitschrift {
    let z = g.zeichen;
    let mut ids = vec![ANFANG, sprache, z.mitschreiben, z.ohne_zeiten];
    let mut verlauf = Verlauf::default();
    let mut l = weiter(&ids, &mut verlauf, m, g);
    let (mut logits, mut summe_log) = (Vec::new(), 0f64);
    for schritt in 0.. {
        for &u in unterdrueckt.iter().chain(if schritt == 0 { am_anfang } else { &[] }) {
            l[u as usize] = f32::NEG_INFINITY;
        }
        let (t, w) = l.iter().enumerate().fold((0, f32::NEG_INFINITY), |b, (i, &v)| if v > b.1 { (i, v) } else { b });
        let norm: f64 = l.iter().map(|v| f64::from(v - w).exp()).sum();
        summe_log -= norm.ln();
        ids.push(t as u32);
        logits.push(w);
        if t as u32 == ENDE || ids.len() == g.masse.stellen {
            break;
        }
        l = weiter(&[t as u32], &mut verlauf, m, g);
    }
    let mittel_log = (summe_log / logits.len() as f64) as f32;
    Mitschrift { ids, logits, mittel_log }
}

/// Die Logits nach dem Anfangstoken allein: daraus liest whisper die
/// Sprache und ob ueberhaupt gesprochen wird.
pub fn anfangslogits(m: &Gedaechtnis, g: &Gewichte) -> Vec<f32> {
    weiter(&[ANFANG], &mut Verlauf::default(), m, g)
}

/// **Welche Sprache spricht die Aufnahme?** Wie whisper: Unter den
/// Sprachtoken gewinnt das groesste Logit nach dem Anfang.
pub fn sprache_aus(anfang: &[f32], sprachen: &Sprachen) -> u32 {
    sprachen.kuerzel.values().copied().fold(DEUTSCH, |b, id| if anfang[id as usize] > anfang[b as usize] { id } else { b })
}

/// **Wird in diesem Stueck ueberhaupt gesprochen?** Mindestens
/// [`GESPROCHEN_AB_S`] in Abschnitten zu 20 ms lauter als [`LEISE_DBFS`].
///
/// ⚑ **Ein Pegel und nicht die Regel von whisper.** whisper verwirft ein
/// Stueck, wenn `<|nospeech|>` nach dem Anfang wahrscheinlich ist; bei
/// large-v3-turbo liegt das bei 10⁻¹⁰, auch fuer reine Stille (gemessen am
/// 2026-11-11, mit `transformers` bestaetigt). 📌 Ohne Wache brachte ein Rest
/// von 0,13 s hinter 30 s Sprache „Vielen Dank.“
pub fn gesprochen(stueck: &[f32]) -> bool {
    let grenze = 10f64.powf(LEISE_DBFS / 20.0);
    let laut = stueck
        .chunks(ABSCHNITT)
        .filter(|a| (a.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / a.len() as f64).sqrt() > grenze)
        .count();
    laut * ABSCHNITT >= (GESPROCHEN_AB_S * crate::spektrum::ABTASTRATE as f64) as usize
}

/// 20 ms bei 16 kHz.
const ABSCHNITT: usize = 320;
/// Leiser als das ist Rauschen eines ruhigen Raums, nicht Sprache.
pub const LEISE_DBFS: f64 = -45.0;
/// So lange muss in einem Stueck gesprochen werden.
pub const GESPROCHEN_AB_S: f64 = 0.3;

/// **Die Sprachtoken** aus `added_tokens.json`: Kuerzel wie `de` nach Token.
pub struct Sprachen {
    kuerzel: BTreeMap<String, u32>,
}

impl Sprachen {
    pub fn laden(datei: &Path) -> Result<Self, String> {
        let roh = std::fs::read(datei).map_err(|f| format!("{}: {f}", datei.display()))?;
        let Json::Objekt(o) = json(&roh, &mut 0)? else { return Err("added_tokens.json ist kein Objekt".into()) };
        let Some(Json::Zahl(uebersetzen)) = o.get("<|translate|>") else { return Err("added_tokens.json: <|translate|> fehlt".into()) };
        let uebersetzen = *uebersetzen as u32;
        let kuerzel = o
            .into_iter()
            .filter_map(|(k, v)| {
                let Json::Zahl(z) = v else { return None };
                let innen = k.strip_prefix("<|")?.strip_suffix("|>")?;
                // Sprachen liegen zwischen dem Anfang und `translate`.
                let id = z as u32;
                (id > ANFANG && id < uebersetzen && innen.chars().all(|c| c.is_ascii_lowercase())).then(|| (innen.to_string(), id))
            })
            .collect();
        Ok(Self { kuerzel })
    }

    /// Das Token zu einem Kuerzel wie `de`.
    pub fn token(&self, kuerzel: &str) -> Option<u32> {
        self.kuerzel.get(kuerzel).copied()
    }

    /// Wie viele Sprachen kennt das Modell?
    pub fn anzahl(&self) -> usize {
        self.kuerzel.len()
    }
}

/// Die Groessen von whisper, die dieser Weg waehlt, die groesste zuerst; je
/// ein Ordner neben dem Hoermodell von whisper.cpp.
pub const GROESSEN: [&str; 3] = ["whisper-large-v3-turbo", "whisper-small", "whisper-base"];

/// Ab so vielen Kernen rechnet large-v3-turbo auch ohne Grafikeinheit.
pub const GROSS_AB_KERNEN: usize = 12;

/// **Welche Groesse von whisper hoert auf dieser Maschine?**
///
/// ⚑ Die Regel, gemessen am 2026-11-11 (Hoerprobe mit 16 Aufnahmen; Behaelter
/// mit 4 Kernen und 4 GB ohne Grafikeinheit): small schreibt Deutsch mit
/// 1 Fehler in 140 Woertern in 3,6 s je Satz, large-v3-turbo fehlerfrei,
/// auch mit Rauschen, aber in 17 s; mit Metal braucht large-v3-turbo 2,4 s.
/// Deshalb large-v3-turbo nur mit Grafikeinheit oder ab
/// [`GROSS_AB_KERNEN`] Kernen, sonst small, zuletzt base (fuer Deutsch
/// schwach, 9 Fehler in 140). `passt` ist die Speicherwache: Sie bekommt den
/// Ordner und sagt, ob die Gewichte neben allem anderen Platz haben.
/// `None`, wenn keine Groesse vorliegt und passt.
pub fn groesse_waehlen(audio: &Path, gpu: bool, kerne: usize, passt: impl Fn(&Path) -> bool) -> Option<std::path::PathBuf> {
    GROESSEN
        .iter()
        .filter(|&&g| g != GROESSEN[0] || gpu || kerne >= GROSS_AB_KERNEN)
        .map(|g| audio.join(g))
        .find(|o| o.join("model.safetensors").is_file() && o.join("config.json").is_file() && passt(o))
}

/// So viele Proben (16 kHz) fasst ein Stueck: 30 s.
pub const STUECK: usize = crate::spektrum::STUECK;

/// **Text aus einer Aufnahme** (16 kHz, ein Kanal), mit den Gewichten in
/// `ordner`. `sprache` ist ein Kuerzel wie `de` oder `auto`.
///
/// Laengere Aufnahmen gehen in Stuecken zu 30 s durch, die Sprache wird am
/// ersten gesprochenen erkannt; stille Stuecke ([`gesprochen`]) ueberspringt
/// der Decoder. ⚠️ An der Grenze zweier Stuecke kann ein Wort zerfallen;
/// whisper selbst setzt dort mit Zeitmarken neu an, die dieser Weg noch
/// nicht liest.
pub fn text(ordner: &Path, proben: &[f32], sprache: &str) -> Result<String, String> {
    let g = Gewichte::laden(&ordner.join("model.safetensors"))?;
    let (u, a) = unterdrueckte(&ordner.join("generation_config.json"))?;
    let woerter = Woerterbuch::laden(&ordner.join("vocab.json"))?;
    let sprachen = Sprachen::laden(&ordner.join("added_tokens.json"))?;
    let mut gewaehlt = match sprache.trim() {
        "" | "auto" => None,
        k => Some(sprachen.token(k).ok_or_else(|| format!("whisper kennt die Sprache '{k}' nicht"))?),
    };
    let mut teile = Vec::new();
    for stueck in proben.chunks(STUECK) {
        if !gesprochen(stueck) {
            continue;
        }
        let m = gedaechtnis(&kodieren(&crate::spektrum::log_mel(stueck, g.masse.baender), &g, g.masse.ebenen_enc), &g);
        let s = *gewaehlt.get_or_insert_with(|| sprache_aus(&anfangslogits(&m, &g), &sprachen));
        let schrift = mitschreiben_aus(&m, &g, s, &u, &a);
        teile.push(woerter.text(&schrift.ids).trim().to_string());
    }
    Ok(teile.into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" "))
}

/// Die unterdrueckten Token aus `generation_config.json`.
pub fn unterdrueckte(datei: &Path) -> Result<(Vec<u32>, Vec<u32>), String> {
    let roh = std::fs::read(datei).map_err(|f| format!("{}: {f}", datei.display()))?;
    let Json::Objekt(o) = json(&roh, &mut 0)? else { return Err("kein Objekt".into()) };
    let liste = |k: &str| -> Vec<u32> {
        match o.get(k) {
            Some(Json::Liste(l)) => l.iter().filter_map(|j| if let Json::Zahl(z) = j { Some(*z as u32) } else { None }).collect(),
            _ => Vec::new(),
        }
    };
    Ok((liste("suppress_tokens"), liste("begin_suppress_tokens")))
}

/// Die Tabelle von GPT-2: Byte zu Zeichen im Woerterbuch.
fn byte_zeichen() -> [char; 256] {
    let mut tafel = ['\0'; 256];
    let mut n = 0u32;
    for b in 0..256u32 {
        let sichtbar = (33..=126).contains(&b) || (161..=172).contains(&b) || (174..=255).contains(&b);
        tafel[b as usize] = if sichtbar {
            char::from_u32(b).expect("Zeichen")
        } else {
            n += 1;
            char::from_u32(255 + n).expect("Zeichen")
        };
    }
    tafel
}

/// **Token zu Text**, aus `vocab.json`; Sondertoken entfallen.
pub struct Woerterbuch {
    texte: Vec<Vec<u8>>,
}

impl Woerterbuch {
    pub fn laden(datei: &Path) -> Result<Self, String> {
        let roh = std::fs::read(datei).map_err(|f| format!("{}: {f}", datei.display()))?;
        let Json::Objekt(o) = json(&roh, &mut 0)? else { return Err("kein Objekt".into()) };
        let tafel = byte_zeichen();
        let zurueck: BTreeMap<char, u8> = tafel.iter().enumerate().map(|(b, c)| (*c, b as u8)).collect();
        let mut texte = vec![Vec::new(); ENDE as usize];
        for (wort, id) in o {
            let Json::Zahl(id) = id else { continue };
            let id = id as usize;
            if id < texte.len() {
                texte[id] = wort.chars().filter_map(|c| zurueck.get(&c).copied()).collect();
            }
        }
        Ok(Self { texte })
    }

    pub fn text(&self, ids: &[u32]) -> String {
        let bytes: Vec<u8> = ids.iter().filter(|&&i| (i as usize) < self.texte.len()).flat_map(|&i| self.texte[i as usize].clone()).collect();
        String::from_utf8_lossy(&bytes).trim().to_string()
    }
}

#[cfg(test)]
mod proben {

    /// Tief verschachtelt endet es mit einem Fehler, nicht mit einem
    /// gesprengten Stapel.
    #[test]
    fn tief_verschachteltes_json_sprengt_nichts() {
        let tief = vec![b'['; 100_000];
        assert!(json(&tief, &mut 0).is_err());
        let hundert = format!("{}{}", "[".repeat(100), "]".repeat(100));
        assert!(json(hundert.as_bytes(), &mut 0).is_ok(), "hundert Ebenen lesen sich");
    }

    use super::*;

    #[test]
    fn halbe_zahlen_werden_genau_gelesen() {
        assert_eq!(halb_zu_f32(0x3C00), 1.0);
        assert_eq!(halb_zu_f32(0xC000), -2.0);
        assert_eq!(halb_zu_f32(0x0000), 0.0);
        assert_eq!(halb_zu_f32(0x7BFF), 65_504.0);
        assert_eq!(halb_zu_f32(0x0001), 2f32.powi(-24), "kleinste subnormale");
        assert!(halb_zu_f32(0x7E00).is_nan());
        // Erschoepfend: jede endliche f16 zurueck und hin.
        for h in 0u16..=0xFFFF {
            let f = halb_zu_f32(h);
            if f.is_finite() && h & 0x7FFF != 0 {
                let e = ((h >> 10) & 0x1F) as i32;
                let m = f32::from(h & 0x3FF);
                let erwartet = if e == 0 { m * 2f32.powi(-24) } else { (1.0 + m / 1024.0) * 2f32.powi(e - 15) };
                let erwartet = if h & 0x8000 != 0 { -erwartet } else { erwartet };
                assert_eq!(f, erwartet, "{h:#06x}");
            }
        }
    }

    #[test]
    fn der_json_kopf_wird_gelesen() {
        let k = br#"{"a": {"dtype": "F16", "shape": [2, 3], "data_offsets": [0, 12]}, "__metadata__": {"x": "y"}}"#;
        let Json::Objekt(o) = json(k, &mut 0).expect("lesbar") else { panic!("kein Objekt") };
        assert_eq!(o.len(), 2);
        assert!(json(b"{\"a\" 1}", &mut 0).is_err());
        assert_eq!(json(br#""a\"b\u00fc\n""#, &mut 0), Ok(Json::Text("a\"bü\n".into())));
    }

    #[test]
    fn gelu_und_erf_stimmen() {
        assert!((erf(1.0) - 0.842_700_792_9).abs() < 2e-7);
        assert!((erf(-0.5) + 0.520_499_877_8).abs() < 2e-7);
        assert!((gelu(1.0) - 0.841_344_7).abs() < 1e-6);
        assert_eq!(gelu(0.0), 0.0);
    }

    #[test]
    fn die_sprachen_kommen_aus_den_zusatztoken() {
        let ort = std::env::temp_dir().join(format!("whisper-sprachen-{}.json", std::process::id()));
        std::fs::write(&ort, r#"{"<|endoftext|>": 50257, "<|startoftranscript|>": 50258, "<|en|>": 50259, "<|de|>": 50261, "<|translate|>": 50359, "<|nospeech|>": 50363}"#).expect("schreiben");
        let s = Sprachen::laden(&ort).expect("lesbar");
        let _ = std::fs::remove_file(&ort);
        assert_eq!((s.token("de"), s.token("en"), s.anzahl()), (Some(DEUTSCH), Some(ENGLISCH), 2));
        assert_eq!(s.token("translate"), None, "kein Sprachtoken");
        assert_eq!(s.token("nospeech"), None, "hinter den Sprachen");
    }

    #[test]
    fn stille_wird_nicht_mitgeschrieben() {
        let ton = |s: f64, pegel: f64| -> Vec<f32> {
            (0..(s * 16_000.0) as usize).map(|n| (pegel * (n as f64 * 0.07).sin()) as f32).collect()
        };
        assert!(!gesprochen(&vec![0.0; 16_000]), "Stille");
        assert!(!gesprochen(&ton(0.13, 0.3)), "ein Rest von 0,13 s");
        assert!(!gesprochen(&ton(2.0, 0.003)), "Rauschen bei -50 dBFS");
        assert!(gesprochen(&ton(1.0, 0.3)), "eine Sekunde Ton");
        // Gegenprobe an der Grenze: 0,3 s genuegen, 0,28 s nicht.
        assert!(gesprochen(&ton(0.3, 0.3)) && !gesprochen(&ton(0.28, 0.3)));
    }

    fn zwischendatei(name: &str, inhalt: &[u8]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("whisper-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&d).expect("Ordner");
        let f = d.join(name);
        std::fs::write(&f, inhalt).expect("schreiben");
        f
    }

    #[test]
    fn die_groesse_folgt_der_maschine() {
        let wurzel = std::env::temp_dir().join(format!("whisper-groessen-{}", std::process::id()));
        for g in GROESSEN {
            std::fs::create_dir_all(wurzel.join(g)).expect("Ordner");
            std::fs::write(wurzel.join(g).join("model.safetensors"), b"x").expect("Datei");
            std::fs::write(wurzel.join(g).join("config.json"), b"{}").expect("Datei");
        }
        let name = |o: Option<std::path::PathBuf>| o.and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
        assert_eq!(name(groesse_waehlen(&wurzel, true, 4, |_| true)).as_deref(), Some("whisper-large-v3-turbo"), "mit Grafikeinheit");
        assert_eq!(name(groesse_waehlen(&wurzel, false, 16, |_| true)).as_deref(), Some("whisper-large-v3-turbo"), "viele Kerne");
        assert_eq!(name(groesse_waehlen(&wurzel, false, 4, |_| true)).as_deref(), Some("whisper-small"), "schwache Maschine");
        // Die Speicherwache lehnt small ab: base.
        let ohne_small = |o: &Path| !o.ends_with("whisper-small");
        assert_eq!(name(groesse_waehlen(&wurzel, false, 4, ohne_small)).as_deref(), Some("whisper-base"));
        // Gegenprobe: Fehlt small, faellt die Wahl nicht auf large-v3-turbo.
        std::fs::remove_dir_all(wurzel.join("whisper-small")).expect("weg");
        assert_eq!(name(groesse_waehlen(&wurzel, false, 4, |_| true)).as_deref(), Some("whisper-base"));
        assert_eq!(groesse_waehlen(&wurzel, false, 4, |_| false), None, "nichts passt");
        let _ = std::fs::remove_dir_all(&wurzel);
    }

    #[test]
    fn die_masse_kommen_aus_der_konfiguration() {
        let gut = br#"{"d_model": 768, "encoder_attention_heads": 12, "decoder_attention_heads": 12, "encoder_layers": 12, "decoder_layers": 12, "num_mel_bins": 80, "max_source_positions": 1500, "max_target_positions": 448}"#;
        let m = Masse::laden(&zwischendatei("config.json", gut)).expect("lesbar");
        assert_eq!((m.breite, m.koepfe, m.ebenen_enc, m.ebenen_dec, m.baender, m.fenster, m.stellen), (768, 12, 12, 12, 80, 1500, 448));
        // Gegenprobe: Koepfe nicht zu 64 und ein fehlendes Mass.
        let schief = String::from_utf8_lossy(gut).replace("\"encoder_attention_heads\": 12", "\"encoder_attention_heads\": 8");
        assert!(Masse::laden(&zwischendatei("config.json", schief.as_bytes())).is_err());
        assert!(Masse::laden(&zwischendatei("config.json", br#"{"d_model": 768}"#)).is_err());
    }

    #[test]
    fn die_sondertoken_folgen_der_datei() {
        let alt = br#"{"<|startoftranscript|>": 50258, "<|translate|>": 50358, "<|transcribe|>": 50359, "<|notimestamps|>": 50363}"#;
        let neu = br#"{"<|endoftext|>": 50257, "<|startoftranscript|>": 50258, "<|translate|>": 50359, "<|transcribe|>": 50360, "<|notimestamps|>": 50364}"#;
        let a = Sondertoken::laden(&zwischendatei("added_tokens.json", alt)).expect("aeltere Lage");
        let n = Sondertoken::laden(&zwischendatei("added_tokens.json", neu)).expect("large-v3");
        assert_eq!((a.mitschreiben, a.ohne_zeiten, a.uebersetzen), (50_359, 50_363, 50_358));
        assert_eq!((n.mitschreiben, n.ohne_zeiten, n.uebersetzen), (50_360, 50_364, 50_359));
        // Gegenprobe: ein Ende an falscher Stelle.
        let falsch = String::from_utf8_lossy(neu).replace("50257", "50256");
        assert!(Sondertoken::laden(&zwischendatei("added_tokens.json", falsch.as_bytes())).is_err());
    }

    /// F16 und F32 aus einer selbst gebauten Safetensors-Datei, einzeln von
    /// der Platte gelesen; ein Tensor, der ueber die Datei ragt, wird
    /// abgewiesen.
    #[test]
    fn tensoren_kommen_einzeln_von_der_platte() {
        let kopf = br#"{"model.encoder.a": {"dtype": "F16", "shape": [2], "data_offsets": [0, 4]}, "model.decoder.b": {"dtype": "F32", "shape": [2], "data_offsets": [4, 12]}, "lm_head": {"dtype": "F32", "shape": [1], "data_offsets": [12, 16]}}"#;
        let mut roh = (kopf.len() as u64).to_le_bytes().to_vec();
        roh.extend_from_slice(kopf);
        roh.extend_from_slice(&0x3C00u16.to_le_bytes());
        roh.extend_from_slice(&0xC000u16.to_le_bytes());
        roh.extend_from_slice(&1.5f32.to_le_bytes());
        roh.extend_from_slice(&(-0.25f32).to_le_bytes());
        roh.extend_from_slice(&9f32.to_le_bytes());
        let f = zwischendatei("model.safetensors", &roh);
        let e = kopf_lesen(&f, whisper_name).expect("Kopf");
        assert_eq!(e.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["decoder.b", "encoder.a"], "nur Encoder und Decoder");
        let w: Vec<Vec<f32>> = e.iter().map(|e| tensor_lesen(&f, e).expect("Tensor").werte).collect();
        assert_eq!(w, [vec![1.5, -0.25], vec![1.0, -2.0]]);
        roh.truncate(roh.len() - 5);
        assert!(kopf_lesen(&zwischendatei("model.safetensors", &roh), whisper_name).is_err(), "ragt ueber die Datei");
    }

    /// ⛔️ **Ein praeparierter Kopf wird abgewiesen, ohne Panik und ohne
    /// Ueberlauf**: `data_offsets` mit einer Zahl, Lagen und Formen ueber
    /// jede Groesse, eine Form, deren Produkt ueberlaeuft, negative und
    /// gebrochene Zahlen; ein Tensor ohne Form hat keine Zeilen.
    #[test]
    fn ein_praeparierter_kopf_wird_abgewiesen() {
        let datei = |eintrag: &str| {
            let kopf = format!(r#"{{"model.encoder.a": {eintrag}}}"#);
            let mut roh = (kopf.len() as u64).to_le_bytes().to_vec();
            roh.extend_from_slice(kopf.as_bytes());
            roh.extend_from_slice(&[0; 16]);
            zwischendatei("praepariert.safetensors", &roh)
        };
        for (eintrag, grund) in [
            (r#"{"dtype": "F32", "shape": [1], "data_offsets": [0]}"#, "zwei Zahlen"),
            (r#"{"dtype": "F32", "shape": [1], "data_offsets": [0, 1e30]}"#, "unlesbar"),
            (r#"{"dtype": "F32", "shape": [1], "data_offsets": [0, 18446744073709551615]}"#, "unlesbar"),
            (r#"{"dtype": "F32", "shape": [-1], "data_offsets": [0, 4]}"#, "Form"),
            (r#"{"dtype": "F32", "shape": [1.5], "data_offsets": [0, 4]}"#, "Form"),
            (r#"{"dtype": "F32", "shape": [16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16], "data_offsets": [0, 0]}"#, "Form"),
            (r#"{"dtype": "F32", "shape": [2], "data_offsets": [4, 20]}"#, "ausserhalb"),
        ] {
            let aus = kopf_lesen(&datei(eintrag), whisper_name);
            assert!(matches!(&aus, Err(f) if f.contains(grund)), "{eintrag}: {aus:?}");
        }
        let f = datei(r#"{"dtype": "F32", "shape": [], "data_offsets": [0, 4]}"#);
        let e = kopf_lesen(&f, whisper_name).expect("ein Skalar ist erlaubt");
        assert!(zeile_lesen(&f, &e[0], 0).is_err());
    }

    /// Zufallszahlen in `[-1, 1)`, ganzzahlig erzeugt.
    fn zufall(n: usize, mut s: u64) -> Vec<f32> {
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                ((s >> 40) as i64 - (1 << 23)) as f32 / (1 << 23) as f32
            })
            .collect()
    }

    fn genau_f64(x: &[f32], w: &[f32], aus: usize) -> Vec<f64> {
        let ein = w.len() / aus;
        x.chunks(ein)
            .flat_map(|xz| w.chunks(ein).map(move |wz| xz.iter().zip(wz).map(|(a, b)| f64::from(*a) * f64::from(*b)).sum::<f64>()))
            .collect()
    }

    fn abstand(a: &[f32], b: &[f64]) -> f64 {
        let fehler: f64 = a.iter().zip(b).map(|(x, y)| (f64::from(*x) - y).powi(2)).sum();
        (fehler / b.iter().map(|y| y * y).sum::<f64>()).sqrt()
    }

    /// **Der ganzzahlige Weg rechnet die Matrix**, gebuendelt wie einzeln,
    /// auch wenn Fenster und Kanaele um Groessenordnungen streuen.
    #[test]
    fn ganzzahlig_wie_genau() {
        let (aus, ein) = (96, 200);
        let mut w = zufall(aus * ein, 7);
        // Ein Kanal hundertmal kleiner: Er braucht eigene Nachkommabits.
        w[..ein].iter_mut().for_each(|v| *v *= 0.01);
        let m = Ganzmatrix::aus_f32(&w, aus);
        for zeilen in [3, 40] {
            let mut x = zufall(zeilen * ein, 11 + zeilen as u64);
            // Ein Fenster tausendmal lauter: Es braucht eine eigene Skala.
            x[..ein].iter_mut().for_each(|v| *v *= 1000.0);
            let genau = genau_f64(&x, &w, aus);
            let y = linear_ganz(&x, zeilen, &m);
            // Je Fenster und je Kanal fuer sich, sonst verdeckte das laute
            // Fenster jeden Fehler der leisen.
            for r in 1..zeilen {
                let d = abstand(&y[r * aus..][..aus], &genau[r * aus..][..aus]);
                assert!(d < 1e-4, "{zeilen} Zeilen, Fenster {r}: {d:e}");
            }
            let kanal0: Vec<f32> = (1..zeilen).map(|r| y[r * aus]).collect();
            let genau0: Vec<f64> = (1..zeilen).map(|r| genau[r * aus]).collect();
            let d = abstand(&kanal0, &genau0);
            assert!(d < 1e-3, "{zeilen} Zeilen, leiser Kanal: {d:e}");
        }
    }

    /// **W8A8 rechnet die Matrix**, gröber als W8A16, aber je Fenster und
    /// je Kanal eingestellt; Gegenprobe: ohne eigene Bits je Kanal bliebe
    /// der leise Kanal auf null.
    #[test]
    fn w8a8_rechnet_die_matrix() {
        let (aus, ein, zeilen) = (96, 200, 40);
        let mut w = zufall(aus * ein, 7);
        w[..ein].iter_mut().for_each(|v| *v *= 0.01);
        let m = Ganzmatrix::aus_f32(&w, aus);
        let mut x = zufall(zeilen * ein, 13);
        x[..ein].iter_mut().for_each(|v| *v *= 1000.0);
        let genau = genau_f64(&x, &w, aus);
        let y = linear_a8(&x, zeilen, &m);
        for r in 0..zeilen {
            let d = abstand(&y[r * aus..][..aus], &genau[r * aus..][..aus]);
            assert!(d < 2e-2, "Fenster {r}: {d:e}");
        }
        let kanal0: Vec<f32> = (0..zeilen).map(|r| y[r * aus]).collect();
        let genau0: Vec<f64> = (0..zeilen).map(|r| genau[r * aus]).collect();
        let d = abstand(&kanal0, &genau0);
        assert!(d < 2e-2, "leiser Kanal: {d:e}");
    }

    /// Gegenprobe: Nur mit der ersten Stelle waere derselbe Fehler um
    /// Groessenordnungen groesser; die zweite traegt also.
    #[test]
    fn ohne_zweite_stelle_waere_es_ungenau() {
        let (aus, ein) = (64, 200);
        let w = zufall(aus * ein, 3);
        let mut m = Ganzmatrix::aus_f32(&w, aus);
        m.stellen.w[aus * ein..].iter_mut().for_each(|v| *v = 0);
        let x = zufall(3 * ein, 5);
        let d = abstand(&linear_ganz(&x, 3, &m), &genau_f64(&x, &w, aus));
        assert!(d > 1e-3, "nur eine Stelle: {d:e}");
    }
}
