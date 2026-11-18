//! **Das Podwerk**: ein Ortsdienst, der über den Pod der Kette rechnet statt
//! über ein Modell im eigenen Prozess (2026-10-26).
//!
//! # ⚑ Der Weg
//!
//! Die Tür eines Knotens reicht einen Auftrag über die Ortsleitung an diesen
//! Prozess; er zerlegt den Prompt in Token und fährt den Koordinator über
//! [`crate::shardweg::UeberDenKnoten`]: über die Relaisleitung des Knotens zu
//! den Knoten der Pod-Mitglieder und deren Shards. Welcher Pod, sagt das
//! Podbild der Kette; je Epoche entsteht der Koordinator neu.
//!
//! # ⚑ Das Bündel
//!
//! Ein eigener Faden sieht alle paar Sekunden nach: Liegen gegen Ende der
//! Epoche ([`EINREICHEN_AB_SECHSTEL`]) gerechnete Segmente vor und ist noch kein
//! Bündel eingereicht, lässt er es von den Positionen unterschreiben und
//! gibt es über die Leitung ab; der Knoten reicht es an die erste Position.
//! Die Kette nimmt ein Bündel je Pod und Epoche, also genau eines am Ende.
//!
//! # ⚑ Der Zwilling (Stufe 1, 2026-10-28)
//!
//! Nennt das Podbild für den ersten Pod einen Zwilling, rechnet dieser
//! jeden Auftrag ein zweites Mal, zugleich, in einem eigenen Faden mit
//! eigenem Koordinator. Die Antwort des zuerst fertigen Pods geht hinaus
//! (optimistisch, Whitepaper 6.4); beide reichen am Ende der Epoche ihr
//! Bündel ein, und die Kette gleicht sie je Sitzung ab: Gleiche Sitzungen
//! werden vergütet, abweichende bestritten, fehlende nachgerechnet.
//!
//! Der Prozess hält keine Gewichte, nur den Wortschatz.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use integer_llm_runtime::tokenizer::Tokenizer;
use myl_types::hash::Hash;
use myl_types::ids::{EpochId, PodId, SegmentId};
use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};
use myl_types::latency_attest::PeerIdBytes;
use myl_types::relais::{Podbild, Relaisantwort, Relaisfrage};

use crate::coordinator::Coordinator;
use crate::entsiegelung::Klartextwerk;
use crate::shardweg::{Ausweis, Shardanfrage, Shardantwort, Shardweg, UeberDenKnoten};

/// ⚑ **Der Rand vor dem Wechsel, in dem das Bündel eingereicht wird**
/// (2026-11-18): mindestens ein Dreißigstel der Epoche und
/// mindestens [`EINREICHEN_RAND_MS`] nach der gemessenen Blockzeit, höchstens
/// ein Sechstel ([`einreichen_ab`]).
///
/// 📌 Bis hierher galt fest das letzte Sechstel: Was danach gerechnet
/// wurde, stand in keinem Bündel, bis zu einem Sechstel der Arbeit ohne
/// Vergütung. Der Rand war nicht für den Weg in den Block nötig, sondern weil
/// das Einreichen auf laufende Sitzungen wartete (seit Fund 713 nicht mehr).
/// Gemessen im gezielten Auftragslauf vom 2026-11-17 unter Last 22 (acht
/// Shards des 35B auf einer Maschine, Runden liefen): Unterschreiben und
/// Abgeben in 0,4 bis 4,2 s, das Bündel 1 bis 3 Blöcke danach in der Kette.
pub const EINREICHEN_RAND_DREISSIGSTEL: u64 = 1;

/// Die zeitliche Untergrenze des Rands: 30 s, rund das Siebenfache des
/// Längsten, was Unterschreiben und Abgeben unter Last brauchten. Sie zählt
/// bei schnellem Blocktakt (60 ms im Podlauf: ein Dreißigstel wären 3,6 s).
pub const EINREICHEN_RAND_MS: u64 = 30_000;

/// **Ab welcher Stelle der Epoche das Bündel fällig ist**, bei `laenge`
/// Blöcken je Epoche und `ms_je_block` gemessener Blockzeit. Ohne Messung
/// das letzte Sechstel, wie bisher.
pub fn einreichen_ab(laenge: u64, ms_je_block: Option<u64>) -> u64 {
    let sechstel = laenge / 6;
    let rand = match ms_je_block {
        None => sechstel,
        Some(ms) => {
            let zeit = EINREICHEN_RAND_MS.div_ceil(ms.max(1));
            (laenge * EINREICHEN_RAND_DREISSIGSTEL / 30).max(zeit).min(sechstel)
        }
    };
    laenge.saturating_sub(rand.max(1))
}

/// **Die Blockzeit, wie dieser Pod sie sieht**: aus der ersten gesehenen
/// Höhe und der jetzigen, über mindestens [`Blocktakt::MINDESTENS`] Blöcke.
/// Fällt die Höhe (ein Neustart der Kette), beginnt die Messung neu.
#[derive(Debug, Default)]
pub struct Blocktakt {
    erste: Option<(std::time::Instant, u64)>,
    ms: Option<u64>,
}

impl Blocktakt {
    /// Über so viele Blöcke wird mindestens gemessen.
    pub const MINDESTENS: u64 = 20;

    /// Eine Höhe, gesehen zur Zeit `jetzt`.
    pub fn sehen(&mut self, hoehe: u64, jetzt: std::time::Instant) {
        match self.erste {
            Some((t, h)) if hoehe >= h => {
                let bloecke = hoehe - h;
                if bloecke >= Self::MINDESTENS {
                    let ms = jetzt.saturating_duration_since(t).as_millis() as u64;
                    self.ms = Some((ms / bloecke).max(1));
                }
            }
            _ => {
                self.erste = Some((jetzt, hoehe));
                self.ms = None;
            }
        }
    }

    /// Die gemessene Zeit je Block, sobald genug Blöcke gesehen sind.
    pub fn ms_je_block(&self) -> Option<u64> {
        self.ms
    }
}

/// **Wie lange auf einen neuen Pod gewartet wird**, bevor über ihn
/// gerechnet wird.
///
/// 📌 **Fund 649, zweiter Teil (2026-10-26, im langen Lauf über Docker):**
/// Gleich nach dem Wechsel liefen bei den neuen Mitgliedern teils noch die
/// Shards der alten Epoche; einer meldete „kein Shard“, ein anderer bekam
/// ein Token ohne Einbettung. Gerechnet wird erst, wenn jedes Mitglied den
/// Zuschnitt meldet, den die Kette ihm gibt. Weit unter der Frist der Tür.
pub const BEREIT_FRIST: Duration = Duration::from_secs(60);

