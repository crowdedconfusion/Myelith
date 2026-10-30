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
//! jeden Auftrag ein zweites Mal, in einem eigenen Faden mit eigenem
//! Koordinator. Die Antwort geht nach dem ersten Pod hinaus (optimistisch,
//! Whitepaper 6.4); beide reichen am Ende der Epoche ihr Bündel ein, und die
//! Kette vergütet nur, wenn die Ausgabenwurzeln gleich sind.
//!
//! Der Prozess hält keine Gewichte, nur den Wortschatz.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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

/// Ab welchem Teil einer Epoche das Bündel eingereicht wird, in Sechsteln:
/// im letzten Sechstel, damit die Arbeit der Epoche fast ganz darin steht und
/// noch Zeit für den Weg in einen Block bleibt. Die Länge der Epoche nennt
/// das Podbild.
pub const EINREICHEN_AB_SECHSTEL: u64 = 5;

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
    eingereicht: bool,
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
    pipeline: Hash,
    deckel: u32,
    /// ⚑ **Ein Koordinator je Pod** (2026-10-28): Jedes Zwillingspaar
    /// rechnet seine Sitzungen, und jeder Pod reicht sein Bündel ein. Nach
    /// Pod-Index; ein Platz wird mit der Epoche neu belegt.
    staende: Arc<Mutex<std::collections::BTreeMap<u32, Platz>>>,
}

