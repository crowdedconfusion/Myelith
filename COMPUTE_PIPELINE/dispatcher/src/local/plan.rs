//! **Der Belegungsplan**: eine reine Funktion in ganzen Zahlen.
//!
//! ```text
//! belegen(angebot, lasten, regeln) → Plan
//! ```
//!
//! # ⚑ Die Reihenfolge der Zuteilung, und warum sie diese ist
//!
//! **Erst jedes Minimum, in Vorrangfolge; dann der Ueberschuss, wieder in
//! Vorrangfolge.**
//!
//! 1. **Minimum.** Fuer ein eigenes Artefakt, in dieser Folge:
//!    - der **feste Teil**: KV-Speicher fuer den Mindestkontext, Zustand,
//!      Zuschlag. Nichts davon laesst sich auslagern; passt er nicht, wird
//!      die Last abgelehnt;
//!    - die **dichten Gewichte** im Arbeitsspeicher, Rand und Ebene um
//!      Ebene, denn sie liest jedes Token; was nicht passt, kommt per mmap
//!      von der Platte, und der Plan sagt, dass es langsam wird;
//!    - ein **Arbeitskontext bis [`KONTEXT_VOR_EXPERTEN`]** (32 768 Token,
//!      hoechstens der eingestellte), so weit er passt: Darunter kommt ein
//!      Agent nicht ueber den ersten Schritt (Fund 562);
//!    - die **heissen Experten**, falls ein Profil vorliegt
//!      ([`crate::local::profil`]): die haeufigsten, bis sie zusammen
//!      [`HEISS_DECKUNG_PROMILLE`] der Aufrufe tragen. Nach den dichten
//!      Gewichten sind sie das, was am haeufigsten gelesen wird;
//!    - der **restliche Arbeitskontext** (`kontext_soll`), bis er erreicht ist oder der
//!      Platz endet, auf ein Raster abgerundet. Er ist eine Einstellung des
//!      Menschen und kein Tempo; wird er gekuerzt, sagt der Plan es.
//!
//!    Fuer ein fremdes Modell: alle seine Dateien und sein Zuschlag. In
//!    Vorrangfolge Agent, Vision, Sprache. ⚑ **Ein fremdes Modell laeuft
//!    je Aufruf als eigener Prozess und darf sich dafuer die heissen
//!    Experten einer hoeheren Last leihen**, nie ihre dichten Gewichte,
//!    ihren festen Teil oder ihren Arbeitskontext (siehe unten).
//! 2. **Ueberschuss**, nur fuer eigene Artefakte: die uebrigen gerouteten
//!    Experten im Arbeitsspeicher statt von der Platte, **die haeufigsten
//!    zuerst** (ohne Profil in der Reihenfolge des Artefakts).
//!
//! ⚑ **Damit weicht eine niedrigere Last nie fuer etwas, das eine hoehere
//! nur ein wenig schneller macht, aber immer fuer etwas, das sie erst so
//! laufen laesst, wie sie eingestellt ist.** Die dichten Gewichte und der
//! Arbeitskontext sind die Last selbst; die heissen Experten liest sie so
//! oft, dass sie praktisch dazugehoeren. Die seltenen Experten sind Tempo
//! am Rand.
//!
//! ⚑ **Festlegung des Projektinhabers vom 2026-10-15**: Die haeufig
//! genutzten Experten kommen direkt nach den dichten Gewichten, also auch
//! vor dem Arbeitskontext und vor Sehen und Hoeren. Gemessen am int8-35B
//! traegt dort ein Fuenftel der Experten gut drei Viertel der Aufrufe; jeder
//! davon, der von der Platte kommt, kostet jedes Token Zeit, waehrend ein
//! Kontext ueber dem Mindestmass nur gebraucht wird, wenn der Verlauf so
//! lang ist. ⚠️ **Der Arbeitskontext kann dafuer bis zum Mindestkontext
//! weichen**, und der Plan sagt es dann.
//!
//! ⚑ **Ausser fuer einen Aufruf von Sehen oder Hoeren** (Entscheidung nach
//! der Belegungssimulation vom 2026-10-15). Haelt der Plan die heissen
//! Experten auch gegen sie fest, lehnt er beim int8-35B auf 24 GiB das
//! Sehen ganz ab, um nach dem Aufruf rund 2 % Tempo zu sparen: Der
//! Dateicache fuellt die verdraengten haeufigen Experten in den naechsten
//! Token von selbst nach, eben weil sie haeufig sind. Ein abgelehntes Bild
//! dagegen ist eine Faehigkeit weniger. Der Plan nimmt dann die seltensten
//! der heissen zuerst und sagt, wie viel geliehen ist.
//!
//! ⚑ **Mehr Kontext als eingestellt gibt es nicht**: Der Client fasst ab
//! dem Arbeitskontext zusammen, und Speicher dafuer waere verschenkt.
//!
//! 📌 *Beim ersten Probelauf am 2026-10-15 kam der Kontext nach dem
//! Minimum und vor den Experten, mit dem Hoechstkontext des Modells als
//! Wunsch. Beim int8-35B auf 24 GB ging damit der ganze Ueberschuss in
//! 183 808 Token Kontext (3,8 GB), und kein einziger Experte lag im
//! Speicher.*
//!
//! # ⚑ Was der Plan heute mit den Karten tut
//!
//! **Er teilt sie zu, er belegt sie nicht.** Auf einer Steckkarte rechnen
//! heute die gebuendelten Matrizen, und die Gewichte gehen je Aufruf hinauf;
//! Grafikspeicher ist damit Puffer und keine Ablage. Eine eingebaute GPU mit
//! gemeinsamem Speicher liest die Gewichte ohne Kopie. Die Karten gehen an
//! die eigene Last mit dem hoechsten Vorrang, die angenommen ist. Liegen
//! Gewichte einmal dauerhaft auf einer Steckkarte, wird sie hier eine Stufe
//! ueber dem Arbeitsspeicher, mit den dichten Teilen zuerst und den
//! Experten nach ihrer Haeufigkeit.
//!
//! # ⚑ Kerne
//!
//! Jede angenommene Last bekommt ihre Mindestzahl; den Rest bekommt die
//! eigene Last mit dem hoechsten Vorrang, denn sie rechnet am Rechenpfad,
//! dessen Kerngrenze dieser Plan setzt. Fremde Laufzeiten waehlen ihre
//! Faeden selbst; ihre Mindestzahl ist das, was ihnen frei bleibt.

use crate::local::bedarf::Modellbedarf;
use crate::local::erhebung::{Hardware, Rueckseite, GIB};
pub use crate::local::profil::HEISS_DECKUNG_PROMILLE;
use crate::local::profil::Expertenprofil;

/// **Wer zuerst kommt.** Festlegung des Projektinhabers: Agent vor Vision
/// vor Sprache, und seit dem 2026-10-21 **das Netz vor allem** (Netz vor
/// Mensch). Die Reihenfolge der Varianten ist die Reihenfolge der
/// Zuteilung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum Vorrang {
    /// ⚑ **Der Shard, den das Netz diesem Rechner für die laufende Epoche
    /// zugeteilt hat** (Stufe 4, P8, Festlegung E7). Er kommt vor jeder
    /// lokalen Last; der Agent bekommt den Rest. Wer sein Gerät zurück will,
    /// sagt für die nächste Epoche weniger zu.
    Netz,
    /// Das eigene Sprachmodell des Agenten.
    Agent,
    /// Sehen: Bilder in Worte.
    Vision,
    /// Hoeren und Sprechen.
    Sprache,
}

impl Vorrang {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Netz => "Netz",
            Self::Agent => "Agent",
            Self::Vision => "Vision",
            Self::Sprache => "Sprache",
        }
    }
}

/// Was eine Last an Gewichten mitbringt.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Gewichte {
    /// Ein eigenes Artefakt, auf das Byte bekannt.
    Eigen(Modellbedarf),
    /// Ein fremdes Modell: die Summe seiner Dateien.
    Fremd { bytes: u64 },
}

/// Eine Arbeitslast, die auf dieser Maschine laufen will.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Last {
    pub name: String,
    pub vorrang: Vorrang,
    pub gewichte: Gewichte,
    /// Was keine Datei nennt; aus einer Eichung oder die Vorgabe.
    pub zuschlag_bytes: u64,
    /// Darunter laeuft die Last nicht (nur eigene Artefakte).
    pub kontext_mindestens: u64,
    /// **Der Kontext, mit dem sie arbeitet**, beim Agenten der eingestellte
    /// Arbeitskontext. Gehoert zu ihrem Minimum; passt er nicht ganz, wird
    /// er bis `kontext_mindestens` gekuerzt (nur eigene Artefakte).
    pub kontext_soll: u64,
    /// Kerne, die ihr mindestens bleiben.
    pub kerne_mindestens: usize,
    /// **Wie oft welcher Experte gewaehlt wird** (nur Gemische). Ohne
    /// Profil gibt es keine heissen Experten, und die uebrigen kommen in der
    /// Reihenfolge des Artefakts.
    #[serde(skip)]
    pub profil: Option<Expertenprofil>,
}