/// Melden alle Mitglieder des Pods den Ebenenbereich aus der Kette?
fn pod_bereit(relais: SocketAddr, ausweis: &Ausweis, pod: &myl_types::relais::Podeintrag) -> bool {
    let weg = UeberDenKnoten::neu(relais, *ausweis, pod.mitglieder.iter().map(|m| m.netzadresse).collect());
    pod.mitglieder.iter().enumerate().all(|(j, m)| {
        matches!(
            weg.frage(j, &Shardanfrage::Zuschnitt),
            Ok(Shardantwort::Zuschnitt(z)) if z.layer_start == u64::from(m.von) && z.layer_end == u64::from(m.bis)
        )
    })
}

/// Der Koordinator einer Epoche und ob ihr Bündel schon eingereicht ist.
struct Podstand {
    epoche: u64,
    kennung: PodId,
    koordinator: Coordinator,
    /// ⚑ **Atomar**, denn eingereicht wird unter der Lesesperre, neben
    /// laufenden Sitzungen ([`einreichen_wenn_faellig`]).
    eingereicht: std::sync::atomic::AtomicBool,
    /// Die Netzadressen der rechnenden Mitglieder, Vertreter eingeschlossen:
    /// bei ihnen werden die Spuren abgelegt (Stufe 2).
    adressen: Vec<PeerIdBytes>,
}

/// Der Ausweis der Relaisleitung, aus seiner Datei, einmal gelesen.
struct Ausweisquelle {
    datei: PathBuf,
    gelesen: Mutex<Option<Ausweis>>,
}

impl Ausweisquelle {
    fn holen(&self) -> Result<Ausweis, String> {
        let mut g = self.gelesen.lock().map_err(|_| "Ausweis vergiftet")?;
        if let Some(a) = *g {
            return Ok(a);
        }
        let a = myl_types::relais::ausweis_lesen(&self.datei).map_err(|e| format!("Ausweis der Relaisleitung: {e}"))?;
        *g = Some(a);
        Ok(a)
    }
}

/// Rechnet über den Pod der Kette.
pub struct Podwerk {
    relais: SocketAddr,
    /// Der Ausweis der Relaisleitung: **gelesen, wenn er gebraucht wird**.
    /// Der Knoten schreibt ihn bei seinem Start, und der kommt nach diesem
    /// Prozess, denn er braucht dessen Ortsausweis.
    ausweis: Arc<Ausweisquelle>,
    wortschatz: Tokenizer,
    /// Die Endmarken aus dem Wortschatz (Fund 702), siehe [`haltemarken`].
    haltemarken: Vec<u32>,
    pipeline: Hash,
    deckel: u32,
    /// ⚑ **Ein Koordinator je Pod** (2026-10-28): Jedes Zwillingspaar
    /// rechnet seine Sitzungen, und jeder Pod reicht sein Bündel ein. Nach
    /// Pod-Index; ein Platz wird mit der Epoche neu belegt.
    staende: Arc<Mutex<std::collections::BTreeMap<u32, Platz>>>,
}

/// ⚑ **Lesend rechnen, schreibend wechseln** (2026-11-15): Sitzungen
/// rechnen unter der Lesesperre nebeneinander ([`Coordinator::rechne`]
/// nimmt `&self`); nur das Neubesetzen nimmt die Schreibsperre. Eingereicht
/// wird lesend (Fund 713): Bündel und Spurablage sehen denselben Stand,
/// weil beide aus den ersten `segmente` Segmenten entstehen
/// ([`Coordinator::kanonisch_bis`]). 📌 Bis dahin ein `Mutex` über die
/// ganze Anfrage: Der Pod rechnete eine nach der anderen.
type Platz = Arc<RwLock<Option<Podstand>>>;

/// Was ein Pod in einer Epoche eingereicht hat, mit den Blättern seines
/// Bündels: daraus entsteht nach dem Wechsel ein Differenzbeweis, wenn sein
/// Zwilling anders rechnete.
struct Eingereicht {
    kennung: PodId,
    zwilling: Option<u32>,
    wurzel: myl_types::ids::MerkleRoot,
    ausgaben: Vec<myl_types::Ausgabenzeugnis>,
    segmente: Vec<myl_types::Segmentzeugnis>,
}

/// Je (Epoche, Pod-Index) das Eingereichte.
type Blaetterbuch = Mutex<std::collections::BTreeMap<(u64, u32), Eingereicht>>;

/// Höchstens so viele Differenzen meldet der Ortsdienst je Paar, wie die
/// Kette annimmt (`DIFFERENZEN_JE_GRUPPE` im Ledger): eine genügt, um den
/// Falschen zu finden.
const DIFFERENZEN_JE_PAAR: usize = 4;

/// ⚑ **Welches Zwillingspaar eine Sitzung rechnet** (Whitepaper 3.4: zwei
/// zugeloste Pods je Segment; Ziel des Projektinhabers vom 2026-10-28: so
/// viele Paare wie möglich, damit viele Anfragen ohne Stau gerechnet
/// werden). Zugelost aus der Sitzung und den Kennungen der Pods dieser
/// Epoche (die die Epoche tragen): Eine Sitzung bleibt so die ganze Epoche
/// bei ihrem Paar, und die Sitzungen verteilen sich gleichmäßig über alle
/// Paare. Geliefert werden der antwortende Pod (der kleinere Index) und
/// sein Zwilling. Ohne jedes Paar (ein Netz mit einem Pod, Anlaufregel) der
/// erste Pod allein; ein Pod in Bereitschaft wird nie gewählt, solange es
/// ein Paar gibt.
pub fn paar_waehlen(bild: &Podbild, sitzung: u64) -> Option<(&myl_types::relais::Podeintrag, Option<&myl_types::relais::Podeintrag>)> {
    let finden = |i: u32| bild.pods.iter().find(|p| p.pod_index == i);
    let mut paare: Vec<(&myl_types::relais::Podeintrag, &myl_types::relais::Podeintrag)> = bild
        .pods
        .iter()
        .filter_map(|p| p.zwilling.filter(|&z| z > p.pod_index).and_then(finden).map(|z| (p, z)))
        .collect();
    if paare.is_empty() {
        return bild.pods.first().map(|p| (p, None));
    }
    paare.sort_by_key(|(p, _)| p.pod_index);
    let mut h = <sha2::Sha256 as sha2::Digest>::new();
    sha2::Digest::update(&mut h, b"myelith-paarwahl-v1");
    for p in &bild.pods {
        sha2::Digest::update(&mut h, p.kennung.as_bytes());
    }
    sha2::Digest::update(&mut h, sitzung.to_le_bytes());
    let d = sha2::Digest::finalize(h);
    let zahl = u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]);
    let (p, z) = paare[(zahl % paare.len() as u64) as usize];
    Some((p, Some(z)))
}

