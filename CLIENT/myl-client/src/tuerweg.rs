//! **Der Agent über die Tür eines Knotens** (2026-11-15).
//!
//! # ⚑ Was hier steht
//!
//! Ein [`Modellweg`], der nicht auf dieser Maschine rechnet, sondern im
//! Netz: Die Schleife des Agenten bleibt dieselbe, Werkzeuge laufen hier,
//! jede Runde des Modells geht über die Tür eines Knotens an einen Pod und
//! wird mit der Vollmacht des Agenten bezahlt.
//!
//! # ⚑ Die Vorlage setzt der Klient
//!
//! Der Prompt entsteht mit derselben [`Vorlage`] wie für ein Modell auf
//! dieser Maschine, Zeichen für Zeichen, und geht als fertiger Prompt an
//! `/v1/completions`. 📌 Über `/v1/chat/completions` setzte die Tür die
//! Unterhaltung als `rolle: inhalt` zusammen (Fund 181), eine Form, auf der
//! das Modell nie trainiert wurde; ein Agent bekäme dort seltener einen
//! Werkzeugaufruf, und niemand sähe dem Ergebnis an, warum.
//!
//! # ⚑ Ohne Denkblock
//!
//! Die Antwort beginnt hinter einem leeren Denkblock. Auf dieser Maschine
//! begrenzt ein Denkbudget das Überlegen (Fund 512); über die Tür gibt es
//! keines, und ein Denkmodell überlegte dort bis zum Deckel, ohne zum
//! Werkzeugaufruf zu kommen.

use std::sync::atomic::{AtomicU64, Ordering};

use myl_local_agent::{Antwort, Modellweg, Nachricht, Tuerfehler, Tuerklient};
use myl_types::inferenzauftrag::{obergrenze, preis, MAX_NEUE_TOKEN};

use crate::oertlich::Vorlage;

/// Die Endmarken, an denen eine Antwort spätestens abgeschnitten wird.
///
/// 📌 Eine Tür vor Fund 702 rechnete bis zum Deckel weiter; was nach der
/// Endmarke kommt, ist eine erfundene nächste Runde und gehört nicht zur
/// Antwort. Ein Werkzeugaufruf daraus wäre einer, den niemand verlangt hat.
pub const ENDMARKEN: [&str; 2] = ["<|im_end|>", "<|endoftext|>"];

/// Ein Modellweg über die Tür, mit der Vorlage des Klienten.
pub struct Vorlagentuer {
    tuer: Tuerklient,
    vorlage: Vorlage,
    modell: String,
    /// Hoechstens so viele Token je Runde, gleich was die Schleife verlangt.
    deckel: Option<u32>,
    /// Runden, Token hinein und Token heraus, seit dem Anfang.
    runden: AtomicU64,
    hinein: AtomicU64,
    heraus: AtomicU64,
    /// Was die Tuer dafuer abrechnet: je Runde Prompt und Ausgabe
    /// ([`preis`], Fund 711).
    abgerechnet: AtomicU64,
    /// Die Bytes der Prompts, die gerechnet wurden: mit `hinein` das
    /// Verhaeltnis, aus dem der Bedarf der naechsten Runde geschaetzt wird.
    bytes_hinein: AtomicU64,
    /// Die teuerste Runde bisher, in Credits.
    teuerste: AtomicU64,
    /// Die längste Runde bisher, in Millisekunden.
    laengste_ms: AtomicU64,
    /// Läuft vor jeder Runde (Fund 707), siehe [`Self::mit_vorlauf`].
    vorlauf: Option<Vorlauf>,
    /// Wie lange vor jedem neuen Anlauf nach einem vorübergehenden Fehler
    /// gewartet wird, siehe [`WIEDERHOLUNG`].
    wiederholung: Vec<std::time::Duration>,
}