/// Eine Karte, wie der Plan sie sieht.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Karte {
    pub kennung: String,
    pub name: String,
    pub rueckseite: Option<Rueckseite>,
    pub frei_bytes: Option<u64>,
    pub gemeinsamer_speicher: bool,
}

/// **Was der Plan verteilen darf**: das Freie, geschnitten mit der
/// Freigabe des Besitzers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Angebot {
    pub ram_bytes: u64,
    pub kerne: usize,
    /// Nur Karten, die ein Rechenweg bedient und die nicht abgeschaltet
    /// sind.
    pub karten: Vec<Karte>,
}

/// Was der Besitzer hergibt. `None` heisst ohne Grenze, wie in der
/// Freigabemaske.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Freigabe {
    pub kerne: Option<usize>,
    pub ram_bytes: Option<u64>,
    /// Kennungen abgeschalteter Rechenwerke.
    pub karten_aus: Vec<String>,
}

/// **Der Arbeitsspeicher, den ein Angebot nennt**, so bemessen, dass nach
/// Abzug der Ruecklage (im Plan) das Kleinere von „frei minus Ruecklage“ und
/// der Freigabe bleibt.
///
/// ⚑ **Die Ruecklage schuetzt das System, nicht die Freigabe.** Wer von 24
/// GiB vier freigibt, soll diese vier nutzen koennen; das System hat dann
/// ohnehin genug. Nur wo das Freie knapper ist als die Freigabe, bleibt die
/// Ruecklage davon frei.
///
/// 📌 *Bis zum 2026-10-15 stand hier das Kleinere von frei und Freigabe,
/// und der Plan zog die Ruecklage davon ab: Eine Freigabe von 4 GiB gab
/// nur 3 GiB her.*
pub fn ram_angebot(frei: u64, freigabe: Option<u64>, ruecklage: u64) -> u64 {
    freigabe.map_or(frei, |g| frei.min(g.saturating_add(ruecklage)))
}

impl Angebot {
    /// **Aus der Erhebung, der Freigabe und den Regeln** (fuer die
    /// Ruecklage, siehe [`ram_angebot`]).
    ///
    /// ⚑ **Ohne erkannten freien Speicher gibt es keinen Plan**, und zwar
    /// als Fehler mit Grund statt als Plan auf eine geratene Zahl.
    pub fn aus(h: &Hardware, f: &Freigabe, regeln: &Regeln) -> Result<Self, String> {
        let frei = h
            .speicher_frei_bytes
            .ok_or("Der freie Arbeitsspeicher ist auf dieser Maschine nicht erkannt; ohne ihn gibt es keinen Plan.")?;
        let ram_bytes = ram_angebot(frei, f.ram_bytes, regeln.ruecklage_bytes);
        let kerne = f.kerne.map_or(h.kerne, |k| k.clamp(1, h.kerne));
        let karten = h
            .rechenwerke
            .iter()
            .filter(|r| r.rueckseite.is_some() && !f.karten_aus.contains(&r.kennung))
            .map(|r| Karte {
                kennung: r.kennung.clone(),
                name: r.beschriftung(),
                rueckseite: r.rueckseite,
                frei_bytes: r.frei_bytes,
                gemeinsamer_speicher: r.gemeinsamer_speicher,
            })
            .collect();
        Ok(Self { ram_bytes, kerne, karten })
    }
}

/// Die Stellschrauben des Plans.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Regeln {
    /// **Was frei bleibt**, fuer das System und fuer das, was nach der
    /// Erhebung jemand oeffnet.
    pub ruecklage_bytes: u64,
    /// Auf dieses Vielfache wird ein gewaehrter Kontext abgerundet.
    pub kontext_raster: u64,
}

/// ⚠️ **Eine Vorgabe und keine Messung**: ein Gibibyte Ruecklage. Eine
/// volle Maschine ist eine langsame, und die Zahl laesst sich setzen.
pub const VORGABE_RUECKLAGE: u64 = GIB;

/// **Die Untergrenze des Arbeitskontexts**: Darunter passen Werkzeugansage,
/// Verlauf und Antwort eines Agenten nicht nebeneinander. Dieselbe Zahl wie
/// im Client, gebunden durch eine Probe dort.
pub const KONTEXT_MINDESTENS: u64 = 8_192;

/// **Der Arbeitskontext ohne Einstellung**, wie die Vorgabe im Client.
pub const KONTEXT_VORGABE: u64 = 65_536;

/// **So viel Arbeitskontext kommt vor den heissen Experten**, wenn das
/// Modell ihn hat und der Speicher ihn traegt (Fund 562, 2026-10-18).
///
/// 📌 **Warum nicht einfach ein hoeheres Minimum.** Bis hier kam nach dem
/// Minimum von [`KONTEXT_MINDESTENS`] (8192) jeder Experte vor jedem
/// weiteren Token Kontext. Beim int8-35B auf 24 GiB blieb es bei 8192, und
/// ein Agent kam damit nicht ueber den ersten Schritt: Systemprompt und
/// voller Werkzeugsatz belegen rund 4200 Token, die Antwort haelt 4000
/// frei. Jeder der 15 Laeufe der Stufe extrem endete nach hoechstens fuenf
/// Aufrufen mit „Kontext voll“. Ein Minimum von 32 768 haette das behoben,
/// aber fuer alle: Die dichten kleinen Modelle brauchen 112 bis 144 KiB je
/// Token, ein 4B also 4,5 GiB fest, und wuerde auf einem kleinen Rechner
/// abgelehnt. Das Minimum bleibt deshalb die Schwelle fuer die Aufnahme;
/// diese Zahl ist die Schwelle fuer den Vorrang der Experten.
///
/// ⚑ **Was es kostet:** Beim 35B traegt ein Token 20 KiB (zehn
/// Aufmerksamkeitsebenen), 8192 bis 32 768 also 480 MiB, rund 160 der
/// seltensten unter gut 2000 haeufigen Experten. Ein Modell ohne Experten
/// verliert nichts, es hat keinen Vorrang abzugeben.
pub const KONTEXT_VOR_EXPERTEN: u64 = 32_768;

/// Erweitert den Kontext der Belegung bis `ziel`, so weit der Speicher
/// reicht, auf `raster` abgerundet (ausser er erreicht `ziel` genau).
fn kontext_erweitern(bel: &mut Belegung, frei: &mut u64, b: &Modellbedarf, ziel: u64, raster: u64) {
    if ziel <= bel.kontext {
        return;
    }
    // Ohne KV-Speicher (ein rein rekurrentes Modell) kostet Kontext nichts.
    let neu = match frei.checked_div(b.kv_je_token) {
        None => ziel,
        Some(passt) => {
            let neu = bel.kontext + passt.min(ziel - bel.kontext);
            if neu == ziel { ziel } else { (neu / raster * raster).max(bel.kontext) }
        }
    };
    let kosten = (neu - bel.kontext) * b.kv_je_token;
    *frei -= kosten;
    bel.ram_bytes += kosten;
    bel.kontext = neu;
}

/// Kontext in Schritten von 256 Token: fein genug, um nichts zu
/// verschenken, grob genug, dass zwei Plaene mit fast gleichem Speicher
/// denselben Kontext ergeben.
pub const VORGABE_KONTEXT_RASTER: u64 = 256;

impl Default for Regeln {
    fn default() -> Self {
        Self { ruecklage_bytes: VORGABE_RUECKLAGE, kontext_raster: VORGABE_KONTEXT_RASTER }
    }
}