impl Podwerk {
    /// Lädt den Wortschatz und startet den Faden, der das Bündel einreicht.
    pub fn neu(artefakte: &Path, relais: SocketAddr, ausweisdatei: PathBuf, pipeline: Hash, deckel: u32) -> Result<Self, String> {
        let ausweis = Arc::new(Ausweisquelle { datei: ausweisdatei, gelesen: Mutex::new(None) });
        let wortschatz = Tokenizer::from_file(
            artefakte.join("tokenizer.json").to_str().ok_or("Artefaktpfad ist kein gültiges UTF-8")?,
        )
        .map_err(|e| format!("Wortschatz laden: {e}"))?;
        let staende: Arc<Mutex<std::collections::BTreeMap<u32, Platz>>> = Arc::default();
        let s = Arc::clone(&staende);
        let a = Arc::clone(&ausweis);
        let buch = Blaetterbuch::default();
        std::thread::Builder::new()
            .name("buendel".into())
            .spawn(move || {
                let mut takt = Blocktakt::default();
                loop {
                    std::thread::sleep(Duration::from_secs(3));
                    // Ohne Ausweis (der Knoten startet noch) gibt es nichts zu tun.
                    let Ok(ausweis) = a.holen() else { continue };
                    let plaetze: Vec<Platz> = s.lock().map(|g| g.values().cloned().collect()).unwrap_or_default();
                    for stand in &plaetze {
                        if let Err(f) = einreichen_wenn_faellig(relais, &ausweis, stand, &buch, &mut takt) {
                            eprintln!("[myl-pod] Bündel: {f}");
                        }
                    }
                    differenzen_melden(relais, &ausweis, &buch);
                }
            })
            .map_err(|e| e.to_string())?;
        let haltemarken = haltemarken(&wortschatz);
        Ok(Self { relais, ausweis, wortschatz, haltemarken, pipeline, deckel, staende })
    }
}

/// **Die Token, bei denen eine Antwort zu Ende ist** (Fund 702).
///
/// ⚑ Aus dem Wortschatz und nicht aus einer Tabelle, nach derselben Regel
/// wie der Klient für ein Modell auf seiner Maschine: `<|im_end|>` beendet
/// eine Runde, `<|endoftext|>` den Text. 📌 Nur eine Marke, die zu
/// **einem** Token wird, zählt; ein Wortschatz ohne sie zerlegte sie in
/// gewöhnliche Stücke, und ein Halt auf `<` beendete jede Antwort mit einer
/// spitzen Klammer.
pub fn haltemarken(wortschatz: &Tokenizer) -> Vec<u32> {
    ["<|im_end|>", "<|endoftext|>"]
        .iter()
        .filter_map(|m| {
            let t = wortschatz.encode(m);
            (t.len() == 1).then(|| t[0] as u32)
        })
        .collect()
}

fn podbild(relais: SocketAddr, ausweis: &Ausweis) -> Result<Podbild, String> {
    match myl_types::relais::fragen(relais, ausweis, &Relaisfrage::Podbild, Duration::from_secs(10))? {
        Relaisantwort::Podbild(b) => Ok(b),
        Relaisantwort::Fehler(f) => Err(f),
        andere => Err(format!("statt Podbild: {andere:?}")),
    }
}

/// ⚑ **Legt die Spuren eines Bündels bei jedem Mitglied ab** (Stufe 2,
/// 2026-10-28): je Segment Zeugnis und Kette (`[Eingang] ++ Spur`), in
/// Bündelfolge. Was über die Grenze einer Nachricht ginge, wird nicht
/// geschickt, sondern gemeldet; ein Mitglied, das nicht erreicht wird, auch.
///
/// 📌 **Fund 680 (2026-10-29): An alle Mitglieder zugleich, nach dem
/// Einreichen.** Vorher ging es Mitglied für Mitglied, bis zu 10 s je
/// Mitglied, und vor dem Einreichen; das letzte Sechstel einer Epoche (im
/// Kurzlauf 18 s) reichte dann nicht, das Bündel kam nach dem Wechsel und
/// wurde abgewiesen, und die Zwillingsgruppe blieb unvollständig (niemand
/// vergütet). Gezogen wird erst mit dem Zertifikat nach dem Wechsel, die
/// Spuren liegen also auch danach noch rechtzeitig.
///
/// ⚑ Derselbe Stand wie das Bündel: die ersten `buendel.segmente` Segmente
/// ([`Coordinator::kanonisch_bis`]). Was seither angehängt hat, gehört in
/// kein Bündel dieser Epoche und nicht in die Ablage.
fn spuren_ablegen(relais: SocketAddr, ausweis: &Ausweis, s: &Podstand, buendel: &myl_types::PoIBundle) {
    let folge = s.koordinator.kanonisch_bis(buendel.segmente as usize);
    let ablage = myl_types::Spurablage {
        epoche: buendel.epoch,
        pod: buendel.pod,
        segments_root: buendel.segments_root,
        // ⚑ In der kanonischen Folge, derselben wie die Blätter des Bündels:
        //   Eine Stelle der Ablage ist eine Stelle unter `segments_root`.
        eintraege: folge
            .iter()
            .map(|c| {
                let zeugnis = myl_types::Segmentzeugnis { id: c.id, spurwurzel: c.spurwurzel };
                (zeugnis, c.kette().into_iter().map(Hash).collect())
            })
            .collect(),
        // Die Tokens dazu, in derselben Folge: Damit rechnet ein Checker ab
        // den Tokens nach (Stufe 2).
        eingaben: folge.iter().map(|c| c.eingabe.clone()).collect(),
        // ⛔️ Das unterschriebene Bündel dazu: Die Mitglieder nehmen eine
        //   Ablage aus dem Netz nur mit ihm (T6, 2026-11-05).
        buendel: Some(Box::new(buendel.clone())),
    };
    let Ok(bytes) = borsh::to_vec(&ablage) else {
        eprintln!("[myl-pod] Spuren der Epoche {}: nicht serialisierbar", s.epoche);
        return;
    };
    // ⚑ **Grösser als eine Nachricht: in Stücken** (T6, 2026-11-05). 📌 Bis
    //   hierher wurde eine solche Ablage gar nicht abgelegt, und bei einigen
    //   Tausend Segmenten je Epoche lieferte niemand den Verlauf.
    if bytes.len() + 1_024 > myl_types::relais::MAX_RELAIS_BYTES || stueckgroesse_im_pruefstand().is_some() {
        stueckweise_ablegen(relais, ausweis, s, &bytes);
        return;
    }
    std::thread::scope(|faeden| {
        for an in &s.adressen {
            let frage = Relaisfrage::SpurenAblegen { an: *an, ablage: ablage.clone() };
            faeden.spawn(move || match myl_types::relais::fragen(relais, ausweis, &frage, Duration::from_secs(10)) {
                Ok(Relaisantwort::Erledigt(_)) => {}
                andere => eprintln!("[myl-pod] Spuren der Epoche {} nicht abgelegt: {andere:?}", s.epoche),
            });
        }
    });
}

/// ⚑ **Nur im Prüfstand** (Merkmal `faelscher`, `MYL_ABLAGE_STUECK=<bytes>`):
/// Mit einem kleinen Modell wird eine Ablage nie grösser als eine Nachricht,
/// und der Weg in Stücken liefe im Netz nie. Im ausgelieferten Programm immer
/// `None`.
fn stueckgroesse_im_pruefstand() -> Option<usize> {
    #[cfg(feature = "faelscher")]
    {
        std::env::var("MYL_ABLAGE_STUECK").ok().and_then(|t| t.parse().ok()).filter(|n| *n > 0)
    }
    #[cfg(not(feature = "faelscher"))]
    {
        None
    }
}