/// **Nach einem vorübergehenden Fehler der Tür noch einmal**, nach diesen
/// Pausen: 15, 30 und 60 s.
///
/// 📌 **Fund 712 (Auftragslauf 2026-11-17):** Der Agent hatte die
/// Nebenkostenabrechnung richtig geprüft (Anteil der Reparatur 158,45,
/// Nachzahlung 245,49); die nächste Runde fiel auf den Epochenwechsel, die
/// Tür meldete 502 „kein Pod hat gerechnet“, und der ganze Auftrag war
/// verloren. Am Wechsel wartet der Ortsdienst bis zu 60 s auf den neuen Pod
/// (`myl_pod::podwerk::BEREIT_FRIST`); die Pausen reichen zusammen darüber
/// hinaus.
///
/// ⚑ **Nur 502, 503 und 504**, also wenn nicht gerechnet wurde. Die Tür
/// rechnet nur ab, was sie ausliefert; ein neuer Anlauf kostet nichts
/// doppelt. Eine Ablehnung (401, 402, 429) ist keine Störung, sondern eine
/// Antwort.
pub const WIEDERHOLUNG: [std::time::Duration; 3] =
    [std::time::Duration::from_secs(15), std::time::Duration::from_secs(30), std::time::Duration::from_secs(60)];

/// Ob die Tür nicht gerechnet hat und ein neuer Anlauf Sinn hat.
pub fn voruebergehend(f: &Tuerfehler) -> bool {
    matches!(f, Tuerfehler::Abgelehnt { status: 502..=504, .. })
}

/// So viele Token verlangt die Tür, wenn eine Anfrage keinen Deckel nennt.
pub const TUER_VORGABE_TOKEN: u32 = 256;

/// ⚑ **Was eine Runde voraussichtlich höchstens kostet** (Fund 711): Prompt
/// und Deckel, die Prompt-Token geschätzt aus dem Verhältnis der bisherigen
/// Runden mit einem Achtel Zuschlag, nie über der sicheren Schranke aus
/// Bytes und Deckel ([`obergrenze`]). Vor der ersten Runde gilt die Schranke.
///
/// Den Preis kennt erst der Pod, denn nur er zählt die Token. Die Schranke
/// ist rund das Dreifache; wer mit ihr nachkauft, kauft Credits, die kurz
/// danach verfallen. Schätzt dieser Klient zu knapp, lehnt die Tür wegen des
/// Guthabens ab, und der zweite Vorlauf nimmt die Schranke.
pub fn bedarf_schaetzen(prompt_bytes: u64, max_tokens: u32, bisher_bytes: u64, bisher_token: u64) -> u64 {
    let schranke = obergrenze(prompt_bytes, max_tokens);
    if bisher_bytes == 0 || bisher_token == 0 {
        return schranke;
    }
    let token = (u128::from(prompt_bytes) * u128::from(bisher_token) * 9).div_ceil(u128::from(bisher_bytes) * 8);
    u64::try_from(token)
        .unwrap_or(u64::MAX)
        .saturating_add(u64::from(max_tokens.min(MAX_NEUE_TOKEN)))
        .min(schranke)
}

/// Was der Vorlauf vor einer Runde erfährt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rundenlage {
    /// So viele Credits kostet die Runde voraussichtlich höchstens
    /// ([`bedarf_schaetzen`]); nach einer Ablehnung wegen des Guthabens die
    /// sichere Schranke.
    pub bedarf: u64,
    /// Die teuerste Runde bisher, in Credits; null vor der ersten. Daran
    /// bemisst ein Nachkauf seinen Vorrat, nicht an der Schranke.
    pub je_runde: u64,
    /// Was bisher über die Tür ging.
    pub stand: Tuerstand,
    /// Was die Tür dafür abrechnet: je Runde Prompt und Ausgabe
    /// ([`preis`], Fund 711).
    pub abgerechnet: u64,
    /// Die längste Runde bisher; null, solange keine gemessen ist.
    pub laengste_runde: std::time::Duration,
    /// Die Tür hat diese Runde eben wegen des Guthabens abgelehnt.
    pub abgelehnt: bool,
}

/// Der Vorlauf einer Runde: `Err` mit dem Grund, wenn sie nicht beginnen
/// soll.
pub type Vorlauf = Box<dyn Fn(&Rundenlage) -> Result<(), String>>;

/// Woran dieser Klient erkennt, dass die Tür wegen des Guthabens ablehnte.
const GUTHABEN_FEHLT: &str = "Guthaben";

/// Was über die Tür gerechnet wurde: Runden, Token hinein, Token heraus.
/// Abgebucht werden beide ([`preis`], Fund 711).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tuerstand {
    pub runden: u64,
    pub hinein: u64,
    pub heraus: u64,
}