/// Was eine angenommene Last bekommt.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Belegung {
    /// Alles, was im Arbeitsspeicher fuer sie zurueckgehalten ist.
    pub ram_bytes: u64,
    /// Davon Gewichte.
    pub gewichte_im_ram: u64,
    /// Gewichte, die per mmap von der Platte kommen.
    pub gewichte_von_platte: u64,
    /// Dichte Ebenen, die nicht ganz in den Arbeitsspeicher passten.
    pub ebenen_von_platte: Vec<usize>,
    /// Gerouteter Experten, die von der Platte kommen.
    pub experten_von_platte: u64,
    /// Davon im Arbeitsspeicher, als die heissen eines Profils, vor dem
    /// Arbeitskontext; sie sind die vordersten der Rangfolge.
    pub experten_heiss: u64,
    /// Ihre Bytes; davon darf sich ein Aufruf eines fremden Modells
    /// leihen (Kopf dieses Moduls).
    pub heiss_bytes: u64,
    /// **Welcher Anteil der Expertenaufrufe** (Promille) nach dem Profil
    /// auf Experten im Arbeitsspeicher faellt; ohne Profil `None`.
    pub deckung_promille: Option<u32>,
    /// Gewaehrter Kontext (null bei fremden Modellen).
    pub kontext: u64,
    pub kerne: usize,
    /// Karten, die fuer sie rechnen.
    pub karten: Vec<String>,
    pub hinweise: Vec<String>,
}

/// Wie es fuer eine Last ausgeht.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Ergebnis {
    Angenommen(Belegung),
    Abgelehnt { grund: String },
}

/// Eine Zeile des Plans.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Zuteilung {
    pub name: String,
    pub vorrang: Vorrang,
    pub ergebnis: Ergebnis,
}

/// Der Plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Plan {
    /// In Vorrangfolge, bei gleichem Vorrang in der Reihenfolge der Eingabe.
    pub zuteilungen: Vec<Zuteilung>,
    /// Was nach der Ruecklage zu verteilen war.
    pub verteilbar_bytes: u64,
    /// Was danach frei bleibt.
    pub rest_bytes: u64,
}

impl Plan {
    /// Die Zeile einer Last.
    pub fn zeile(&self, name: &str) -> Option<&Zuteilung> {
        self.zuteilungen.iter().find(|z| z.name == name)
    }

    /// Die Belegung einer Last, falls sie angenommen ist.
    pub fn belegung(&self, name: &str) -> Option<&Belegung> {
        match &self.zeile(name)?.ergebnis {
            Ergebnis::Angenommen(b) => Some(b),
            Ergebnis::Abgelehnt { .. } => None,
        }
    }
}

/// **Was eine Ebene dauerhaft belegt**: dichte Gewichte, alle ihre
/// gerouteten Experten und ihr Anteil am KV-Speicher (achtsam) bzw. am
/// Zustand (rekurrent) fuer `kontext` Token.
fn ebene_belegt(b: &Modellbedarf, e: &crate::local::bedarf::Ebene, kontext: u64) -> u64 {
    use crate::local::bedarf::Ebenenart;
    let anzahl = |art| b.ebenen.iter().filter(|x| x.art == art).count() as u64;
    let folge = match e.art {
        Ebenenart::Achtsam => (b.kv_je_token / anzahl(Ebenenart::Achtsam).max(1)).saturating_mul(kontext),
        Ebenenart::Rekurrent => b.zustand_je_folge / anzahl(Ebenenart::Rekurrent).max(1),
    };
    e.dicht_bytes.saturating_add(e.experten_bytes.iter().sum::<u64>()).saturating_add(folge)
}

/// **Wie viele zusammenhaengende Ebenen dauerhaft in `budget` passen**: die
/// groesste Zahl n, fuer die **jedes** Fenster aus n Ebenen hineinpasst.
///
/// ⚑ **Jedes Fenster und nicht ein durchschnittliches**, weil die Ebenen
/// verschieden schwer sind (beim hybriden 35B mischen 30 von 40 rekurrent
/// und tragen keinen KV-Speicher); wem ein Bereich zugeteilt wird, kann
/// nicht waehlen, welcher. Einbettung und Kopf zaehlen nicht mit: Sie
/// gehoeren an den ersten und den letzten Bereich und stehen im Rand.
///
/// Das ist die Zahl, die ein Knoten fuer ein Modell ankuendigen kann, und
/// fuer eine Steckkarte die, die dauerhaft darauf laege, wenn Gewichte dort
/// bleiben duerften.
pub fn ebenen_hoechstens(b: &Modellbedarf, budget: u64, kontext: u64) -> usize {
    let kosten: Vec<u64> = b.ebenen.iter().map(|e| ebene_belegt(b, e, kontext)).collect();
    let mut beste = 0;
    for n in 1..=kosten.len() {
        let schwerstes = kosten.windows(n).map(|w| w.iter().fold(0u64, |s, x| s.saturating_add(*x))).max().unwrap_or(0);
        if schwerstes > budget {
            break;
        }
        beste = n;
    }
    beste
}

/// **Der Plan als Text fuer Menschen**, Zeile um Zeile.
///
/// ⚑ **Eine Stelle fuer jede Anzeige**: das Programm `dispatcher` und
/// `myl plan` zeigen denselben Plan, und zwei Schreibweisen derselben
/// Zeile liefen auseinander.
///
/// ⚑ **Je Steckkarte mit bekanntem Freiem** steht dabei, wie viele Ebenen
/// der ersten eigenen Last dauerhaft darauf passten (nach Abzug der
/// Ruecklage, mit dem gewaehrten Kontext). Das ist Auskunft: Heute gehen die
/// Gewichte je Aufruf auf die Karte, und genau diese Zahl gehoert zur
/// Entscheidung, ob sie dort bleiben sollen.
pub fn bericht(angebot: &Angebot, lasten: &[Last], regeln: &Regeln, p: &Plan) -> String {
    let mut aus = vec![format!(
        "Angebot: {} Arbeitsspeicher, {} Kerne, {} Rechenwerke; Rücklage {}, zu verteilen {}",
        gib(angebot.ram_bytes),
        angebot.kerne,
        angebot.karten.len(),
        gib(regeln.ruecklage_bytes),
        gib(p.verteilbar_bytes)
    )];
    for z in &p.zuteilungen {
        match &z.ergebnis {
            Ergebnis::Angenommen(b) if b.kontext == 0 => {
                aus.push(format!("{:<8} {}: {} im Speicher", z.vorrang.name(), z.name, gib(b.ram_bytes)));
            }
            Ergebnis::Angenommen(b) => aus.push(format!(
                "{:<8} {}: {} im Speicher (Gewichte {}, von der Platte {}), Kontext {}, {} Kerne{}",
                z.vorrang.name(),
                z.name,
                gib(b.ram_bytes),
                gib(b.gewichte_im_ram),
                gib(b.gewichte_von_platte),
                b.kontext,
                b.kerne,
                if b.karten.is_empty() { String::new() } else { format!(", Rechenwerke {}", b.karten.join(" ")) }
            )),
            Ergebnis::Abgelehnt { grund } => aus.push(format!("{:<8} {}: abgelehnt. {grund}", z.vorrang.name(), z.name)),
        }
        if let Ergebnis::Angenommen(b) = &z.ergebnis {
            aus.extend(b.hinweise.iter().map(|h| format!("         {h}")));
        }
    }
    let eigen = lasten.iter().find_map(|l| match (&l.gewichte, p.belegung(&l.name)) {
        (Gewichte::Eigen(b), Some(bel)) => Some((b, bel.kontext)),
        _ => None,
    });
    if let Some((b, kontext)) = eigen {
        for k in angebot.karten.iter().filter(|k| !k.gemeinsamer_speicher) {
            match k.frei_bytes {
                Some(f) => aus.push(format!(
                    "Karte    {}: {} frei; dauerhaft passten {} von {} Ebenen von {} (dicht und alle Experten, Kontext {})",
                    k.kennung,
                    gib(f),
                    ebenen_hoechstens(b, f.saturating_sub(regeln.ruecklage_bytes), kontext),
                    b.ebenen.len(),
                    b.name,
                    kontext
                )),
                None => aus.push(format!("Karte    {}: das Freie ist nicht erkannt", k.kennung)),
            }
        }
    }
    aus.push(format!("Frei danach: {}", gib(p.rest_bytes)));
    aus.join("\n")
}

/// Bytes lesbar, ganzzahlig auf eine Nachkommastelle.
pub fn gib(b: u64) -> String {
    let zehntel = (b as u128 * 10 + GIB as u128 / 2) / GIB as u128;
    format!("{},{} GiB", zehntel / 10, zehntel % 10)
}

/// **Was ein eigenes Artefakt im Speicher halten muss, um fluessig zu
/// laufen**: KV-Speicher fuer `kontext` Token, Zustand, Zuschlag und alle
/// dichten Gewichte (samt Einbettung und Kopf). Die gerouteten Experten
/// zaehlen nicht mit; sie kommen bei Bedarf ueber den Dateicache.
///
/// ⚑ **Dieselbe Grenze wie im Plan**: Bis hierher gehoert es zum Minimum
/// einer Last (Kopf dieses Moduls). Wer eine Freigabe gegen ein Artefakt
/// prueft, prueft gegen diese Zahl und nicht gegen die Groesse auf der
/// Platte, die bei einem Gemisch vor allem aus Experten besteht.
pub fn fluessig_bytes(b: &Modellbedarf, kontext: u64, zuschlag: u64) -> Option<u64> {
    b.kv_bytes(kontext)?.checked_add(b.zustand_je_folge)?.checked_add(zuschlag)?.checked_add(b.dicht_bytes())
}