/// Legt eine Ablage in Stücken bei jedem Mitglied ab, je Mitglied Stück für
/// Stück in Folge (das Mitglied setzt lückenlos zusammen); ein Fehler bricht
/// bei diesem Mitglied ab, die anderen bekommen ihre trotzdem.
fn stueckweise_ablegen(relais: SocketAddr, ausweis: &Ausweis, s: &Podstand, bytes: &[u8]) {
    let gesamt = bytes.len() as u32;
    let abdruck = Hash::sha256(bytes);
    std::thread::scope(|faeden| {
        for an in &s.adressen {
            faeden.spawn(move || {
                let stueck = stueckgroesse_im_pruefstand().unwrap_or(myl_types::relais::ABLAGE_STUECK_BYTES);
                for (i, daten) in bytes.chunks(stueck).enumerate() {
                    let ab = (i * stueck) as u32;
                    let frage = Relaisfrage::SpurenAblegenTeil { an: *an, gesamt, abdruck, ab, daten: daten.to_vec() };
                    match myl_types::relais::fragen(relais, ausweis, &frage, Duration::from_secs(30)) {
                        Ok(Relaisantwort::Erledigt(_)) => {}
                        andere => {
                            eprintln!("[myl-pod] Spuren der Epoche {} ab Byte {ab} nicht abgelegt: {andere:?}", s.epoche);
                            return;
                        }
                    }
                }
            });
        }
    });
}

/// Reicht das Bündel der laufenden Epoche ein, wenn es fällig ist.
fn einreichen_wenn_faellig(
    relais: SocketAddr,
    ausweis: &Ausweis,
    stand: &RwLock<Option<Podstand>>,
    buch: &Blaetterbuch,
    takt: &mut Blocktakt,
) -> Result<(), String> {
    let bild = podbild(relais, ausweis)?;
    takt.sehen(bild.hoehe, std::time::Instant::now());
    let laenge = bild.bloecke_je_epoche.max(1);
    let ab = einreichen_ab(laenge, takt.ms_je_block());
    let faellig = |s: &Podstand| {
        !s.eingereicht.load(std::sync::atomic::Ordering::SeqCst)
            && s.epoche == bild.epoche
            && bild.hoehe % laenge >= ab
            && s.koordinator.segmentzahl() > 0
    };
    // ⚑ **Unter der Lesesperre, neben laufenden Sitzungen** (Fund 713).
    //   📌 Hier stand die Schreibsperre, und sie wartete auf jede laufende
    //   Sitzung. Im gezielten Auftragslauf vom 2026-11-17 liefen zum Fünftel
    //   der Epoche Runden des 35B von 5 bis 15 Minuten; das Einreichen kam
    //   erst nach dem Wechsel dran, die Shards der Epoche waren beendet, kein
    //   Mitglied unterschrieb, und beide Pods bekamen für die ganze Epoche
    //   nichts. Der Koordinator nimmt einen Schnappschuss (ein Lauf kommt
    //   ganz oder gar nicht hinein), und ein Shard prüft nur, ob Anspruch und
    //   Segmentzahl des Bündels zueinander passen; die Sperre schützte nichts.
    let g = stand.read().map_err(|_| "Stand vergiftet")?;
    let Some(s) = g.as_ref() else { return Ok(()) };
    if !faellig(s) {
        return Ok(());
    }
    // ⚑ Atomar getauscht: Von zwei Fäden reicht genau einer ein.
    if s.eingereicht.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Ok(());
    }
    // ⚑ **Gemessen für den Rand** (2026-11-17): wie lange Unterschreiben und
    //   Abgeben dauern und wie viele Blöcke der Epoche danach bleiben; der
    //   Rand ([`einreichen_ab`]) steht auf dieser Messung, die Zeile prüft
    //   ihn in jedem Lauf.
    let anfang = std::time::Instant::now();
    let rest_bei_beginn = laenge - bild.hoehe % laenge;
    // Scheitert schon das Bündel (ein Mitglied unterschreibt gerade nicht),
    // versucht es der nächste Blick wieder, wie bisher. Ist es abgegeben,
    // ist es eingereicht, auch wenn die Kette ablehnt: Ein zweiter Versuch
    // derselben Epoche fände dieselben Gründe.
    let gebaut = s.koordinator.build_signed_poi_bundle().and_then(|b| Ok((borsh::to_vec(&b).map_err(|e| e.to_string())?, b)));
    let (bytes, buendel) = match gebaut {
        Ok(x) => x,
        Err(f) => {
            s.eingereicht.store(false, std::sync::atomic::Ordering::SeqCst);
            return Err(f);
        }
    };
    let antwort = myl_types::relais::fragen(relais, ausweis, &Relaisfrage::BuendelEinreichen { buendel: bytes }, Duration::from_secs(30));
    // ⚑ Stufe 2: Danach legt jedes Mitglied die Spuren ab, damit es auf die
    //   Stichprobe antworten kann, wenn sein Shard längst endete (Fund 680:
    //   nach dem Einreichen, nicht davor).
    spuren_ablegen(relais, ausweis, s, &buendel);
    match antwort? {
        Relaisantwort::Erledigt(z) => {
            eprintln!(
                "[myl-pod] Bündel der Epoche {}: {z} (unterschrieben und abgegeben in {} ms, {rest_bei_beginn} Blöcke vor dem Wechsel begonnen)",
                s.epoche,
                anfang.elapsed().as_millis()
            );
            // Die Blätter für einen Differenzbeweis nach dem Wechsel.
            if let Some(eintrag) = bild.pods.iter().find(|p| p.kennung == s.kennung) {
                let (ausgaben, segmente) = s.koordinator.blaetter_bis(buendel.segmente as usize);
                if let Ok(mut b) = buch.lock() {
                    b.insert(
                        (s.epoche, eintrag.pod_index),
                        Eingereicht { kennung: s.kennung, zwilling: eintrag.zwilling, wurzel: buendel.ausgabenwurzel, ausgaben, segmente },
                    );
                }
            }
            Ok(())
        }
        Relaisantwort::Fehler(f) => Err(format!("Epoche {}: {f}", s.epoche)),
        andere => Err(format!("unerwartet: {andere:?}")),
    }
}