impl Vorlagentuer {
    /// Über `tuer`, mit `vorlage`, für das Modell `modell` (der Name, den die
    /// Tür in der Antwort zurückgibt; er wählt nichts aus).
    pub fn neu(tuer: Tuerklient, vorlage: Vorlage, modell: impl Into<String>) -> Self {
        Self {
            tuer,
            vorlage,
            modell: modell.into(),
            deckel: None,
            runden: AtomicU64::new(0),
            hinein: AtomicU64::new(0),
            heraus: AtomicU64::new(0),
            abgerechnet: AtomicU64::new(0),
            bytes_hinein: AtomicU64::new(0),
            teuerste: AtomicU64::new(0),
            laengste_ms: AtomicU64::new(0),
            vorlauf: None,
            wiederholung: WIEDERHOLUNG.to_vec(),
        }
    }

    /// Andere Pausen vor einem neuen Anlauf (leer: keiner), siehe
    /// [`WIEDERHOLUNG`].
    pub fn mit_wiederholung(mut self, pausen: Vec<std::time::Duration>) -> Self {
        self.wiederholung = pausen;
        self
    }

    /// ⚑ **Vor jeder Runde läuft `vorlauf`** (Fund 707): der Ort für den
    /// Nachkauf von Credits. Lehnt er ab, geht die Runde nicht an die Tür,
    /// und die Schleife sieht eine Ablehnung mit seinem Grund. Lehnt die
    /// Tür trotz Vorlauf wegen des Guthabens ab (ihr Knoten sieht den Kauf
    /// noch nicht), läuft er einmal mit `abgelehnt` und die Runde danach
    /// noch einmal.
    pub fn mit_vorlauf(mut self, vorlauf: Vorlauf) -> Self {
        self.vorlauf = Some(vorlauf);
        self
    }

    /// **Höchstens `deckel` Token je Runde** (2026-11-15).
    ///
    /// 📌 Die Schleife verlangt je Runde mindestens 4 000 Token
    /// (`agentenwahl::agent_antwortlaenge`), für eine Maschine, die das in
    /// Sekunden rechnet. Über die Pods des 35B sind es 3 bis 4 Token je
    /// Sekunde, und die Tür wartet je Auftrag höchstens 540 s: Eine Runde mit
    /// 1 487 Token brauchte im Auftragslauf 356 s. Was über den Deckel geht,
    /// schreibt die Schleife weiter (`weiterschreiben`).
    pub fn mit_deckel(mut self, deckel: u32) -> Self {
        self.deckel = Some(deckel.max(1));
        self
    }

    /// Was bisher über die Tür ging.
    pub fn stand(&self) -> Tuerstand {
        Tuerstand {
            runden: self.runden.load(Ordering::Relaxed),
            hinein: self.hinein.load(Ordering::Relaxed),
            heraus: self.heraus.load(Ordering::Relaxed),
        }
    }

    /// Der Prompt, wie er an die Tür geht.
    pub fn prompt(&self, nachrichten: &[Nachricht]) -> String {
        crate::oertlich::bauen(self.vorlage, nachrichten, false)
    }
}

/// Schneidet `text` an der ersten Endmarke ab.
pub fn bis_zur_endmarke(text: &mut String) {
    let ende = ENDMARKEN.iter().filter_map(|m| text.find(m)).min();
    if let Some(i) = ende {
        text.truncate(i);
    }
}