/// Der feste Teil einer Last: was nicht ausgelagert werden kann.
fn fester_teil(l: &Last, kontext: u64) -> Option<u64> {
    match &l.gewichte {
        Gewichte::Eigen(b) => b.kv_bytes(kontext)?.checked_add(b.zustand_je_folge)?.checked_add(l.zuschlag_bytes),
        Gewichte::Fremd { bytes } => bytes.checked_add(l.zuschlag_bytes),
    }
}

/// **Die Experten einer Last in der Folge, in der sie Speicher bekommen**,
/// als `(Platz der Ebene im Bedarf, Experte)`, und wie viele vorne heiss
/// sind.
struct Rang {
    reihe: Vec<(usize, usize)>,
    heisse: usize,
}

impl Rang {
    fn aus(l: &Last) -> Self {
        let Gewichte::Eigen(b) = &l.gewichte else {
            return Self { reihe: Vec::new(), heisse: 0 };
        };
        // Vom Index der Ebene (wie ihn das Profil zaehlt) auf ihren Platz.
        let platz = |index: usize| b.ebenen.iter().position(|e| e.index == index).expect("aus dem Bedarf");
        match &l.profil {
            Some(p) => {
                let reihe = p.rangfolge(b);
                let heisse = p.heisse(&reihe, HEISS_DECKUNG_PROMILLE);
                Self { reihe: reihe.into_iter().map(|(e, x)| (platz(e), x)).collect(), heisse }
            }
            None => Self {
                reihe: b.ebenen.iter().enumerate().flat_map(|(k, e)| (0..e.experten_bytes.len()).map(move |x| (k, x))).collect(),
                heisse: 0,
            },
        }
    }
}