/// ⚑ **Differenzen der vorigen Epochen melden** (Whitepaper 6.4 und 6.6,
/// Festlegung des Projektinhabers vom 2026-10-29): Der Ortsdienst rechnet
/// beide Zwillinge und kennt ihre Blätter. Ist die Epoche vorbei und haben
/// zwei Zwillinge verschiedene Ausgabenwurzeln, sucht er die Segmente mit
/// verschiedenem Blatt und schickt für jedes, höchstens
/// [`DIFFERENZEN_JE_PAAR`], einen Differenzbeweis an den Knoten; der reicht
/// ihn an die Koordinatoren beider Pods. Danach vergisst er die Epoche.
fn differenzen_melden(relais: SocketAddr, ausweis: &Ausweis, buch: &Blaetterbuch) {
    let Ok(bild) = podbild(relais, ausweis) else { return };
    let Ok(mut b) = buch.lock() else { return };
    let vorbei: Vec<(u64, u32)> = b.keys().copied().filter(|(e, _)| *e < bild.epoche).collect();
    for &(epoche, i) in &vorbei {
        let Some(a) = b.get(&(epoche, i)) else { continue };
        let Some(j) = a.zwilling.filter(|j| *j > i) else { continue };
        let Some(z) = b.get(&(epoche, j)) else { continue };
        if a.wurzel == z.wurzel {
            continue;
        }
        let andere: std::collections::BTreeMap<SegmentId, &myl_types::Ausgabenzeugnis> = z.ausgaben.iter().map(|x| (x.id, x)).collect();
        let mut verschieden: Vec<SegmentId> =
            a.ausgaben.iter().filter(|x| andere.get(&x.id).is_some_and(|y| *y != *x)).map(|x| x.id).collect();
        verschieden.sort_by(|x, y| x.as_ref().cmp(y.as_ref()));
        for id in verschieden.into_iter().take(DIFFERENZEN_JE_PAAR) {
            let (Some(sa), Some(sb)) = (
                myl_types::differenzseite(&a.ausgaben, &a.segmente, &id),
                myl_types::differenzseite(&z.ausgaben, &z.segmente, &id),
            ) else {
                continue;
            };
            let frage = Relaisfrage::DifferenzMelden {
                epoche,
                pod_a: a.kennung,
                pod_b: z.kennung,
                beweis: Box::new(myl_types::Differenzbeweis { a: sa, b: sb }),
            };
            match myl_types::relais::fragen(relais, ausweis, &frage, Duration::from_secs(10)) {
                Ok(Relaisantwort::Erledigt(w)) => eprintln!("[myl-pod] Differenz der Epoche {epoche}, Pods {i} und {j}: {w}"),
                andere => eprintln!("[myl-pod] Differenz der Epoche {epoche}, Pods {i} und {j} nicht gemeldet: {andere:?}"),
            }
        }
    }
    for k in vorbei {
        b.remove(&k);
    }
}

/// Meldet das Mitglied an Stelle `j` des Wegs `adressen` den Bereich `von..bis`?
fn mitglied_bereit(relais: SocketAddr, ausweis: &Ausweis, adressen: &[PeerIdBytes], j: usize, von: u32, bis: u32) -> bool {
    let weg = UeberDenKnoten::neu(relais, *ausweis, adressen.to_vec());
    matches!(
        weg.frage(j, &Shardanfrage::Zuschnitt),
        Ok(Shardantwort::Zuschnitt(z)) if z.layer_start == u64::from(von) && z.layer_end == u64::from(bis)
    )
}

/// ⚑ **Die Besetzung des Pods, mit Ersatz aus der Reserve** (Whitepaper 6.8,
/// Festlegung des Projektinhabers vom 2026-10-28: der Weg ersetzt
/// sofort). Meldet ein Mitglied seinen Bereich nicht, bittet das Podwerk über
/// die Relaisleitung das nächste Reservemitglied, die Position zu übernehmen;
/// dessen Knoten prüft das gegen die Kette und startet den Shard. Meldet die
/// Reserve den Bereich, rechnet sie an dieser Stelle, und das Bündel nennt
/// sie. Ein neuer Aufbau des Caches braucht es nicht: Jeder Auftrag geht hier
/// mit vollem Prefill durch den Pod. Liefert die Netzadressen in
/// Shardfolge und die Vertreter.
fn besetzen(
    relais: SocketAddr,
    ausweis: &Ausweis,
    epoche: u64,
    pod: &myl_types::relais::Podeintrag,
) -> (Vec<PeerIdBytes>, Vec<(u32, myl_types::ids::MinerId)>) {
    let mut adressen: Vec<PeerIdBytes> = pod.mitglieder.iter().map(|m| m.netzadresse).collect();
    let mut vertreter = Vec::new();
    let mut frei = pod.reserve.iter();
    for (j, m) in pod.mitglieder.iter().enumerate() {
        if mitglied_bereit(relais, ausweis, &adressen, j, m.von, m.bis) {
            continue;
        }
        for r in frei.by_ref() {
            let frage = Relaisfrage::Uebernahme { an: r.netzadresse, epoche, pod: pod.pod_index, position: j as u32 };
            match myl_types::relais::fragen(relais, ausweis, &frage, Duration::from_secs(30)) {
                Ok(Relaisantwort::Erledigt(_)) => {}
                andere => {
                    eprintln!("[myl-pod] Pod {}, Position {j}: Reserve lehnt ab: {andere:?}", pod.pod_index);
                    continue;
                }
            }
            adressen[j] = r.netzadresse;
            let bis = std::time::Instant::now() + BEREIT_FRIST;
            while !mitglied_bereit(relais, ausweis, &adressen, j, m.von, m.bis) && std::time::Instant::now() < bis {
                std::thread::sleep(Duration::from_secs(2));
            }
            if mitglied_bereit(relais, ausweis, &adressen, j, m.von, m.bis) {
                eprintln!("[myl-pod] Pod {}, Position {j}: von der Reserve übernommen", pod.pod_index);
                vertreter.push((j as u32, r.miner));
                break;
            }
            adressen[j] = m.netzadresse;
        }
    }
    (adressen, vertreter)
}

/// Was ein Pod für einen Auftrag rechnet: Sitzung, Token, höchstens so
/// viele neue.
#[derive(Clone, Copy)]
struct Arbeit<'a> {
    sitzung: u64,
    eingabe: &'a [u32],
    deckel: u64,
    /// Die Endmarken des Modells (Fund 702).
    halt: &'a [u32],
}

/// Besetzt `pod` für `epoche` und baut seinen Koordinator.
fn podstand_bauen(
    relais: SocketAddr,
    ausweis: &Ausweis,
    epoche: u64,
    pod: &myl_types::relais::Podeintrag,
    halt: &[u32],
) -> Result<Podstand, String> {
    // Erst wenn der Pod steht (Fund 649, zweiter Teil).
    let bis = std::time::Instant::now() + BEREIT_FRIST;
    while !pod_bereit(relais, ausweis, pod) && std::time::Instant::now() < bis {
        std::thread::sleep(Duration::from_secs(2));
    }
    // ⚑ Wer dann noch fehlt, wird aus der Reserve ersetzt (6.8).
    let (mitglieder, vertreter) = besetzen(relais, ausweis, epoche, pod);
    let weg = Arc::new(UeberDenKnoten::neu(relais, *ausweis, mitglieder.clone()));
    let mut k = Coordinator::ueber_weg(pod.kennung, EpochId(epoche), weg)?;
    k.vertreter_setzen(vertreter);
    k.haltemarken_setzen(halt.to_vec());
    Ok(Podstand {
        epoche,
        kennung: pod.kennung,
        koordinator: k,
        eingereicht: std::sync::atomic::AtomicBool::new(false),
        adressen: mitglieder,
    })
}