impl Vorlagentuer {
    /// Eine Runde über die Tür, gezählt und an der Endmarke geschnitten.
    fn runde(&self, prompt: &str, max_tokens: Option<u32>) -> Result<Antwort, Tuerfehler> {
        let max_tokens = match (max_tokens, self.deckel) {
            (Some(m), Some(d)) => Some(m.min(d)),
            (None, d) => d,
            (m, None) => m,
        };
        let deckel = max_tokens.unwrap_or(TUER_VORGABE_TOKEN);
        let bytes = prompt.len() as u64;
        let lage = |abgelehnt: bool| Rundenlage {
            bedarf: if abgelehnt {
                obergrenze(bytes, deckel)
            } else {
                bedarf_schaetzen(bytes, deckel, self.bytes_hinein.load(Ordering::Relaxed), self.hinein.load(Ordering::Relaxed))
            },
            je_runde: self.teuerste.load(Ordering::Relaxed),
            stand: self.stand(),
            abgerechnet: self.abgerechnet.load(Ordering::Relaxed),
            laengste_runde: std::time::Duration::from_millis(self.laengste_ms.load(Ordering::Relaxed)),
            abgelehnt,
        };
        let nachkauf = |abgelehnt: bool| match &self.vorlauf {
            Some(v) => v(&lage(abgelehnt)).map_err(|grund| Tuerfehler::Abgelehnt { status: 402, rumpf: format!("Nachkauf: {grund}") }),
            None => Ok(()),
        };
        nachkauf(false)?;
        let anfang = std::time::Instant::now();
        let einmal = || match self.tuer.vervollstaendigen(&self.modell, prompt, max_tokens) {
            Err(Tuerfehler::Abgelehnt { status: 402, rumpf }) if self.vorlauf.is_some() && rumpf.contains(GUTHABEN_FEHLT) => {
                nachkauf(true)?;
                self.tuer.vervollstaendigen(&self.modell, prompt, max_tokens)
            }
            anders => anders,
        };
        let mut ergebnis = einmal();
        for pause in &self.wiederholung {
            match &ergebnis {
                Err(f) if voruebergehend(f) => {
                    eprintln!("[myl] die Tuer meldet {f}; neuer Anlauf in {} s", pause.as_secs());
                    std::thread::sleep(*pause);
                    ergebnis = einmal();
                }
                _ => break,
            }
        }
        let mut a = ergebnis?;
        self.laengste_ms.fetch_max(u64::try_from(anfang.elapsed().as_millis()).unwrap_or(u64::MAX), Ordering::Relaxed);
        self.runden.fetch_add(1, Ordering::Relaxed);
        self.hinein.fetch_add(u64::from(a.prompt_token), Ordering::Relaxed);
        self.heraus.fetch_add(u64::from(a.antwort_token), Ordering::Relaxed);
        let kosten = preis(u64::from(a.prompt_token), u64::from(a.antwort_token));
        self.abgerechnet.fetch_add(kosten, Ordering::Relaxed);
        self.teuerste.fetch_max(kosten, Ordering::Relaxed);
        self.bytes_hinein.fetch_add(bytes, Ordering::Relaxed);
        bis_zur_endmarke(&mut a.text);
        Ok(a)
    }
}

impl Modellweg for Vorlagentuer {
    fn chat(&self, _modell: &str, nachrichten: &[Nachricht], max_tokens: Option<u32>) -> Result<Antwort, Tuerfehler> {
        self.runde(&self.prompt(nachrichten), max_tokens)
    }