/// **Stellt den Plan auf.** Rein und deterministisch: dieselben Eingaben,
/// derselbe Plan.
pub fn belegen(angebot: &Angebot, lasten: &[Last], regeln: &Regeln) -> Plan {
    let verteilbar = angebot.ram_bytes.saturating_sub(regeln.ruecklage_bytes);
    let mut frei = verteilbar;
    let mut kerne_frei = angebot.kerne;
    let raster = regeln.kontext_raster.max(1);

    // In Vorrangfolge; bei gleichem Vorrang zaehlt die Eingabe.
    let mut reihe: Vec<usize> = (0..lasten.len()).collect();
    reihe.sort_by_key(|&i| (lasten[i].vorrang, i));

    // Je Last die Rangfolge ihrer Experten, und welche davon im Speicher sind.
    let rang: Vec<Rang> = lasten.iter().map(Rang::aus).collect();
    let mut genommen: Vec<Vec<bool>> = rang.iter().map(|r| vec![false; r.reihe.len()]).collect();
    let mut belegt: Vec<Option<Belegung>> = vec![None; lasten.len()];
    let mut gruende: Vec<Option<String>> = vec![None; lasten.len()];
    // Je eigener Last: Gewichtsbytes im Speicher.
    let mut gewichte_im_ram = vec![0u64; lasten.len()];

    // ── 1. Minimum ────────────────────────────────────────────────────
    for &i in &reihe {
        let l = &lasten[i];
        if let Gewichte::Eigen(b) = &l.gewichte {
            if l.kontext_mindestens == 0 || l.kontext_mindestens > b.hoechstkontext {
                gruende[i] = Some(format!(
                    "Der Mindestkontext {} liegt nicht zwischen 1 und dem Höchstkontext {} des Modells.",
                    l.kontext_mindestens, b.hoechstkontext
                ));
                continue;
            }
        }
        if l.kerne_mindestens > kerne_frei {
            gruende[i] = Some(format!(
                "Braucht mindestens {} Kerne, frei sind nach Lasten mit höherem Vorrang {kerne_frei}.",
                l.kerne_mindestens
            ));
            continue;
        }
        let Some(fest) = fester_teil(l, l.kontext_mindestens) else {
            gruende[i] = Some("Der Bedarf läuft über den Zahlbereich.".into());
            continue;
        };
        // Was ein Aufruf eines fremden Modells sich bei hoeheren Lasten leihen darf.
        let leihbar: u64 = match l.gewichte {
            Gewichte::Fremd { .. } => reihe
                .iter()
                .filter(|&&j| lasten[j].vorrang < l.vorrang)
                .filter_map(|&j| belegt[j].as_ref().map(|b| b.heiss_bytes))
                .sum(),
            Gewichte::Eigen(_) => 0,
        };
        if fest > frei.saturating_add(leihbar) {
            gruende[i] = Some(format!(
                "Braucht mindestens {} im Arbeitsspeicher, frei sind nach Lasten mit höherem Vorrang {}{}.",
                gib(fest),
                gib(frei),
                if leihbar > 0 { format!(", leihbar aus deren häufigen Experten {}", gib(leihbar)) } else { String::new() }
            ));
            continue;
        }
        // Leihen, von der niedrigsten der hoeheren Lasten zuerst und dort
        // vom seltensten der heissen Experten an.
        for &j in reihe.iter().rev().filter(|&&j| lasten[j].vorrang < l.vorrang) {
            let (Some(bj), Gewichte::Eigen(b)) = (belegt[j].as_mut(), &lasten[j].gewichte) else { continue };
            let vorher = bj.heiss_bytes;
            for k in (0..rang[j].heisse).rev() {
                if frei >= fest {
                    break;
                }
                if genommen[j][k] {
                    let (e, x) = rang[j].reihe[k];
                    let bytes = b.ebenen[e].experten_bytes[x];
                    genommen[j][k] = false;
                    frei += bytes;
                    gewichte_im_ram[j] -= bytes;
                    bj.heiss_bytes -= bytes;
                    bj.experten_heiss -= 1;
                }
            }
            if bj.heiss_bytes < vorher {
                bj.hinweise.push(format!(
                    "Während {} läuft, leiht es sich {} aus den häufigen Experten; der Dateicache holt sie danach \
                     zurück.",
                    l.name,
                    gib(vorher - bj.heiss_bytes)
                ));
            }
        }
        frei -= fest;
        kerne_frei -= l.kerne_mindestens;
        let mut bel = Belegung {
            ram_bytes: fest,
            gewichte_im_ram: 0,
            gewichte_von_platte: 0,
            ebenen_von_platte: Vec::new(),
            experten_von_platte: 0,
            experten_heiss: 0,
            heiss_bytes: 0,
            deckung_promille: None,
            kontext: 0,
            kerne: l.kerne_mindestens,
            karten: Vec::new(),
            hinweise: Vec::new(),
        };
        if let Gewichte::Eigen(b) = &l.gewichte {
            // Die dichten Gewichte, der Rand zuerst, dann Ebene um Ebene.
            if b.rand_bytes <= frei {
                frei -= b.rand_bytes;
                gewichte_im_ram[i] += b.rand_bytes;
            }
            for e in &b.ebenen {
                if e.dicht_bytes <= frei {
                    frei -= e.dicht_bytes;
                    gewichte_im_ram[i] += e.dicht_bytes;
                } else {
                    bel.ebenen_von_platte.push(e.index);
                }
            }
            // ⚑ **Ein Arbeitskontext, mit dem ein Agent arbeiten kann, vor den
            //   heissen Experten** (Fund 562, siehe [`KONTEXT_VOR_EXPERTEN`]).
            let soll = l.kontext_soll.clamp(l.kontext_mindestens, b.hoechstkontext);
            bel.kontext = l.kontext_mindestens;
            kontext_erweitern(&mut bel, &mut frei, b, soll.min(KONTEXT_VOR_EXPERTEN.max(l.kontext_mindestens)), raster);
            // Die heissen Experten, die haeufigsten zuerst.
            let r = &rang[i];
            for (k, &(e, x)) in r.reihe.iter().enumerate().take(r.heisse) {
                let bytes = b.ebenen[e].experten_bytes[x];
                if bytes <= frei {
                    frei -= bytes;
                    gewichte_im_ram[i] += bytes;
                    genommen[i][k] = true;
                    bel.experten_heiss += 1;
                    bel.heiss_bytes += bytes;
                }
            }
            // Der restliche Arbeitskontext, so weit er passt.
            kontext_erweitern(&mut bel, &mut frei, b, soll, raster);
            if bel.kontext < soll {
                bel.hinweise.push(format!(
                    "Kontext {} statt eingestellt {soll}: Mehr passt nicht in den Arbeitsspeicher{}.",
                    bel.kontext,
                    if bel.experten_heiss > 0 { " neben den häufigen Experten" } else { "" }
                ));
            }
        }
        belegt[i] = Some(bel);
    }

    // ── 2. Ueberschuss: die uebrigen Experten, die haeufigsten zuerst ─
    for &i in &reihe {
        let (Some(_), Gewichte::Eigen(b)) = (&belegt[i], &lasten[i].gewichte) else {
            continue;
        };
        for (k, &(e, x)) in rang[i].reihe.iter().enumerate() {
            let bytes = b.ebenen[e].experten_bytes[x];
            if !genommen[i][k] && bytes <= frei {
                frei -= bytes;
                gewichte_im_ram[i] += bytes;
                genommen[i][k] = true;
            }
        }
    }

    // ── 3. Kerne, Karten, Abschluss ──────────────────────────────────
    let erste_eigene =
        reihe.iter().copied().find(|&i| matches!(lasten[i].gewichte, Gewichte::Eigen(_)) && belegt[i].is_some());
    // ⚑ **Netz vor Mensch, mit Grund** (P8, Festlegung E7): Läuft ein Shard
    //   des Netzes, sagt jede Ablehnung, wer den Platz hat und wie man ihn
    //   zurückbekommt, statt still zu verdrängen.
    let netz_laeuft: Option<String> = reihe
        .iter()
        .find(|&&j| lasten[j].vorrang == Vorrang::Netz && belegt[j].is_some())
        .map(|&j| lasten[j].name.clone());
    let mut zuteilungen = Vec::with_capacity(lasten.len());
    for &i in &reihe {
        let l = &lasten[i];
        let ergebnis = match belegt[i].take() {
            None => {
                let mut grund = gruende[i].take().expect("eine abgelehnte Last hat einen Grund");
                if let (Some(netz), true) = (&netz_laeuft, l.vorrang != Vorrang::Netz) {
                    grund.push_str(&format!(
                        " Das Netz hat Vorrang ({netz}) und rechnet bis zum Ende der Epoche; wer das Gerät zurück \
                         will, sagt für die nächste Epoche weniger zu."
                    ));
                }
                Ergebnis::Abgelehnt { grund }
            }
            Some(mut bel) => {
                if Some(i) == erste_eigene {
                    bel.kerne += kerne_frei;
                    bel.karten = angebot.karten.iter().map(|k| k.kennung.clone()).collect();
                    if angebot.karten.iter().any(|k| k.gemeinsamer_speicher) {
                        bel.hinweise.push(
                            "Die eingebaute GPU rechnet die gebündelten Matrizen und liest die Gewichte ohne Kopie \
                             aus dem gemeinsamen Speicher."
                                .into(),
                        );
                    }
                    if angebot.karten.iter().any(|k| !k.gemeinsamer_speicher) {
                        bel.hinweise.push(
                            "Die Steckkarten rechnen die gebündelten Matrizen; die Gewichte gehen je Aufruf hinauf, \
                             ihr Speicher ist Puffer und keine Ablage."
                                .into(),
                        );
                    }
                }
                match &l.gewichte {
                    Gewichte::Eigen(b) => {
                        bel.gewichte_im_ram = gewichte_im_ram[i];
                        bel.ram_bytes += bel.gewichte_im_ram;
                        bel.gewichte_von_platte = b.gewichte_bytes() - bel.gewichte_im_ram;
                        let alle: u64 = b.ebenen.iter().map(|e| e.experten_bytes.len() as u64).sum();
                        let im_ram: Vec<(usize, usize)> = rang[i]
                            .reihe
                            .iter()
                            .zip(&genommen[i])
                            .filter(|(_, &g)| g)
                            .map(|(&(e, x), _)| (b.ebenen[e].index, x))
                            .collect();
                        bel.experten_von_platte = alle - im_ram.len() as u64;
                        bel.deckung_promille = l.profil.as_ref().map(|p| p.deckung_promille(&im_ram));
                        if !bel.ebenen_von_platte.is_empty() {
                            bel.hinweise.push(format!(
                                "{} dichte Ebenen passen nicht in den Arbeitsspeicher und kommen bei jedem Token von der \
                                 Platte; das Modell läuft, aber langsam.",
                                bel.ebenen_von_platte.len()
                            ));
                        }
                        if bel.experten_von_platte > 0 {
                            bel.hinweise.push(format!(
                                "{} von {alle} gerouteten Experten kommen von der Platte, über den Dateicache.",
                                bel.experten_von_platte
                            ));
                        }
                        if let (Some(d), Some(p)) = (bel.deckung_promille, &l.profil) {
                            bel.hinweise.push(format!(
                                "Nach dem Profil ({} Token) fallen {},{} % der Expertenaufrufe auf Experten im \
                                 Arbeitsspeicher; {} davon sind die häufigen, vor dem restlichen Arbeitskontext.",
                                p.token,
                                d / 10,
                                d % 10,
                                bel.experten_heiss
                            ));
                        }
                    }
                    Gewichte::Fremd { bytes } => {
                        bel.gewichte_im_ram = *bytes;
                        bel.hinweise.push("Die fremde Laufzeit wählt ihr Gerät und ihre Fäden selbst.".into());
                    }
                }
                Ergebnis::Angenommen(bel)
            }
        };
        zuteilungen.push(Zuteilung { name: l.name.clone(), vorrang: l.vorrang, ergebnis });
    }
    Plan { zuteilungen, verteilbar_bytes: verteilbar, rest_bytes: frei }
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::local::bedarf::{Ebene, Ebenenart};

    const MIB: u64 = 1024 * 1024;

    /// Ein Gemisch: zwei Ebenen, je 100 MiB dicht und vier Experten zu 50 MiB,
    /// Rand 20 MiB, 1 KiB KV je Token, Zustand 10 MiB. Mit 30 MiB Zuschlag
    /// und 4096 Token Mindestkontext ist der feste Teil 44 MiB, mit den
    /// dichten Gewichten 264 MiB, mit 65536 Token Arbeitskontext 324 MiB.
    fn gemisch() -> Modellbedarf {
        Modellbedarf {
            name: "gemisch".into(),
            rand_bytes: 20 * MIB,
            rand_selten_bytes: 0,
            ebenen: (0..2)
                .map(|i| Ebene { index: i, art: Ebenenart::Achtsam, dicht_bytes: 100 * MIB, experten_bytes: vec![50 * MIB; 4] })
                .collect(),
            kv_je_token: 1024,
            zustand_je_folge: 10 * MIB,
            hoechstkontext: 1 << 20,
        }
    }

    fn agent_mit(soll: u64) -> Last {
        Last {
            name: "agent".into(),
            vorrang: Vorrang::Agent,
            gewichte: Gewichte::Eigen(gemisch()),
            zuschlag_bytes: 30 * MIB,
            kontext_mindestens: 4096,
            kontext_soll: soll,
            kerne_mindestens: 1,
            profil: None,
        }
    }

    fn agent() -> Last {
        agent_mit(65536)
    }

    fn fremd(name: &str, v: Vorrang, bytes: u64) -> Last {
        Last {
            name: name.into(),
            vorrang: v,
            gewichte: Gewichte::Fremd { bytes },
            zuschlag_bytes: 10 * MIB,
            kontext_mindestens: 0,
            kontext_soll: 0,
            kerne_mindestens: 1,
            profil: None,
        }
    }

    fn angebot(ram: u64) -> Angebot {
        Angebot { ram_bytes: ram, kerne: 8, karten: Vec::new() }
    }

    fn ohne_ruecklage() -> Regeln {
        Regeln { ruecklage_bytes: 0, kontext_raster: 256 }
    }

    fn abgelehnt(p: &Plan, name: &str) -> String {
        match &p.zeile(name).expect("Zeile").ergebnis {
            Ergebnis::Abgelehnt { grund } => grund.clone(),
            anders => panic!("{name} ist nicht abgelehnt: {anders:?}"),
        }
    }

    /// ⚑ **Alles passt**: alle Gewichte im Speicher, der volle
    /// Arbeitskontext, und die Summe stimmt aufs Byte.
    #[test]
    fn wenn_alles_passt_liegt_alles_im_speicher() {
        let p = belegen(&angebot(10 * GIB), &[agent()], &ohne_ruecklage());
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!((b.gewichte_von_platte, b.experten_von_platte, b.kontext), (0, 0, 65536));
        let erwartet = 65536 * 1024 + 10 * MIB + 30 * MIB + gemisch().gewichte_bytes();
        assert_eq!(b.ram_bytes, erwartet);
        assert_eq!(p.rest_bytes, 10 * GIB - erwartet, "was verteilt ist und was frei bleibt, ergibt das Angebot");
        assert_eq!(b.kerne, 8, "die erste eigene Last bekommt alle freien Kerne");
        assert!(b.hinweise.is_empty(), "{:?}", b.hinweise);
    }

    /// ⚑ **Experten kommen nach dem Arbeitskontext**, und einer, der nicht
    /// passt, kommt von der Platte.
    #[test]
    fn experten_kommen_nach_dem_arbeitskontext() {
        // 324 MiB fuer die Last selbst, 50 MiB bleiben: genau ein Experte.
        let p = belegen(&angebot(374 * MIB), &[agent()], &ohne_ruecklage());
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!(b.kontext, 65536, "der Arbeitskontext kommt vor den Experten");
        assert_eq!(b.experten_von_platte, 7);
        assert_eq!(b.gewichte_von_platte, 7 * 50 * MIB);
        assert_eq!(p.rest_bytes, 0);
        assert!(b.hinweise.iter().any(|h| h.contains("7 von 8")), "{:?}", b.hinweise);
    }

    /// ⚑ **Ein Arbeitskontext, der nicht passt, wird gekuerzt**, auf das
    /// Raster, nie unter das Minimum und nie ueber die Einstellung, und der
    /// Plan sagt es.
    #[test]
    fn der_arbeitskontext_wird_gekuerzt_und_folgt_dem_raster() {
        // 264 MiB fuer feste Teile und dichte Gewichte, dazu 1000 KiB: 1000
        // Token mehr, abgerundet von 5096 auf 4864.
        let p = belegen(&angebot(264 * MIB + 1000 * 1024), &[agent()], &ohne_ruecklage());
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!(b.kontext, 4864);
        assert!(b.hinweise.iter().any(|h| h.contains("Kontext 4864 statt eingestellt 65536")), "{:?}", b.hinweise);
        // Genau die Einstellung, wenn sie passt, auch neben dem Raster; nie mehr.
        let p = belegen(&angebot(10 * GIB), &[agent_mit(5000)], &ohne_ruecklage());
        assert_eq!(p.belegung("agent").map(|b| b.kontext), Some(5000));
        // Eine Einstellung unter dem Minimum gilt als Minimum.
        let p = belegen(&angebot(10 * GIB), &[agent_mit(10)], &ohne_ruecklage());
        assert_eq!(p.belegung("agent").map(|b| b.kontext), Some(4096));
    }

    /// ⚑ **Netz vor Mensch** (P8, Festlegung E7): Der Shard des Netzes
    /// kommt vor dem Agenten, auch wenn er zuletzt genannt ist; reicht der
    /// Rest dem Agenten nicht, sagt die Ablehnung, wer den Platz hat und wie
    /// man ihn zurückbekommt. Ohne Netzlast fehlt der Zusatz.
    #[test]
    fn das_netz_kommt_vor_dem_agenten_und_sagt_es() {
        let netz = Last {
            name: "Netz: gemisch, Ebene 1".into(),
            vorrang: Vorrang::Netz,
            gewichte: Gewichte::Eigen(gemisch().bereich(1, 2, false, true).expect("Bereich")),
            ..agent()
        };
        // Der Bereich: eine Ebene (100 MiB dicht, 200 MiB Experten), der Rand
        // mit Kopf, die Hälfte des KV-Speichers.
        let b = gemisch().bereich(1, 2, false, true).expect("Bereich");
        assert_eq!((b.ebenen.len(), b.rand_bytes, b.kv_je_token), (1, 20 * MIB, 512));
        assert!(gemisch().bereich(1, 3, false, true).is_err(), "über das Modell hinaus");
        // Das Minimum des Netzes (30 MiB Zuschlag, 2 MiB KV) passt in 60
        // MiB, das des Agenten (44 MiB) danach nicht mehr.
        let p = belegen(&angebot(60 * MIB), &[agent(), netz.clone()], &ohne_ruecklage());
        assert!(p.belegung("Netz: gemisch, Ebene 1").is_some(), "das Netz kommt zuerst");
        let grund = abgelehnt(&p, "agent");
        assert!(grund.contains("Das Netz hat Vorrang (Netz: gemisch, Ebene 1)"), "{grund}");
        assert!(grund.contains("für die nächste Epoche weniger zu"), "{grund}");
        let ohne = belegen(&angebot(40 * MIB), &[agent()], &ohne_ruecklage());
        assert!(!abgelehnt(&ohne, "agent").contains("Netz"), "ohne Netzlast kein Zusatz");
    }

    /// ⚑ **Vorrang: Agent vor Vision vor Sprache**, auch wenn die Eingabe
    /// anders sortiert ist; die niedrigste Last weicht.
    #[test]
    fn der_vorrang_entscheidet_und_nicht_die_reihenfolge() {
        // Agent 324, Vision 210, Sprache 110: 644 MiB, da sind 600.
        let lasten = [fremd("sprache", Vorrang::Sprache, 100 * MIB), fremd("vision", Vorrang::Vision, 200 * MIB), agent()];
        let p = belegen(&angebot(600 * MIB), &lasten, &ohne_ruecklage());
        let namen: Vec<&str> = p.zuteilungen.iter().map(|z| z.name.as_str()).collect();
        assert_eq!(namen, ["agent", "vision", "sprache"]);
        assert!(p.belegung("vision").is_some());
        assert!(abgelehnt(&p, "sprache").contains("Braucht mindestens 0,1 GiB"));
        // Die 66 MiB, die Sprache nicht reichten, gehen an einen Experten.
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!((b.kontext, b.experten_von_platte), (65536, 7));
        assert_eq!(p.rest_bytes, 16 * MIB);
    }

    /// ⚑ **Vision geht vor den Experten des Agenten, aber nicht vor seinem
    /// Arbeitskontext**: Experten sind Tempo, der Kontext ist die Einstellung.
    #[test]
    fn vision_geht_vor_den_experten_aber_nicht_vor_dem_arbeitskontext() {
        let lasten = [agent(), fremd("vision", Vorrang::Vision, 200 * MIB)];
        let p = belegen(&angebot(534 * MIB), &lasten, &ohne_ruecklage());
        assert!(p.belegung("vision").is_some(), "Vision passt neben den Agenten ohne Experten");
        assert_eq!(p.belegung("agent").map(|b| b.experten_von_platte), Some(8));
        let p = belegen(&angebot(500 * MIB), &lasten, &ohne_ruecklage());
        assert!(p.belegung("vision").is_none(), "Vision ging vor den Arbeitskontext des Agenten");
        assert_eq!(p.belegung("agent").map(|b| b.kontext), Some(65536));
    }

    /// Ein Profil fuer das Gemisch: Experte 3 der Ebene 1 traegt 70 von 100
    /// Aufrufen, Experte 2 der Ebene 0 weitere 20; zusammen sind sie die
    /// heissen (90 % >= 80 %). Danach folgen (0, 0) mit 4 und (0, 1) mit 3.
    fn mit_profil(mut l: Last) -> Last {
        let mut p = Expertenprofil { modell: "gemisch".into(), token: 13, ..Default::default() };
        for (e, x, n) in [(1, 3, 70), (0, 2, 20), (0, 0, 4), (0, 1, 3), (0, 3, 1), (1, 0, 1), (1, 1, 1)] {
            for _ in 0..n {
                p.zaehlen(e, x);
            }
        }
        l.profil = Some(p);
        l
    }

    /// ⚑ **Die haeufigen Experten kommen direkt nach den dichten Gewichten**
    /// und einem Arbeitskontext bis [`KONTEXT_VOR_EXPERTEN`], vor dem Rest
    /// des Arbeitskontexts; der weicht, und der Plan sagt es. Ohne Profil
    /// bleibt es beim Kontext zuerst.
    ///
    /// 📌 **Bis zum 2026-10-18 (Fund 562) kamen die Experten vor jedem Token
    /// ueber dem Minimum**, und hier stand `(2, 100 MiB, 6, 4096)`: zwei
    /// Experten, Kontext 4096. Jetzt kommen erst 32 768 Token (28 MiB bei
    /// 1 KiB je Token), dann passt einer der beiden Experten (50 MiB), und
    /// der Rest (22 MiB) geht wieder in den Kontext.
    #[test]
    fn die_haeufigen_experten_kommen_vor_dem_arbeitskontext() {
        // 264 MiB fuer feste Teile und dichte Gewichte, 100 fuer Kontext und Experten.
        let p = belegen(&angebot(364 * MIB), &[mit_profil(agent())], &ohne_ruecklage());
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!((b.experten_heiss, b.heiss_bytes, b.experten_von_platte, b.kontext), (1, 50 * MIB, 7, 55296));
        assert!(b.kontext >= KONTEXT_VOR_EXPERTEN, "der Agent bekam weniger als seinen Kontext vor den Experten");
        assert_eq!(b.deckung_promille, Some(700));
        assert!(b.hinweise.iter().any(|h| h.contains("neben den häufigen Experten")), "{:?}", b.hinweise);
        assert!(b.hinweise.iter().any(|h| h.contains("13 Token") && h.contains("70,0 %")), "{:?}", b.hinweise);
        // Ohne Profil: der volle Kontext (60 MiB), und fuer keinen Experten Platz.
        let p = belegen(&angebot(364 * MIB), &[agent()], &ohne_ruecklage());
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!((b.experten_heiss, b.experten_von_platte, b.kontext, b.deckung_promille), (0, 8, 65536, None));
    }

    /// ⚑ **Reicht der Speicher nicht fuer den Kontext vor den Experten**,
    /// bekommt der Agent, was da ist, und kein Experte verdraengt ihn; mit
    /// genug Speicher beides. Fund 562.
    #[test]
    fn der_kontext_vor_den_experten_geht_vor_und_nur_bis_dahin() {
        // 264 MiB fest und dicht, dazu 20 MiB: Kontext 4096 + 20 480 Token, kein Experte.
        let p = belegen(&angebot(284 * MIB), &[mit_profil(agent())], &ohne_ruecklage());
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!((b.experten_heiss, b.kontext), (0, 4096 + 20 * 1024));
        // 28 MiB: genau bis 32 768, kein Experte.
        let p = belegen(&angebot(292 * MIB), &[mit_profil(agent())], &ohne_ruecklage());
        assert_eq!(p.belegung("agent").map(|b| (b.experten_heiss, b.kontext)), Some((0, KONTEXT_VOR_EXPERTEN)));
        // 28 + 50 MiB: der erste Experte, und der Kontext bleibt bei 32 768.
        let p = belegen(&angebot(342 * MIB), &[mit_profil(agent())], &ohne_ruecklage());
        assert_eq!(p.belegung("agent").map(|b| (b.experten_heiss, b.kontext)), Some((1, KONTEXT_VOR_EXPERTEN)));
    }

    /// ⚑ **Ein Aufruf von Sehen leiht sich die haeufigen Experten**, den
    /// seltensten zuerst, aber nie den Arbeitskontext; und der Rest der
    /// Experten kommt nach Sehen, die haeufigsten zuerst.
    #[test]
    fn sehen_leiht_sich_die_haeufigen_experten_und_der_rest_folgt_der_haeufigkeit() {
        let lasten = [mit_profil(agent_mit(4096)), fremd("vision", Vorrang::Vision, 200 * MIB)];
        // Agent 364 (davon 100 heisse Experten), Vision 210.
        let p = belegen(&angebot(574 * MIB), &lasten, &ohne_ruecklage());
        assert!(p.belegung("vision").is_some());
        assert_eq!(p.belegung("agent").map(|b| b.experten_heiss), Some(2));
        // Ein MiB weniger: Sehen leiht sich den selteneren der beiden.
        let p = belegen(&angebot(573 * MIB), &lasten, &ohne_ruecklage());
        assert!(p.belegung("vision").is_some(), "Sehen wurde abgelehnt, statt zu leihen");
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!((b.experten_heiss, b.heiss_bytes, b.deckung_promille), (1, 50 * MIB, Some(700)));
        assert!(b.hinweise.iter().any(|h| h.contains("Während vision läuft, leiht es sich 0,0 GiB")), "{:?}", b.hinweise);
        // Ganz geliehen bei 474, abgelehnt darunter; der Kontext bleibt.
        let p = belegen(&angebot(474 * MIB), &lasten, &ohne_ruecklage());
        assert_eq!(p.belegung("agent").map(|b| (b.experten_heiss, b.kontext)), Some((0, 4096)));
        assert!(p.belegung("vision").is_some());
        let p = belegen(&angebot(473 * MIB), &lasten, &ohne_ruecklage());
        assert!(abgelehnt(&p, "vision").contains("leihbar aus deren häufigen Experten 0,1 GiB"));
        assert_eq!(p.belegung("agent").map(|b| b.experten_heiss), Some(2), "eine Ablehnung leiht nichts");
        // Genug fuer zwei Experten mehr: die naechsthaeufigen (4 und 3 Aufrufe).
        let p = belegen(&angebot(674 * MIB), &lasten, &ohne_ruecklage());
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!((b.experten_heiss, b.experten_von_platte, b.deckung_promille), (2, 4, Some(970)));
        // Alles passt: alles im Speicher, volle Deckung.
        let p = belegen(&angebot(10 * GIB), &lasten, &ohne_ruecklage());
        assert_eq!(p.belegung("agent").and_then(|b| b.deckung_promille), Some(1000));
    }

    /// ⚑ **Dichte Gewichte, die nicht passen, lehnen nicht ab**: Das Modell
    /// laeuft von der Platte, und der Plan sagt es. Abgelehnt wird erst,
    /// wenn der feste Teil nicht passt.
    #[test]
    fn ohne_platz_fuer_die_gewichte_laeuft_es_von_der_platte() {
        // 200 MiB: fest 44, Rand 20, erste Ebene 100; fuer die zweite bleiben 36.
        let p = belegen(&angebot(200 * MIB), &[agent_mit(4096)], &ohne_ruecklage());
        let b = p.belegung("agent").expect("der feste Teil passt");
        assert_eq!(b.ebenen_von_platte, vec![1], "Rand und eine Ebene passen, die zweite nicht");
        assert_eq!(b.gewichte_von_platte, 100 * MIB + 8 * 50 * MIB);
        assert!(b.hinweise.iter().any(|h| h.contains("langsam")), "{:?}", b.hinweise);
        let p = belegen(&angebot(40 * MIB), &[agent()], &ohne_ruecklage());
        assert!(abgelehnt(&p, "agent").contains("Braucht mindestens"), "der feste Teil (44 MiB) passt nicht");
    }

    /// ⚑ **Die Ruecklage bleibt frei**, und ohne Angebot wird nichts
    /// angenommen.
    #[test]
    fn die_ruecklage_bleibt_frei() {
        let r = Regeln { ruecklage_bytes: GIB, kontext_raster: 256 };
        let p = belegen(&angebot(GIB + 264 * MIB), &[agent_mit(4096)], &r);
        assert_eq!(p.verteilbar_bytes, 264 * MIB);
        assert_eq!(p.belegung("agent").map(|b| b.ram_bytes), Some(264 * MIB));
        assert_eq!(p.rest_bytes, 0);
        let p = belegen(&angebot(GIB / 2), &[agent()], &r);
        assert!(p.belegung("agent").is_none());
    }

    /// ⚑ **Kerne**: jede angenommene Last ihr Minimum, die erste eigene den
    /// Rest; zu wenige Kerne lehnen die niedrigere Last ab.
    #[test]
    fn kerne_folgen_dem_vorrang() {
        let mut v = fremd("vision", Vorrang::Vision, MIB);
        v.kerne_mindestens = 3;
        let p = belegen(&angebot(10 * GIB), &[agent(), v.clone()], &ohne_ruecklage());
        assert_eq!(p.belegung("agent").map(|b| b.kerne), Some(5));
        assert_eq!(p.belegung("vision").map(|b| b.kerne), Some(3));
        v.kerne_mindestens = 8;
        let p = belegen(&angebot(10 * GIB), &[agent(), v], &ohne_ruecklage());
        assert!(abgelehnt(&p, "vision").contains("Kerne"), "Vision bekommt Kerne, die der Agent schon hat");
    }

    fn karte(kennung: &str, gemeinsam: bool) -> Karte {
        Karte {
            kennung: kennung.into(),
            name: "Karte".into(),
            rueckseite: Some(if gemeinsam { Rueckseite::Metal } else { Rueckseite::Cuda }),
            frei_bytes: (!gemeinsam).then_some(GIB),
            gemeinsamer_speicher: gemeinsam,
        }
    }

    /// ⚑ **Die Karten gehen an die erste angenommene eigene Last**, fremde
    /// bekommen keine, und der Hinweis unterscheidet Steckkarte und
    /// gemeinsamen Speicher.
    #[test]
    fn die_karten_gehen_an_den_agenten() {
        let mut a = angebot(10 * GIB);
        a.karten = vec![karte("k0", false)];
        let p = belegen(&a, &[fremd("vision", Vorrang::Vision, MIB), agent()], &ohne_ruecklage());
        let b = p.belegung("agent").expect("angenommen");
        assert_eq!(b.karten, vec!["k0".to_string()]);
        assert!(b.hinweise.iter().any(|h| h.contains("je Aufruf")), "{:?}", b.hinweise);
        assert!(!b.hinweise.iter().any(|h| h.contains("ohne Kopie")), "{:?}", b.hinweise);
        assert_eq!(p.belegung("vision").map(|b| b.karten.len()), Some(0));
        a.karten = vec![karte("m", true)];
        let p = belegen(&a, &[agent()], &ohne_ruecklage());
        let h = &p.belegung("agent").expect("angenommen").hinweise;
        assert!(h.iter().any(|h| h.contains("ohne Kopie")) && !h.iter().any(|h| h.contains("je Aufruf")), "{h:?}");
    }

    /// ⚑ **Ein Mindestkontext ueber dem Hoechstkontext** wird abgelehnt und
    /// nicht still gekuerzt.
    #[test]
    fn ein_unmoeglicher_kontext_wird_abgelehnt() {
        let mut a = agent();
        a.kontext_mindestens = (1 << 20) + 1;
        let p = belegen(&angebot(10 * GIB), &[a], &ohne_ruecklage());
        assert!(abgelehnt(&p, "agent").contains("Höchstkontext"));
    }

    /// ⚑ **Deterministisch**: zweimal derselbe Plan, und nie mehr verteilt,
    /// als da war, ueber viele Groessen hinweg.
    #[test]
    fn nie_mehr_verteilt_als_da_ist() {
        let lasten = [agent(), fremd("vision", Vorrang::Vision, 200 * MIB), fremd("sprache", Vorrang::Sprache, 90 * MIB)];
        for mib in (0..2000).step_by(7) {
            let a = angebot(mib * MIB);
            let p = belegen(&a, &lasten, &ohne_ruecklage());
            assert_eq!(p, belegen(&a, &lasten, &ohne_ruecklage()));
            let belegt: u64 = p
                .zuteilungen
                .iter()
                .filter_map(|z| match &z.ergebnis {
                    Ergebnis::Angenommen(b) => Some(b.ram_bytes),
                    _ => None,
                })
                .sum();
            assert_eq!(belegt + p.rest_bytes, p.verteilbar_bytes, "bei {mib} MiB");
            // Eine angenommene niedrigere Last heisst: Die hoehere ist auch angenommen.
            let an = |n: &str| p.belegung(n).is_some();
            assert!(!an("vision") || an("agent") || mib < 44, "bei {mib} MiB");
        }
    }

    /// ⚑ **Der Bericht nennt jede Last mit ihrem Ergebnis**, abgelehnte mit
    /// Grund, und endet mit dem Rest.
    #[test]
    fn der_bericht_nennt_jede_last() {
        let lasten = [agent(), fremd("vision", Vorrang::Vision, 200 * MIB), fremd("sprache", Vorrang::Sprache, 100 * MIB)];
        let a = angebot(600 * MIB);
        let r = ohne_ruecklage();
        let text = bericht(&a, &lasten, &r, &belegen(&a, &lasten, &r));
        let zeilen: Vec<&str> = text.lines().collect();
        assert!(zeilen[0].starts_with("Angebot: 0,6 GiB"), "{text}");
        assert!(zeilen[1].starts_with("Agent    agent:") && zeilen[1].contains("Kontext 65536"), "{text}");
        assert!(text.contains("Vision   vision: 0,2 GiB im Speicher"), "{text}");
        assert!(text.contains("Sprache  sprache: abgelehnt. Braucht"), "{text}");
        assert!(zeilen.last().is_some_and(|z| z.starts_with("Frei danach:")), "{text}");
    }

    /// ⚑ **Jedes Fenster muss passen, nicht der Durchschnitt**, und der
    /// Kontext zaehlt mit.
    #[test]
    fn die_ebenen_einer_karte_zaehlen_das_schwerste_fenster() {
        // Je Ebene 100 dicht und 200 Experten: 300 MiB, dazu je Token 512 Byte KV.
        let b = gemisch();
        assert_eq!(ebenen_hoechstens(&b, 600 * MIB, 0), 2);
        assert_eq!(ebenen_hoechstens(&b, 600 * MIB - 1, 0), 1);
        assert_eq!(ebenen_hoechstens(&b, 299 * MIB, 0), 0);
        // 2048 Token: je Ebene 1 MiB KV mehr.
        assert_eq!(ebenen_hoechstens(&b, 601 * MIB, 2048), 1);
        assert_eq!(ebenen_hoechstens(&b, 602 * MIB, 2048), 2);
        // Ungleich schwere Ebenen: [1, 5, 1, 1]. Zwei passen in 6 (jedes Paar
        // hoechstens 6), drei nicht (1 + 5 + 1 = 7).
        let mut u = gemisch();
        u.kv_je_token = 0;
        u.ebenen = [1u64, 5, 1, 1]
            .iter()
            .enumerate()
            .map(|(i, m)| Ebene { index: i, art: Ebenenart::Achtsam, dicht_bytes: m * MIB, experten_bytes: vec![] })
            .collect();
        assert_eq!(ebenen_hoechstens(&u, 6 * MIB, 0), 2);
        // [1, 1, 3, 1] in 3: Das erste Paar (2) passt, das zweite (4) nicht.
        // Wer nur vorne nachsaehe, kaeme auf 2.
        for (e, m) in u.ebenen.iter_mut().zip([1u64, 1, 3, 1]) {
            e.dicht_bytes = m * MIB;
        }
        assert_eq!(ebenen_hoechstens(&u, 3 * MIB, 0), 1);
        // Der Bericht nennt es je Steckkarte.
        let mut a = angebot(10 * GIB);
        a.karten = vec![karte("k0", false), karte("m", true)];
        let r = ohne_ruecklage();
        let lasten = [agent()];
        let text = bericht(&a, &lasten, &r, &belegen(&a, &lasten, &r));
        assert!(text.contains("Karte    k0: 1,0 GiB frei; dauerhaft passten 2 von 2 Ebenen"), "{text}");
        assert!(!text.contains("Karte    m:"), "eine GPU mit gemeinsamem Speicher hat kein eigenes Freies: {text}");
    }

    /// ⚑ **Fluessig heisst: alles ausser den Experten**, und es ist genau das,
    /// was der Plan als Minimum mit dichten Gewichten haelt.
    #[test]
    fn fluessig_ist_das_minimum_ohne_experten() {
        let b = gemisch();
        // 4 MiB KV, 10 Zustand, 30 Zuschlag, 220 dicht.
        assert_eq!(fluessig_bytes(&b, 4096, 30 * MIB), Some(264 * MIB));
        let p = belegen(&angebot(264 * MIB), &[agent_mit(4096)], &ohne_ruecklage());
        let bel = p.belegung("agent").expect("angenommen");
        assert_eq!((bel.ram_bytes, bel.ebenen_von_platte.len()), (264 * MIB, 0));
        assert_eq!(fluessig_bytes(&b, u64::MAX, 0), None, "ein Ueberlauf ist kein Bedarf");
    }

    /// ⚑ **Eine Freigabe gilt ganz, wenn genug frei ist**; die Ruecklage
    /// greift nur, wo das Freie knapper ist.
    #[test]
    fn die_freigabe_gilt_ganz_wenn_genug_frei_ist() {
        let r = Regeln { ruecklage_bytes: GIB, kontext_raster: 256 };
        let verteilbar = |frei, freigabe| belegen(&angebot(ram_angebot(frei, freigabe, GIB)), &[], &r).verteilbar_bytes;
        assert_eq!(verteilbar(20 * GIB, Some(4 * GIB)), 4 * GIB);
        assert_eq!(verteilbar(4 * GIB, Some(4 * GIB)), 3 * GIB, "knapp: die Ruecklage bleibt");
        assert_eq!(verteilbar(20 * GIB, None), 19 * GIB);
    }

    #[test]
    fn gib_rundet_ganzzahlig() {
        assert_eq!(gib(0), "0,0 GiB");
        assert_eq!(gib(GIB), "1,0 GiB");
        // `GIB / 20` schnitte ab und laege knapp unter 1,05; ein Zehntel liegt klar darueber.
        assert_eq!(gib(GIB + GIB / 20 - 1), "1,0 GiB");
        assert_eq!(gib(GIB + GIB / 10), "1,1 GiB");
        assert_eq!(gib(17 * GIB / 10), "1,7 GiB");
    }
}