/// ⚑ **Neu besetzen und erben** (2026-11-17, Restpunkt 9 zu Fund 706): Der
/// neue Koordinator übernimmt die abgeschlossenen Segmente des alten
/// derselben Epoche und dessen Vermerk, ob schon eingereicht ist (sonst
/// reichte er ein zweites Bündel derselben Epoche ein, das die Kette
/// abweist). Unter einer Schreibsperre: Laufende Sitzungen des alten hängen
/// vorher noch an, und zwei Fäden, die zugleich neu besetzen, erben
/// nacheinander. Scheitert der Bau, bleibt der alte stehen, mit seinen
/// Segmenten. 📌 Bis hierher wurde der Platz geleert, und alles, was der
/// Pod in dieser Epoche schon gerechnet hatte, ging unvergütet verloren.
fn neu_besetzen(
    relais: SocketAddr,
    ausweis: &Ausweis,
    platz: &Platz,
    epoche: u64,
    pod: &myl_types::relais::Podeintrag,
    halt: &[u32],
) -> Result<(), String> {
    let mut g = platz.write().map_err(|_| "Stand vergiftet")?;
    let mut neu = podstand_bauen(relais, ausweis, epoche, pod, halt)?;
    if let Some(alt) = g.as_ref().filter(|a| a.epoche == epoche && a.kennung == pod.kennung) {
        erben(&mut neu, alt, pod.pod_index);
    }
    *g = Some(neu);
    Ok(())
}

/// Segmente und Einreichvermerk von `alt` nach `neu` ([`neu_besetzen`]).
fn erben(neu: &mut Podstand, alt: &Podstand, pod_index: u32) {
    match neu.koordinator.segmente_erben(&alt.koordinator) {
        Ok(zahl) => eprintln!("[myl-pod] Epoche {}, Pod {pod_index}: neu besetzt, {zahl} Segmente übernommen", neu.epoche),
        Err(f) => eprintln!("[myl-pod] Epoche {}, Pod {pod_index}: neu besetzt, Segmente nicht übernommen: {f}", neu.epoche),
    }
    let eingereicht = alt.eingereicht.load(std::sync::atomic::Ordering::SeqCst);
    neu.eingereicht.store(eingereicht, std::sync::atomic::Ordering::SeqCst);
}

/// Rechnet einen Auftrag einmal über `pod`: Token, letztes Segment, Epoche.
/// Der Koordinator in `stand` entsteht je Epoche und Pod neu.
fn ueber_pod(
    relais: SocketAddr,
    ausweis: &Ausweis,
    stand: &RwLock<Option<Podstand>>,
    epoche: u64,
    pod: &myl_types::relais::Podeintrag,
    arbeit: Arbeit<'_>,
) -> Result<(Vec<u32>, SegmentId, u64), String> {
    let Arbeit { sitzung, eingabe, deckel, halt } = arbeit;
    let passt = |g: &Option<Podstand>| g.as_ref().map(|s| (s.epoche, s.kennung)) == Some((epoche, pod.kennung));
    // Lesend, wenn der Pod schon steht; sonst schreibend besetzen und dann
    // lesend rechnen (eine Schreibsperre lässt sich nicht herabstufen).
    let g = loop {
        let g = stand.read().map_err(|_| "Stand vergiftet")?;
        if passt(&g) {
            break g;
        }
        drop(g);
        let mut g = stand.write().map_err(|_| "Stand vergiftet")?;
        if passt(&g) {
            continue;
        }
        *g = Some(podstand_bauen(relais, ausweis, epoche, pod, halt)?);
    };
    let s = g.as_ref().ok_or("kein Koordinator")?;
    let beginn = std::time::Instant::now();
    let lauf = s.koordinator.rechne(sitzung, eingabe, deckel);
    let token = lauf.token;
    eprintln!(
        "[myl-pod] Epoche {}, Pod {}: {} Prompt-Token, {} neue in {} ms",
        s.epoche,
        pod.pod_index,
        eingabe.len(),
        token.len(),
        beginn.elapsed().as_millis()
    );
    let segment = lauf.letztes_segment.unwrap_or(SegmentId::new([0u8; 32]));
    s.koordinator.sitzung_abschliessen(sitzung);
    Ok((token, segment, s.epoche))
}

impl Podwerk {
    /// Der Platz des Koordinators für `pod`.
    fn platz(&self, pod: u32) -> Result<Platz, String> {
        let mut g = self.staende.lock().map_err(|_| "Stände vergiftet")?;
        Ok(Arc::clone(g.entry(pod).or_default()))
    }

    /// Rechnet einen Auftrag über das Paar, das die Sitzung zugelost bekommt
    /// ([`paar_waehlen`]).
    ///
    /// ⚑ **Beide Zwillinge zugleich, die erste Antwort geht hinaus**
    /// (Whitepaper 6.4, Stufe 1 je Sitzung). Je Pod ein Faden mit dem
    /// Koordinator dieses Pods; wer zuerst Token liefert, antwortet, der
    /// andere rechnet zu Ende und hängt an. 📌 Bis dahin begann der Zwilling
    /// erst nach der Antwort des ersten Pods: Er lag um einen Auftrag zurück,
    /// und was beim Einreichen noch lief, fehlte in seinem Bündel.
    fn einmal(
        &self,
        bild: &Podbild,
        ausweis: &Ausweis,
        sitzung: u64,
        eingabe: &[u32],
        deckel: u64,
    ) -> Result<(Vec<u32>, SegmentId, u64), String> {
        let (pod, zwilling) = paar_waehlen(bild, sitzung).ok_or("kein Pod in dieser Epoche")?;
        let Some(zwilling) = zwilling else {
            let arbeit = Arbeit { sitzung, eingabe, deckel, halt: &self.haltemarken };
            return mit_neubesetzung(self.relais, ausweis, &self.platz(pod.pod_index)?, bild.epoche, pod, arbeit);
        };
        let (senden, empfangen) = std::sync::mpsc::channel();
        for p in [pod, zwilling] {
            let (relais, ausweis, platz, epoche, p) = (self.relais, *ausweis, self.platz(p.pod_index)?, bild.epoche, p.clone());
            let (eingabe, halt, senden) = (eingabe.to_vec(), self.haltemarken.clone(), senden.clone());
            let faden = std::thread::Builder::new().name(format!("pod-{}", p.pod_index)).spawn(move || {
                let arbeit = Arbeit { sitzung, eingabe: &eingabe, deckel, halt: &halt };
                let ergebnis = mit_neubesetzung(relais, &ausweis, &platz, epoche, &p, arbeit);
                if let Err(f) = &ergebnis {
                    eprintln!("[myl-pod] Pod {}: {f}", p.pod_index);
                }
                // Niemand hört mehr zu, wenn der andere schon geantwortet hat.
                let _ = senden.send(ergebnis);
            });
            if let Err(f) = faden {
                eprintln!("[myl-pod] kein Faden für einen Pod: {f}");
            }
        }
        drop(senden);
        erste_antwort(empfangen)
    }
}