    /// ⚑ **Eine abgeschnittene Antwort fortsetzen** (2026-11-15): derselbe
    /// Prompt, dahinter der angefangene Text, und die Tür rechnet weiter.
    /// Die Schleife setzt so einen offenen Werkzeugaufruf fort, der am Deckel
    /// einer Runde abbrach, statt ihn zu verwerfen.
    ///
    /// ⚠️ Anders als auf dieser Maschine geht der Zustand nicht weiter: Der
    /// Pod rechnet den ganzen Prompt neu, und der angefangene Text wird neu in
    /// Token zerlegt, an der Naht nicht unbedingt in dieselben. Der Text ist
    /// derselbe.
    fn weiterschreiben(
        &self,
        _modell: &str,
        nachrichten: &[Nachricht],
        anfang: &str,
        max_tokens: Option<u32>,
    ) -> Option<Result<Antwort, Tuerfehler>> {
        Some(self.runde(&format!("{}{anfang}", self.prompt(nachrichten)), max_tokens))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Eine Tür, die eine Anfrage liest und einmal antwortet.
    fn stummel(antwort: &'static str) -> (u16, std::sync::mpsc::Receiver<String>) {
        let horcher = TcpListener::bind("127.0.0.1:0").expect("binden");
        let port = horcher.local_addr().expect("Adresse").port();
        let (sender, empfaenger) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut strom, _) = horcher.accept().expect("annehmen");
            let mut roh = Vec::new();
            let mut puffer = [0u8; 4096];
            // Bis zum Ende der Koepfe, dann so viel Rumpf, wie angekuendigt.
            loop {
                let n = strom.read(&mut puffer).unwrap_or(0);
                roh.extend_from_slice(&puffer[..n]);
                if let Some(i) = roh.windows(4).position(|f| f == b"\r\n\r\n") {
                    let kopf = String::from_utf8_lossy(&roh[..i]).to_ascii_lowercase();
                    let laenge: usize = kopf
                        .lines()
                        .find_map(|z| z.strip_prefix("content-length:").map(|w| w.trim().parse().unwrap_or(0)))
                        .unwrap_or(0);
                    while roh.len() < i + 4 + laenge {
                        let n = strom.read(&mut puffer).unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        roh.extend_from_slice(&puffer[..n]);
                    }
                    break;
                }
                if n == 0 {
                    break;
                }
            }
            let _ = sender.send(String::from_utf8_lossy(&roh).into_owned());
            let _ = strom.write_all(
                format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{antwort}", antwort.len())
                    .as_bytes(),
            );
        });
        (port, empfaenger)
    }

    const ANTWORT: &str = r#"{"id":"myl-1","choices":[{"text":"<tool_call>a</tool_call><|im_end|>\n<|im_start|>user\nerfunden","finish_reason":"stop"}],"usage":{"prompt_tokens":30,"completion_tokens":9}}"#;

    /// ⚑ **Der Prompt ist der der Vorlage, und die Antwort endet an der
    /// Endmarke.** Gegenprobe zu beidem: ein Prompt in anderer Form oder eine
    /// Antwort mit der erfundenen Runde lässt die Prüfung fallen.
    #[test]
    fn die_runde_geht_in_der_vorlage_hinaus_und_endet_an_der_marke() {
        let (port, gesehen) = stummel(ANTWORT);
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven");
        let nachrichten = [Nachricht::system("sei knapp"), Nachricht::nutzer("Lies a.txt")];
        let a = weg.chat("egal", &nachrichten, Some(64)).expect("Antwort");
        assert_eq!(a.text, "<tool_call>a</tool_call>");
        assert_eq!(a.antwort_token, 9);
        assert_eq!(weg.stand(), Tuerstand { runden: 1, hinein: 30, heraus: 9 });

        let anfrage = gesehen.recv_timeout(std::time::Duration::from_secs(5)).expect("Anfrage");
        assert!(anfrage.starts_with("POST /v1/completions "), "{anfrage}");
        let rumpf = &anfrage[anfrage.find("\r\n\r\n").expect("Rumpf") + 4..];
        let v: serde_json::Value = serde_json::from_str(rumpf).expect("JSON");
        assert_eq!(v["prompt"].as_str(), Some(crate::oertlich::bauen(Vorlage::ChatMlDenkblock, &nachrichten, false).as_str()));
        assert!(v["prompt"].as_str().unwrap_or_default().ends_with("<|im_start|>assistant\n<think>\n\n</think>\n\n"));
        assert_eq!(v["max_tokens"], 64);
    }

    /// ⚑ Weiterschreiben schickt den Prompt der Vorlage mit dem angefangenen
    /// Text dahinter und zählt die Runde mit.
    #[test]
    fn weiterschreiben_haengt_den_anfang_an_den_prompt() {
        let (port, gesehen) = stummel(ANTWORT);
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven");
        let nachrichten = [Nachricht::nutzer("Schreib a.txt")];
        let a = weg.weiterschreiben("egal", &nachrichten, "<tool_call>{\"name\": \"write_file\"", Some(32)).expect("Weg kann es").expect("Antwort");
        assert_eq!(a.text, "<tool_call>a</tool_call>");
        assert_eq!(weg.stand().runden, 1);
        let anfrage = gesehen.recv_timeout(std::time::Duration::from_secs(5)).expect("Anfrage");
        let rumpf = &anfrage[anfrage.find("\r\n\r\n").expect("Rumpf") + 4..];
        let v: serde_json::Value = serde_json::from_str(rumpf).expect("JSON");
        let soll = format!("{}<tool_call>{{\"name\": \"write_file\"", crate::oertlich::bauen(Vorlage::ChatMlDenkblock, &nachrichten, false));
        assert_eq!(v["prompt"].as_str(), Some(soll.as_str()));
    }

    /// ⚑ Der Deckel gilt je Runde, auch wenn die Schleife mehr verlangt; ohne
    /// Verlangen der Schleife gilt er allein. Gegenprobe: ohne Deckel geht das
    /// Verlangen der Schleife unverändert hinaus.
    #[test]
    fn der_deckel_gilt_je_runde() {
        for (deckel, verlangt, soll) in [(Some(512), Some(4000), 512), (Some(512), None, 512), (None, Some(4000), 4000)] {
            let (port, gesehen) = stummel(ANTWORT);
            let mut weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven");
            if let Some(d) = deckel {
                weg = weg.mit_deckel(d);
            }
            weg.chat("egal", &[Nachricht::nutzer("x")], verlangt).expect("Antwort");
            let anfrage = gesehen.recv_timeout(std::time::Duration::from_secs(5)).expect("Anfrage");
            let rumpf = &anfrage[anfrage.find("\r\n\r\n").expect("Rumpf") + 4..];
            let v: serde_json::Value = serde_json::from_str(rumpf).expect("JSON");
            assert_eq!(v["max_tokens"], soll, "Deckel {deckel:?}, verlangt {verlangt:?}");
        }
    }

    /// Eine Tür, die nacheinander diese Antworten gibt, je Verbindung eine.
    fn stummel_folge(antworten: Vec<(u16, &'static str)>) -> u16 {
        let horcher = TcpListener::bind("127.0.0.1:0").expect("binden");
        let port = horcher.local_addr().expect("Adresse").port();
        std::thread::spawn(move || {
            for (status, rumpf) in antworten {
                let (mut strom, _) = horcher.accept().expect("annehmen");
                let mut roh = Vec::new();
                let mut puffer = [0u8; 4096];
                loop {
                    let n = strom.read(&mut puffer).unwrap_or(0);
                    roh.extend_from_slice(&puffer[..n]);
                    if let Some(i) = roh.windows(4).position(|f| f == b"\r\n\r\n") {
                        let kopf = String::from_utf8_lossy(&roh[..i]).to_ascii_lowercase();
                        let laenge: usize = kopf
                            .lines()
                            .find_map(|z| z.strip_prefix("content-length:").map(|w| w.trim().parse().unwrap_or(0)))
                            .unwrap_or(0);
                        while roh.len() < i + 4 + laenge {
                            let n = strom.read(&mut puffer).unwrap_or(0);
                            if n == 0 {
                                break;
                            }
                            roh.extend_from_slice(&puffer[..n]);
                        }
                        break;
                    }
                    if n == 0 {
                        break;
                    }
                }
                let wort = if status == 200 { "OK" } else { "Payment Required" };
                let _ = strom.write_all(
                    format!("HTTP/1.1 {status} {wort}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{rumpf}", rumpf.len())
                        .as_bytes(),
                );
            }
        });
        port
    }

    /// ⚑ **Der Vorlauf läuft vor der Runde, mit ihrem Deckel als Bedarf, und
    /// sein Nein hält sie auf** (Fund 707). Gegenprobe: Ginge die Runde
    /// trotzdem hinaus, stünde sie im Stand; hier gibt es nicht einmal eine
    /// Tür, die antworten könnte.
    #[test]
    fn der_vorlauf_sieht_den_deckel_und_kann_die_runde_aufhalten() {
        let gesehen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let g = gesehen.clone();
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", 1, "v"), Vorlage::ChatMlDenkblock, "raven")
            .mit_deckel(512)
            .mit_vorlauf(Box::new(move |l| {
                g.lock().unwrap().push(*l);
                Err("Budget erschöpft".into())
            }));
        let f = weg.chat("egal", &[Nachricht::nutzer("x")], Some(4000)).expect_err("aufgehalten");
        assert!(matches!(&f, Tuerfehler::Abgelehnt { status: 402, rumpf } if rumpf == "Nachkauf: Budget erschöpft"), "{f:?}");
        assert_eq!(weg.stand().runden, 0);
        let g = gesehen.lock().unwrap();
        assert_eq!(g.len(), 1);
        // Vor der ersten Runde die sichere Schranke: Bytes und Deckel (Fund 711).
        let prompt = weg.prompt(&[Nachricht::nutzer("x")]);
        assert_eq!((g[0].bedarf, g[0].je_runde, g[0].abgelehnt), (obergrenze(prompt.len() as u64, 512), 0, false));
    }

    /// ⚑ **Lehnt die Tür wegen des Guthabens ab, läuft der Vorlauf noch
    /// einmal und die Runde danach auch.** Gegenprobe im selben Test: Eine
    /// andere Ablehnung (der Kontrakt deckt nicht) wird nicht wiederholt.
    #[test]
    fn eine_ablehnung_wegen_guthaben_wird_einmal_wiederholt() {
        const NEIN: &str = r#"{"error":{"message":"das Guthaben des Inhabers deckt diesen Auftrag nicht"}}"#;
        let port = stummel_folge(vec![(402, NEIN), (200, ANTWORT)]);
        let rufe = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let r = rufe.clone();
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven").mit_vorlauf(Box::new(
            move |l| {
                r.lock().unwrap().push(l.abgelehnt);
                Ok(())
            },
        ));
        let a = weg.chat("egal", &[Nachricht::nutzer("x")], Some(64)).expect("zweiter Anlauf");
        assert_eq!(a.antwort_token, 9);
        assert_eq!(*rufe.lock().unwrap(), vec![false, true]);
        assert_eq!(weg.stand().runden, 1);

        const KONTRAKT: &str = r#"{"error":{"message":"der Kontrakt deckt diesen Auftrag nicht"}}"#;
        let port = stummel_folge(vec![(402, KONTRAKT)]);
        let rufe = std::sync::Arc::new(std::sync::Mutex::new(0u32));
        let r = rufe.clone();
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven").mit_vorlauf(Box::new(
            move |_| {
                *r.lock().unwrap() += 1;
                Ok(())
            },
        ));
        assert!(matches!(weg.chat("egal", &[Nachricht::nutzer("x")], Some(64)), Err(Tuerfehler::Abgelehnt { status: 402, .. })));
        assert_eq!(*rufe.lock().unwrap(), 1);
    }

    /// ⚑ **Fund 712: Nach einem 502 rechnet die Tür beim nächsten Anlauf**,
    /// und die Runde zählt einmal. Gegenprobe: ohne Pausen bleibt es beim
    /// 502; eine Ablehnung (402 ohne Vorlauf) wird nicht wiederholt.
    #[test]
    fn nach_einem_502_kommt_ein_neuer_anlauf() {
        const KEIN_POD: &str = r#"{"error":{"message":"kein Pod hat gerechnet"}}"#;
        let kurz = vec![std::time::Duration::from_millis(10); 3];
        let port = stummel_folge(vec![(502, KEIN_POD), (502, KEIN_POD), (200, ANTWORT)]);
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven").mit_wiederholung(kurz.clone());
        let a = weg.chat("egal", &[Nachricht::nutzer("x")], Some(64)).expect("dritter Anlauf");
        assert_eq!((a.antwort_token, weg.stand().runden), (9, 1));

        let port = stummel_folge(vec![(502, KEIN_POD), (200, ANTWORT)]);
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven").mit_wiederholung(Vec::new());
        assert!(matches!(weg.chat("egal", &[Nachricht::nutzer("x")], Some(64)), Err(Tuerfehler::Abgelehnt { status: 502, .. })));

        const KONTRAKT: &str = r#"{"error":{"message":"der Kontrakt deckt diesen Auftrag nicht"}}"#;
        let port = stummel_folge(vec![(402, KONTRAKT), (200, ANTWORT)]);
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven").mit_wiederholung(kurz);
        assert!(matches!(weg.chat("egal", &[Nachricht::nutzer("x")], Some(64)), Err(Tuerfehler::Abgelehnt { status: 402, .. })));
        assert!(voruebergehend(&Tuerfehler::Abgelehnt { status: 503, rumpf: String::new() }));
        assert!(!voruebergehend(&Tuerfehler::Abgelehnt { status: 429, rumpf: String::new() }));
    }

    /// ⚑ **Der Vorlauf rechnet so, wie die Tür abrechnet** (Fund 711):
    /// Prompt und Ausgabe. Eine Runde ohne neues Token kostet ihren Prompt
    /// (30), eine mit neun Token 38; nach beiden sieht er 68, nicht neun
    /// wie vor Fund 711. Und der Vorrat bemisst sich an der teuersten Runde.
    #[test]
    fn eine_runde_kostet_prompt_und_ausgabe() {
        const LEER: &str = r#"{"id":"myl-0","choices":[{"text":"","finish_reason":"stop"}],"usage":{"prompt_tokens":30,"completion_tokens":0}}"#;
        let port = stummel_folge(vec![(200, LEER), (200, ANTWORT), (200, ANTWORT)]);
        let gesehen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let g = gesehen.clone();
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven").mit_vorlauf(Box::new(
            move |l| {
                g.lock().unwrap().push((l.abgerechnet, l.stand.heraus, l.je_runde));
                Ok(())
            },
        ));
        for _ in 0..3 {
            weg.chat("egal", &[Nachricht::nutzer("x")], Some(64)).expect("Runde");
        }
        assert_eq!(*gesehen.lock().unwrap(), vec![(0, 0, 0), (30, 0, 30), (68, 9, 38)]);
    }

    /// ⚑ **Der Bedarf kommt aus dem Verhältnis der bisherigen Runden** (Fund
    /// 711), mit einem Achtel Zuschlag und nie über der Schranke. Gegenprobe:
    /// ohne Vorgeschichte die Schranke.
    #[test]
    fn der_bedarf_folgt_dem_verhaeltnis_und_nie_ueber_die_schranke() {
        assert_eq!(bedarf_schaetzen(12_000, 1_000, 0, 0), 13_000, "ohne Vorgeschichte die Schranke");
        // 3 Bytes je Token bisher: 12 000 Bytes sind 4 000 Token, ein Achtel
        // dazu 4 500, und der Deckel.
        assert_eq!(bedarf_schaetzen(12_000, 1_000, 30_000, 10_000), 5_500);
        // Ein Verhältnis über einem Token je Byte bleibt an der Schranke.
        assert_eq!(bedarf_schaetzen(12_000, 1_000, 1_000, 2_000), 13_000);
        // Der Deckel zählt höchstens mit dem des Protokolls.
        assert_eq!(bedarf_schaetzen(8, u32::MAX, 0, 0), 8 + u64::from(MAX_NEUE_TOKEN));
    }

    /// ⚑ **Nach einer Ablehnung wegen des Guthabens sieht der zweite Vorlauf
    /// die sichere Schranke** (Fund 711): Die Schätzung war zu knapp.
    #[test]
    fn nach_einer_ablehnung_gilt_die_schranke() {
        const NEIN: &str = r#"{"error":{"message":"das Guthaben des Inhabers deckt diesen Auftrag nicht"}}"#;
        let port = stummel_folge(vec![(200, ANTWORT), (402, NEIN), (200, ANTWORT)]);
        let gesehen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let g = gesehen.clone();
        let weg = Vorlagentuer::neu(Tuerklient::neu("127.0.0.1", port, "v"), Vorlage::ChatMlDenkblock, "raven").mit_vorlauf(Box::new(
            move |l| {
                g.lock().unwrap().push((l.bedarf, l.abgelehnt));
                Ok(())
            },
        ));
        let n = [Nachricht::nutzer("x")];
        weg.chat("egal", &n, Some(64)).expect("erste Runde");
        weg.chat("egal", &n, Some(64)).expect("zweite Runde nach dem Nachkauf");
        let bytes = weg.prompt(&n).len() as u64;
        let g = gesehen.lock().unwrap();
        assert_eq!(g.len(), 3);
        assert_eq!(g[0], (obergrenze(bytes, 64), false), "vor der ersten Runde die Schranke");
        assert_eq!(g[1], (bedarf_schaetzen(bytes, 64, bytes, 30), false), "danach die Schätzung");
        assert!(g[1].0 < g[0].0, "die Schätzung liegt unter der Schranke");
        assert_eq!(g[2], (obergrenze(bytes, 64), true), "nach der Ablehnung wieder die Schranke");
    }

    #[test]
    fn ohne_endmarke_bleibt_der_text_ganz() {
        let mut t = String::from("fertig");
        bis_zur_endmarke(&mut t);
        assert_eq!(t, "fertig");
        let mut t = String::from("a<|endoftext|>b<|im_end|>c");
        bis_zur_endmarke(&mut t);
        assert_eq!(t, "a");
    }
}