type Platz = Arc<Mutex<Option<Podstand>>>;

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
            .spawn(move || loop {
                std::thread::sleep(Duration::from_secs(3));
                // Ohne Ausweis (der Knoten startet noch) gibt es nichts zu tun.
                let Ok(ausweis) = a.holen() else { continue };
                let plaetze: Vec<Platz> = s.lock().map(|g| g.values().cloned().collect()).unwrap_or_default();
                for stand in &plaetze {
                    if let Err(f) = einreichen_wenn_faellig(relais, &ausweis, stand, &buch) {
                        eprintln!("[myl-pod] Bündel: {f}");
                    }
                }
                differenzen_melden(relais, &ausweis, &buch);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { relais, ausweis, wortschatz, pipeline, deckel, staende })
    }
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
fn spuren_ablegen(relais: SocketAddr, ausweis: &Ausweis, s: &Podstand, buendel: &myl_types::PoIBundle) {
    let ablage = myl_types::Spurablage {
        epoche: buendel.epoch,
        pod: buendel.pod,
        segments_root: buendel.segments_root,
        eintraege: s
            .koordinator
            .completed_segments()
            .iter()
            .map(|c| {
                let zeugnis = myl_types::Segmentzeugnis { id: c.id, spurwurzel: c.spurwurzel };
                (zeugnis, c.kette().into_iter().map(Hash).collect())
            })
            .collect(),
        // Die Tokens dazu, in derselben Folge: Damit rechnet ein Checker ab
        // den Tokens nach (Stufe 2).
        eingaben: s.koordinator.completed_segments().iter().map(|c| c.eingabe.clone()).collect(),
    };
    let groesse = borsh::to_vec(&ablage).map_or(usize::MAX, |b| b.len());
    if groesse + 1_024 > myl_types::relais::MAX_RELAIS_BYTES {
        eprintln!("[myl-pod] Spuren der Epoche {}: {groesse} Bytes, über der Grenze einer Nachricht, nicht abgelegt", s.epoche);
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

/// Reicht das Bündel der laufenden Epoche ein, wenn es fällig ist.
fn einreichen_wenn_faellig(
    relais: SocketAddr,
    ausweis: &Ausweis,
    stand: &Mutex<Option<Podstand>>,
    buch: &Blaetterbuch,
) -> Result<(), String> {
    let bild = podbild(relais, ausweis)?;
    let mut g = stand.lock().map_err(|_| "Stand vergiftet")?;
    let Some(s) = g.as_mut() else { return Ok(()) };
    let laenge = bild.bloecke_je_epoche.max(1);
    if s.eingereicht || s.epoche != bild.epoche || bild.hoehe % laenge < laenge * EINREICHEN_AB_SECHSTEL / 6 {
        return Ok(());
    }
    if s.koordinator.completed_segments().is_empty() {
        return Ok(());
    }
    let buendel = s.koordinator.build_signed_poi_bundle()?;
    let bytes = borsh::to_vec(&buendel).map_err(|e| e.to_string())?;
    // Eingereicht ist eingereicht, auch wenn es scheitert: Ein zweiter
    // Versuch derselben Epoche fände dieselben Gründe.
    s.eingereicht = true;
    let antwort = myl_types::relais::fragen(relais, ausweis, &Relaisfrage::BuendelEinreichen { buendel: bytes }, Duration::from_secs(30));
    // ⚑ Stufe 2: Danach legt jedes Mitglied die Spuren ab, damit es auf die
    //   Stichprobe antworten kann, wenn sein Shard längst endete (Fund 680:
    //   nach dem Einreichen, nicht davor).
    spuren_ablegen(relais, ausweis, s, &buendel);
    match antwort? {
        Relaisantwort::Erledigt(z) => {
            eprintln!("[myl-pod] Bündel der Epoche {}: {z}", s.epoche);
            // Die Blätter für einen Differenzbeweis nach dem Wechsel.
            if let Some(eintrag) = bild.pods.iter().find(|p| p.kennung == s.kennung) {
                let (ausgaben, segmente) = s.koordinator.blaetter();
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
/// Festlegung des Projektinhabers vom 2026-10-28, wie Petals: der Weg ersetzt
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
struct Arbeit<'a> {
    sitzung: u64,
    eingabe: &'a [u32],
    deckel: u64,
}

/// Rechnet einen Auftrag einmal über `pod`: Token, letztes Segment, Epoche.
/// Der Koordinator in `stand` entsteht je Epoche und Pod neu.
fn ueber_pod(
    relais: SocketAddr,
    ausweis: &Ausweis,
    stand: &Mutex<Option<Podstand>>,
    epoche: u64,
    pod: &myl_types::relais::Podeintrag,
    arbeit: Arbeit<'_>,
) -> Result<(Vec<u32>, SegmentId, u64), String> {
    let Arbeit { sitzung, eingabe, deckel } = arbeit;
    let mut g = stand.lock().map_err(|_| "Stand vergiftet")?;
    if g.as_ref().map(|s| (s.epoche, s.kennung)) != Some((epoche, pod.kennung)) {
        // Erst wenn der Pod steht (Fund 649, zweiter Teil).
        let bis = std::time::Instant::now() + BEREIT_FRIST;
        while !pod_bereit(relais, ausweis, pod) && std::time::Instant::now() < bis {
            std::thread::sleep(Duration::from_secs(2));
        }
        // ⚑ Wer dann noch fehlt, wird aus der Reserve ersetzt (6.8).
        let (mitglieder, vertreter) = besetzen(relais, ausweis, epoche, pod);
        let weg = Arc::new(UeberDenKnoten::neu(relais, *ausweis, mitglieder.clone()));
        let mut k = Coordinator::ueber_weg(pod.kennung, EpochId(epoche), weg, crate::coordinator::DEFAULT_WINDOW_MS)?;
        k.vertreter_setzen(vertreter);
        *g = Some(Podstand { epoche, kennung: pod.kennung, koordinator: k, eingereicht: false, adressen: mitglieder });
    }
    let s = g.as_mut().ok_or("kein Koordinator")?;
    let beginn = std::time::Instant::now();
    let token = s.koordinator.run_prompt(sitzung, eingabe, deckel);
    eprintln!(
        "[myl-pod] Epoche {}, Pod {}: {} Prompt-Token, {} neue in {} ms",
        s.epoche,
        pod.pod_index,
        eingabe.len(),
        token.len(),
        beginn.elapsed().as_millis()
    );
    let segment = s.koordinator.completed_segments().last().map(|c| c.id).unwrap_or(SegmentId::new([0u8; 32]));
    s.koordinator.sitzung_abschliessen(sitzung);
    Ok((token, segment, s.epoche))
}

impl Podwerk {
    /// Der Platz des Koordinators für `pod`.
    fn platz(&self, pod: u32) -> Result<Platz, String> {
        let mut g = self.staende.lock().map_err(|_| "Stände vergiftet")?;
        Ok(Arc::clone(g.entry(pod).or_default()))
    }

    /// Rechnet einen Auftrag einmal über den Pod, den die Sitzung zugelost
    /// bekommt ([`paar_waehlen`]), und, hat er einen Zwilling, im
    /// Hintergrund noch einmal über diesen.
    fn einmal(
        &self,
        bild: &Podbild,
        ausweis: &Ausweis,
        sitzung: u64,
        eingabe: &[u32],
        deckel: u64,
    ) -> Result<(Vec<u32>, SegmentId, u64), String> {
        let (pod, zwilling) = paar_waehlen(bild, sitzung).ok_or("kein Pod in dieser Epoche")?;
        let platz = self.platz(pod.pod_index)?;
        let mut ergebnis = ueber_pod(self.relais, ausweis, &platz, bild.epoche, pod, Arbeit { sitzung, eingabe, deckel })?;
        // ⚑ Kein Token **mitten in der Epoche**: Der Pod wird einmal neu
        //   besetzt (ein Mitglied kann ausgefallen sein, 6.8) und rechnet
        //   erneut. 📌 Nur in derselben Epoche: Ist sie inzwischen vorbei,
        //   rechnet der Pod der neuen (Fund 649); der erste Einbau wartete
        //   hier auf den alten Pod und verlor die Antwort.
        let dieselbe = podbild(self.relais, ausweis).is_ok_and(|neu| neu.epoche == bild.epoche);
        if ergebnis.0.is_empty() && dieselbe {
            if let Ok(mut g) = platz.lock() {
                *g = None;
            }
            ergebnis = ueber_pod(self.relais, ausweis, &platz, bild.epoche, pod, Arbeit { sitzung, eingabe, deckel })?;
        }
        // ⚑ Stufe 1: derselbe Auftrag über den Zwilling, ohne die Antwort
        //   aufzuhalten. Ein Faden je Auftrag; sie warten auf denselben
        //   Koordinator und rechnen damit in der Folge der Aufträge.
        if let Some(zwilling) = zwilling.cloned() {
            let (relais, ausweis, stand, epoche) = (self.relais, *ausweis, self.platz(zwilling.pod_index)?, bild.epoche);
            let eingabe = eingabe.to_vec();
            let _ = std::thread::Builder::new().name("zwilling".into()).spawn(move || {
                let arbeit = Arbeit { sitzung, eingabe: &eingabe, deckel };
                if let Err(f) = ueber_pod(relais, &ausweis, &stand, epoche, &zwilling, arbeit) {
                    eprintln!("[myl-pod] Zwilling (Pod {}): {f}", zwilling.pod_index);
                }
            });
        }
        Ok(ergebnis)
    }
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
        let ausweis = match self.ausweis.holen() {
            Ok(a) => a,
            Err(f) => return abgelehnt(&f),
        };
        let deckel = u64::from(auftrag.max_token.min(self.deckel));
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
        let ausgabe: Vec<usize> = token.iter().map(|t| *t as usize).collect();
        Inferenzantwort::Ergebnis { sitzung, text: self.wortschatz.decode(&ausgabe), token, segment, prompt_token: eingabe.len() as u64 }
    }

    fn pipeline(&self) -> Hash {
        self.pipeline
    }

    fn shards(&self) -> u32 {
        let plaetze: Vec<Platz> = self.staende.lock().map(|g| g.values().cloned().collect()).unwrap_or_default();
        plaetze
            .iter()
            .find_map(|p| p.lock().ok().and_then(|g| g.as_ref().map(|s| s.koordinator.num_shards() as u32)))
            .unwrap_or(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