/// Die erste Antwort mit Token; kommt keine, die letzte ohne Fehler, sonst
/// der letzte Fehler. Endet, sobald beide Seiten geantwortet haben.
fn erste_antwort<T>(antworten: impl IntoIterator<Item = Result<(Vec<u32>, T, u64), String>>) -> Result<(Vec<u32>, T, u64), String> {
    let mut bisher = Err("kein Pod antwortete".to_string());
    for antwort in antworten {
        if antwort.as_ref().is_ok_and(|a| !a.0.is_empty()) {
            return antwort;
        }
        if bisher.is_err() || antwort.is_ok() {
            bisher = antwort;
        }
    }
    bisher
}

/// Rechnet einen Auftrag über `pod` und, kam mitten in der Epoche kein
/// Token, nach neuer Besetzung noch einmal.
///
/// ⚑ Der Pod wird einmal neu besetzt (ein Mitglied kann ausgefallen sein,
/// 6.8). 📌 Nur in derselben Epoche: Ist sie inzwischen vorbei, rechnet der
/// Pod der neuen (Fund 649); der erste Einbau wartete hier auf den alten Pod
/// und verlor die Antwort. Gilt für beide Zwillinge gleich. Der neue
/// Koordinator erbt, was der alte schon abgeschlossen hat ([`neu_besetzen`]).
fn mit_neubesetzung(
    relais: SocketAddr,
    ausweis: &Ausweis,
    platz: &Platz,
    epoche: u64,
    pod: &myl_types::relais::Podeintrag,
    arbeit: Arbeit<'_>,
) -> Result<(Vec<u32>, SegmentId, u64), String> {
    let ergebnis = ueber_pod(relais, ausweis, platz, epoche, pod, arbeit)?;
    if !ergebnis.0.is_empty() || !podbild(relais, ausweis).is_ok_and(|neu| neu.epoche == epoche) {
        return Ok(ergebnis);
    }
    if let Err(f) = neu_besetzen(relais, ausweis, platz, epoche, pod, arbeit.halt) {
        eprintln!("[myl-pod] Epoche {epoche}, Pod {}: Neubesetzung gescheitert: {f}", pod.pod_index);
    }
    ueber_pod(relais, ausweis, platz, epoche, pod, arbeit)
}

impl Klartextwerk for Podwerk {
    fn rechne(&self, auftrag: &Inferenzauftrag, prompt: &[u8]) -> Inferenzantwort {
        let sitzung = auftrag.sitzung;
        let abgelehnt = |grund: &str| {
            eprintln!("[myl-pod] abgelehnt: {grund}");
            Inferenzantwort::Abgelehnt { sitzung }
        };
        let Ok(text) = core::str::from_utf8(prompt) else {
            return abgelehnt("kein UTF-8");
        };
        let eingabe: Vec<u32> = self.wortschatz.encode(text).iter().map(|t| *t as u32).collect();
        if eingabe.is_empty() {
            return abgelehnt("leerer Prompt");
        }
        // ⚑ **Der Hoechstbetrag, jetzt wo die Token gezaehlt sind** (Fund
        //   711): Kostet schon der Prompt mehr, wird nicht gerechnet, und die
        //   Tuer erfaehrt es als eigene Antwort; sonst wird die Ausgabe so
        //   gekuerzt, dass die Abrechnung den Betrag nicht uebersteigt. Vor
        //   dem Ausweis und dem Pod, denn ein ungedeckter Auftrag soll nichts
        //   kosten.
        let prompt_token = eingabe.len() as u64;
        let Some(im_rahmen) = auftrag.neue_token_im_rahmen(prompt_token) else {
            eprintln!("[myl-pod] nicht gedeckt: {prompt_token} Prompt-Token, hoechstens {} Credits", auftrag.hoechstens);
            return Inferenzantwort::Ungedeckt { sitzung, prompt_token };
        };
        let ausweis = match self.ausweis.holen() {
            Ok(a) => a,
            Err(f) => return abgelehnt(&f),
        };
        let deckel = u64::from(im_rahmen.min(self.deckel));
        let bild = match podbild(self.relais, &ausweis) {
            Ok(b) => b,
            Err(f) => return abgelehnt(&f),
        };
        let (mut token, mut segment, epoche) = match self.einmal(&bild, &ausweis, sitzung, &eingabe, deckel) {
            Ok(t) => t,
            Err(f) => return abgelehnt(&f),
        };
        // 📌 **Fund 649: ein Auftrag über die Epochengrenze.** Am Wechsel
        //    beenden die Knoten ihre Shards, auch mitten in einem Auftrag,
        //    und auch die Wiederholung des Koordinators trifft dann nur noch
        //    die alten Plätze. Kam kein Token und steht die Kette inzwischen
        //    in einer neuen Epoche, rechnet der Pod dieser Epoche den Auftrag
        //    einmal neu.
        if token.is_empty() {
            if let Ok(neu) = podbild(self.relais, &ausweis) {
                if neu.epoche != epoche {
                    eprintln!("[myl-pod] Epoche {epoche} endete im Auftrag; der Pod der Epoche {} rechnet ihn neu", neu.epoche);
                    match self.einmal(&neu, &ausweis, sitzung, &eingabe, deckel) {
                        Ok((t, seg, _)) => {
                            token = t;
                            segment = seg;
                        }
                        Err(f) => return abgelehnt(&f),
                    }
                }
            }
        }
        if token.is_empty() {
            return abgelehnt("der Pod lieferte kein Token");
        }
        // ⚑ Der Text ohne die Endmarke (Fund 702): Sie sagt, dass die Antwort
        //   zu Ende ist, und gehört nicht zu ihr. Die Token behalten sie, denn
        //   sie ist gerechnet und wird abgerechnet wie jedes andere.
        let ende = match token.last() {
            Some(t) if self.haltemarken.contains(t) => token.len() - 1,
            _ => token.len(),
        };
        let ausgabe: Vec<usize> = token[..ende].iter().map(|t| *t as usize).collect();
        Inferenzantwort::Ergebnis { sitzung, text: self.wortschatz.decode(&ausgabe), token, segment, prompt_token }
    }

    fn pipeline(&self) -> Hash {
        self.pipeline
    }

    fn shards(&self) -> u32 {
        let plaetze: Vec<Platz> = self.staende.lock().map(|g| g.values().cloned().collect()).unwrap_or_default();
        plaetze
            .iter()
            .find_map(|p| p.read().ok().and_then(|g| g.as_ref().map(|s| s.koordinator.num_shards() as u32)))
            .unwrap_or(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Der Rand vor dem Wechsel** (2026-11-18): ohne Messung das
    /// Sechstel; mit Messung ein Dreißigstel, aber nie unter 30 s und nie über
    /// das Sechstel. Gegenproben: Bei 60 ms je Block wäre ein Dreißigstel 3,6 s,
    /// kürzer als das Gemessene; dort bleibt das Sechstel.
    #[test]
    fn der_rand_ist_ein_dreissigstel_und_nie_unter_dreissig_sekunden() {
        assert_eq!(einreichen_ab(1800, None), 1500, "ohne Messung das Sechstel");
        assert_eq!(einreichen_ab(1800, Some(600)), 1740, "600 ms: 60 Blöcke, 36 s");
        assert_eq!(einreichen_ab(1800, Some(2_000)), 1740, "Testnetz: 60 Blöcke, 2 min");
        assert_eq!(einreichen_ab(1800, Some(400)), 1725, "400 ms: 75 Blöcke für 30 s");
        assert_eq!(einreichen_ab(1800, Some(60)), 1500, "60 ms: höchstens das Sechstel");
        for ms in [1u64, 60, 100, 400, 500, 600, 1_000, 2_000, 10_000] {
            let rand = 1800 - einreichen_ab(1800, Some(ms));
            assert!((60..=300).contains(&rand), "{ms} ms: Rand {rand}");
            assert!(rand * ms >= EINREICHEN_RAND_MS || rand == 300, "{ms} ms: Rand {rand} unter 30 s");
        }
        assert_eq!(einreichen_ab(1, Some(600)), 0, "eine Epoche aus einem Block");
    }

    /// Der Blocktakt misst über mindestens zwanzig Blöcke und beginnt neu,
    /// wenn die Höhe fällt.
    #[test]
    fn der_blocktakt_misst_ueber_zwanzig_bloecke() {
        let t0 = std::time::Instant::now();
        let mut b = Blocktakt::default();
        b.sehen(100, t0);
        b.sehen(110, t0 + Duration::from_secs(6));
        assert_eq!(b.ms_je_block(), None, "zehn Blöcke sind zu wenig");
        b.sehen(130, t0 + Duration::from_secs(18));
        assert_eq!(b.ms_je_block(), Some(600));
        b.sehen(5, t0 + Duration::from_secs(20));
        assert_eq!(b.ms_je_block(), None, "nach einem Neustart neu");
    }
    use myl_types::relais::Podeintrag;

    fn bild(paare: &[(u32, Option<u32>)], epoche: u64) -> Podbild {
        Podbild {
            epoche,
            hoehe: epoche * 1800,
            bloecke_je_epoche: 1800,
            pods: paare
                .iter()
                .map(|&(i, z)| Podeintrag {
                    pod_index: i,
                    kennung: myl_types::pod_kennung(epoche, i),
                    mitglieder: Vec::new(),
                    zwilling: z,
                    reserve: Vec::new(),
                })
                .collect(),
        }
    }

    /// ⚑ **Die Sitzungen verteilen sich über alle Paare** (W7): gleichmäßig,
    /// je Sitzung fest in der Epoche, nie ein Pod in Bereitschaft.
    #[test]
    fn jede_sitzung_bekommt_ein_paar_und_alle_paare_rechnen() {
        // Fünf Pods: zwei Paare, Pod 4 in Bereitschaft.
        let b = bild(&[(0, Some(2)), (1, Some(3)), (2, Some(0)), (3, Some(1)), (4, None)], 7);
        let mut je_pod = std::collections::BTreeMap::<u32, u32>::new();
        for sitzung in 0..2_000u64 {
            let (p, z) = paar_waehlen(&b, sitzung).expect("ein Paar");
            assert_eq!(z.map(|z| z.zwilling), Some(Some(p.pod_index)), "der Zwilling gehört zum Paar");
            assert!(p.pod_index < z.map_or(0, |z| z.pod_index), "es antwortet der kleinere Index");
            *je_pod.entry(p.pod_index).or_default() += 1;
            // Dieselbe Sitzung, dasselbe Paar (KV-Cache).
            assert_eq!(paar_waehlen(&b, sitzung).map(|(p, _)| p.pod_index), Some(p.pod_index));
        }
        assert_eq!(je_pod.keys().copied().collect::<Vec<_>>(), vec![0, 1], "nie Bereitschaft, nie der Zwilling");
        assert!(je_pod.values().all(|&n| (800..=1_200).contains(&n)), "gleichmäßig: {je_pod:?}");
        // Eine andere Epoche lost neu.
        let c = bild(&[(0, Some(2)), (1, Some(3)), (2, Some(0)), (3, Some(1)), (4, None)], 8);
        let anders = (0..200u64).filter(|&s| paar_waehlen(&b, s).map(|x| x.0.pod_index) != paar_waehlen(&c, s).map(|x| x.0.pod_index)).count();
        assert!(anders > 50, "die Epoche lost neu: {anders} von 200");
        // ⛔️ Gegenproben: ein Pod allein (Anlaufregel) rechnet ohne
        //    Zwilling; ohne Pods nichts.
        let allein = bild(&[(0, None)], 7);
        assert_eq!(paar_waehlen(&allein, 5).map(|(p, z)| (p.pod_index, z.is_none())), Some((0, true)));
        assert!(paar_waehlen(&bild(&[], 7), 5).is_none());
    }

    /// ⚑ **Die erste Antwort mit Token geht hinaus**, gleich von welchem
    /// Zwilling; ein Fehler oder eine leere Antwort davor hält sie nicht
    /// auf. ⛔️ Gegenproben: Nach ihr wird nicht weiter gewartet; ohne Token
    /// zählt die leere Antwort vor dem Fehler, und ohne jede Antwort ist es
    /// ein Fehler.
    #[test]
    fn die_erste_antwort_mit_token_geht_hinaus() {
        let mit = |t: u32| Ok((vec![t], (), 7u64));
        let leer = || Ok((Vec::new(), (), 7u64));
        let fehler = |f: &str| Err(f.to_string());
        assert_eq!(erste_antwort([mit(1), mit(2)]), mit(1));
        assert_eq!(erste_antwort([fehler("a"), mit(2)]), mit(2));
        assert_eq!(erste_antwort([leer(), mit(2)]), mit(2));
        // Nach der ersten mit Token liest niemand mehr.
        let mut gelesen = 0;
        let folge = [mit(1), mit(2), mit(3)].into_iter().inspect(|_| gelesen += 1);
        assert_eq!(erste_antwort(folge), mit(1));
        assert_eq!(gelesen, 1);
        assert_eq!(erste_antwort([fehler("a"), leer()]), leer());
        assert_eq!(erste_antwort([leer(), fehler("b")]), leer());
        assert_eq!(erste_antwort([fehler("a"), fehler("b")]), fehler("b"));
        assert!(erste_antwort(Vec::<Result<(Vec<u32>, (), u64), String>>::new()).is_err());
    }
}
