//! Was diese Maschine hergibt, und was ihr Besitzer davon freigibt.
//!
//! # ⚑ Warum das eine Erklaerung ist und keine Einstellung
//!
//! **Ein Schieber, der nur oertlich etwas abschaltet, ist eine
//! Einstellung. Einer, der dem Netz etwas zusagt, ist ein Versprechen**,
//! und aus der Summe dieser Versprechen leitet das Netz spaeter sein
//! Budget ab. Deshalb steht hier nicht „wieviel darf der Rechenpfad
//! nehmen", sondern „wieviel gibt der Besitzer her": Die zweite Frage
//! traegt beide Haelften, die erste nur die oertliche.
//!
//! ⚑ **Und deshalb ist die Form schon die der Erklaerung.** Je
//! Betriebsmittel ein Regler mit Hoechstwert, freigegebenem Wert und
//! Einheit. Was heute oertlich wirkt, ist eine Teilmenge davon; was
//! noch nicht wirkt, sagt in [`Regler::sperrgrund`] **warum**, und zwar
//! mit dem, was dafuer geschrieben werden muss.
//!
//! # ⚑ Die Erhebung selbst steht im Dispatcher
//!
//! Was die Maschine hat und was davon frei ist, erhebt seit dem 2026-10-15
//! `myl-dispatcher` (`local::erhebung`), denn auch der Knoten und das
//! Messwerkzeug brauchen sie. Hier stehen weiter die Regler der
//! Freigabemaske und ihr Umsetzen; die Typen werden unter ihren alten
//! Namen weitergereicht, damit kein Aufrufer etwas merkt.

pub use myl_dispatcher::local::erhebung::{Hardware, Platte, Rechenwerk, Rueckseite, GIB};

/// **Die Rechenwerke dieser Maschine, einmal je Prozess gesucht.**
///
/// 📌 **Warum gemerkt wird (Meldung des Projektinhabers, 2026-10-17):**
/// Start und Einstellungsseite des Fensters dauerten lange, und die
/// Startanimation stand still. Gemessen: Der Befehl `einstellungen` lief
/// bei jedem Aufruf durch den ganzen Hardwarescan, rund 200 ms auf dem
/// Hauptfaden (davon 135 ms `system_profiler`), und das Fenster ruft ihn an
/// mehr als zehn Stellen, unter Speicherdruck jedesmal laenger. Welche
/// Karten im Rechner stecken, aendert sich waehrend eines Programmlaufs
/// nicht oft; wer eine Karte im Betrieb ansteckt, sieht sie, sobald eine
/// Seite [`rechenwerke_neu_suchen`] ruft (der Reiter „Knoten" und `/mining`
/// beim Oeffnen).
///
/// ⚑ **Die Sperre haelt waehrend der ersten Suche**, wie vorher die
/// `OnceLock`: Wer gleichzeitig fragt, wartet auf dieses eine Ergebnis,
/// statt einen zweiten Unterprozess zu starten.
pub fn rechenwerke_gemerkt() -> Vec<Rechenwerk> {
    let mut g = GEMERKT.lock().unwrap_or_else(|v| v.into_inner());
    g.get_or_insert_with(Hardware::rechenwerke).clone()
}

static GEMERKT: std::sync::Mutex<Option<Vec<Rechenwerk>>> = std::sync::Mutex::new(None);

/// **Sucht die Rechenwerke neu** und merkt sich das Ergebnis. Gibt `true`,
/// wenn sich die Menge der Karten geaendert hat (eine dazu, eine weg).
///
/// ⚑ **Gesucht wird ausserhalb der Sperre**: Waehrend der Unterprozess
/// laeuft, bekommt jeder andere Aufrufer die alte Liste sofort.
/// ⚑ **Verglichen werden die Kennungen**, nicht ganze Eintraege: Das Freie
/// einer Karte aendert sich bei jeder Suche, und eine Seite, die deshalb
/// neu zeichnete, zeichnete immer neu.
///
/// 📌 Gewuenscht vom Projektinhaber (2026-10-17): Eine eGPU, die im
/// Betrieb angesteckt wird, soll im Reiter erscheinen, ohne das Programm
/// neu zu starten. Bis dahin sah sie erst der naechste Start.
pub fn rechenwerke_neu_suchen() -> bool {
    let neu = Hardware::rechenwerke();
    let kennungen = |w: &[Rechenwerk]| w.iter().map(|w| w.kennung.clone()).collect::<Vec<_>>();
    let mut g = GEMERKT.lock().unwrap_or_else(|v| v.into_inner());
    let anders = g.as_deref().is_none_or(|alt| kennungen(alt) != kennungen(&neu));
    *g = Some(neu);
    anders
}

/// **Wie `Hardware::erheben`, mit den gemerkten Rechenwerken**: Speicher
/// und Platte sind frisch, die Suche nach Karten geschieht einmal.
pub fn erheben(datenort: &std::path::Path) -> Hardware {
    Hardware::erheben_mit(datenort, rechenwerke_gemerkt())
}

/// **Sucht die Rechenwerke im Hintergrund**, damit die erste Seite, die sie
/// braucht, nicht wartet. Fuer den Start einer Oberflaeche.
pub fn vorwaermen() {
    std::thread::spawn(|| {
        rechenwerke_gemerkt();
    });
}

/// **Wo die Messungen dieser Maschine liegen**: neben der Einstellungsdatei.
///
/// ⚑ **Dort und nicht im Datenort**: Der liegt in einem Klon unter
/// `INTEGER_LLM/`, und eine Messung dieser Maschine ist so wenig Teil des
/// Repositoriums wie ihre Einstellungen. Eine Probe, die die Einstellungen
/// abschirmt (`MYL_EINSTELLUNGEN`), schirmt damit auch das Buch ab.
pub fn eichbuch_pfad() -> std::path::PathBuf {
    crate::einstellungen::Einstellungen::vorgabepfad().with_file_name("eichbuch.json")
}

/// **Das Eichbuch dieser Maschine**; fehlt oder klemmt es, ein leeres, und
/// dann gelten die Vorgaben.
///
/// ⚠️ Ein kaputtes Buch ist hier kein Grund, nicht zu laden oder nicht zu
/// sehen: Es kostet nur Genauigkeit. `myl eichen` meldet es, wenn es
/// schreiben will.
pub fn eichbuch() -> myl_dispatcher::local::eichung::Eichbuch {
    myl_dispatcher::local::eichung::Eichbuch::lesen(&eichbuch_pfad()).unwrap_or_default()
}

/// **Wo das Expertenprofil eines Modells liegt**: neben dem Eichbuch, je
/// Modell eine Datei.
///
/// ⚑ Aus demselben Grund wie das Eichbuch nicht im Datenort: Welche
/// Experten auf dieser Maschine haeufig sind, haengt an der Arbeit ihres
/// Besitzers und nicht am Repositorium.
pub fn profil_pfad(modell: &str) -> std::path::PathBuf {
    eichbuch_pfad().with_file_name(format!("expertenprofil-{modell}.json"))
}

/// **Das Expertenprofil eines Modells**, falls eines da ist und zu ihm
/// gehoert.
///
/// ⚠️ Fehlt es oder klemmt es, kein Profil, und dann bekommt der Plan keine
/// heissen Experten: Es kostet Tempo und nichts sonst. Der Betrieb schreibt
/// es danach neu.
pub fn expertenprofil(modell: &str) -> Option<myl_dispatcher::local::profil::Expertenprofil> {
    myl_dispatcher::local::profil::Expertenprofil::lesen(&profil_pfad(modell))
        .ok()
        .flatten()
        .filter(|p| p.modell == modell && p.token > 0)
}

/// Womit ein Regler misst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Einheit {
    /// Ganze Kerne.
    Kerne,
    /// Gibibyte.
    Gib,
    /// Anteil in Prozent, fuer Groessen ohne verlaessliche Absolutzahl.
    Prozent,
}

impl Einheit {
    /// **Wie ein Wert in dieser Einheit gelesen wird.**
    ///
    /// ⚑ **Hier und nicht in der Konsole.** Sie zeigt dieselben Regler
    /// wie das Fenster, und eine zweite Schreibweise derselben Einheit
    /// waere eine Zeile, die je nach Bedieninstrument anders aussieht.
    ///
    /// ⚠️ **Das Fenster formatiert noch selbst**, in `ui/app.js`: Es
    /// braucht die Schrift **waehrend** des Ziehens, bevor irgendetwas
    /// ueber die Naht gegangen ist. Die beiden Stellen sind damit
    /// bekannt und nicht uebersehen.
    pub fn wie(self, n: u64, sprache: crate::einstellungen::Sprache) -> String {
        match self {
            Self::Kerne if n == 1 => sprache.waehlen("1 Kern", "1 core").to_string(),
            Self::Kerne => format!("{n} {}", sprache.waehlen("Kerne", "cores")),
            Self::Gib => format!("{n} GiB"),
            Self::Prozent => format!("{n} %"),
        }
    }
}

/// Ein Betriebsmittel, das der Besitzer freigeben kann.
///
/// ⚑ **Der Hoechstwert kommt aus dem Scan und nicht aus einer
/// Vorgabe.** Ein Regler, dessen Ende jemand geraten hat, laesst
/// entweder etwas verschenken oder etwas versprechen, das die Maschine
/// nicht hat.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Regler {
    /// Der Name, unter dem die Freigabe abgelegt wird.
    pub name: String,
    pub titel: String,
    pub einheit: Einheit,
    /// Was die Maschine hergibt. `None` heisst: nicht erkannt, und dann
    /// laesst sich nichts freigeben, was ein Anteil von etwas waere.
    pub hoechstens: Option<u64>,
    /// ⚑ **Das linke Ende, und es ist nicht ueberall dasselbe.** Bei
    /// einer Grenze ist es `1`: Null Kerne waeren kein enger gestellter
    /// Klient, sondern ein Stillstand. Bei einem Rechenwerk ist es `0`,
    /// denn „dieses Geraet nicht benutzen" ist eine Einstellung, die
    /// jemand wirklich treffen will.
    pub mindestens: u64,
    /// Was heute freigegeben ist. ⚑ **`None` heisst „ohne Grenze" und
    /// steht ganz rechts**, nicht links und nicht bei null: Wer nichts
    /// eingestellt hat, gibt alles her.
    pub wert: Option<u64>,
    /// Wie die **linke** Endstellung heisst, wenn sie einen eigenen
    /// Namen hat. `None`: dort steht der Wert selbst.
    ///
    /// ⚑ **Die Worte stehen hier und nicht in der Oberflaeche**, in der
    /// eingestellten Sprache. Fenster und Konsole zeigen denselben
    /// Regler; zwei Stellen mit je eigenen Worten laufen auseinander,
    /// und die zweite ist die schlechter gepruefte.
    pub links: Option<String>,
    /// Wie die **rechte** Endstellung heisst. Immer benannt, denn dort
    /// steht nie eine Zahl, sondern „ohne Grenze".
    pub rechts: String,
    /// Was der Regler bewirkt, in einem Satz.
    pub hinweis: String,
    /// ⚑ **Warum er nichts bewirkt, und was dafuer fehlt.** `None`
    /// heisst: Er wirkt. Ein gesperrter Regler ohne diesen Satz waere
    /// eine Zierde, und Zierde in einer Freigabemaske beruhigt ueber
    /// eine Stelle, die nichts leistet.
    pub sperrgrund: Option<String>,
}

/// **Setzt um, was der Nutzer an Kapazitaet freigegeben hat**: die
/// Kerngrenze und die Rechenwerke.
///
/// # ⚑ Eine Stelle, drei Bedieninstrumente
///
/// `myl`, `myelith` und das Fenster geben dieselben Einstellungen frei.
/// **Drei Umsetzungen derselben Freigabe liefen auseinander**, und die
/// Konsole zeigte, wie das aussieht: Sie las `kap.kerne`, zeigte es an
/// und wandte es nie an (Fund 381).
///
/// # ⚑ Der Scan laeuft nur, wenn er etwas aendern kann
///
/// Ein Rechenwerk abzuschalten verlangt, sein Geraet zu kennen, und das
/// kostet auf macOS einen Unterprozess von rund einer Sekunde.
/// **Abgeschaltet wird ein Werk nur durch eine ausdrueckliche Null in
/// der Ablage**; steht dort keine, gibt es nichts abzuschalten und
/// nichts zu scannen. Das ist der Normalfall: Ohne Eintrag ist ein Werk
/// ganz freigegeben.
/// # ⚑ Warum sie etwas zurueckgibt
///
/// **Damit sich pruefen laesst, ob der Durchlauf lief.** Ob ein
/// Rechenwerk wirklich abgeschaltet wurde, sieht man nur auf einer
/// Maschine mit Rechenwerk, und in der CI steckt keine Grafikkarte.
/// Der Rueckgabewert ist die Stelle, an der die **Entscheidung**
/// nachpruefbar wird, und sie ist es, die zweimal falsch sein koennte.
///
/// `true` heisst: Die Rechenwerke wurden durchgegangen.
pub fn anwenden(e: &crate::einstellungen::Einstellungen) -> bool {
    if let Some(n) = e.kapazitaet.kerne {
        crate::kapazitaet::kerne_setzen(n);
    }
    let eine_null = e.kapazitaet.rechenwerke.values().any(|p| *p == 0);
    // ⚑ **Die Abkuerzung gilt nur, solange auch nichts abgeschaltet
    // ist.** Ohne die Marke waere das Zurueckdrehen des Reglers
    // wirkungslos: Wer ein Werk auf null zieht und gleich darauf wieder
    // hinauf, haette keine Null mehr in der Ablage, und der Durchlauf,
    // der das Werk zurueckholt, faende hier sein Ende. **Ein Schalter,
    // der nur in eine Richtung wirkt, ist keiner.**
    if !eine_null && !ABGESCHALTET.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    // ⚑ **Kein Eintrag heisst ganz freigegeben**, wie ueberall sonst:
    // Der Regler steht dann am rechten Anschlag.
    let (karten, wege) = schaltplan(&Hardware::rechenwerke(), |kennung| {
        e.kapazitaet.rechenwerke.get(kennung).is_none_or(|p| *p > 0)
    });
    for (pci, an) in &karten {
        crate::rechenwege::karte_setzen(pci, *an);
    }
    for (weg, an) in wege {
        crate::rechenwege::setzen(weg, an);
    }
    ABGESCHALTET.store(eine_null, std::sync::atomic::Ordering::Relaxed);
    true
}

/// Je Karte die PCI-Adresse und an oder aus; je Rechenweg an oder aus.
pub(crate) type Schaltplan = (Vec<(String, bool)>, Vec<(&'static str, bool)>);

/// **Was [`anwenden`] schaltet**: je Karte mit PCI-Adresse die Karte, je
/// Rechenweg den ganzen Weg.
///
/// 📌 **Fund 534: Vorher wurde der Weg je Karte geschaltet** (2026-10-14
/// gefunden, beim Bau des Wegs fuer mehrere Karten): `setzen("cuda", an)` fuer jede
/// Karte nacheinander, und bei zwei NVIDIA-Karten, von denen eine aus sein
/// sollte, entschied die zuletzt bearbeitete fuer beide. ⚑ Jetzt gilt:
///
/// - Eine Karte mit Adresse wird **einzeln** geschaltet, und ihr Weg
///   bleibt ihretwegen an.
/// - Ein Weg ist an, **wenn irgendeine seiner Karten an ist**; aus nur,
///   wenn alle ohne Adresse aus sind. Das ist bei Metal (eine GPU) genau
///   der Regler, und bei mehreren Karten ohne Adresse (Windows) die
///   einzige Wahl, die keinen Regler ueberstimmt, der Rechnen erlaubt.
pub(crate) fn schaltplan(werke: &[Rechenwerk], an: impl Fn(&str) -> bool) -> Schaltplan {
    let mut karten = Vec::new();
    let mut wege: Vec<(&'static str, bool)> = Vec::new();
    for w in werke {
        let Some(r) = w.rueckseite else { continue };
        let werk_an = an(&w.kennung);
        let weg_an = match (r, &w.pci) {
            (Rueckseite::Cuda | Rueckseite::Rocm, Some(pci)) => {
                karten.push((pci.clone(), werk_an));
                true
            }
            _ => werk_an,
        };
        match wege.iter_mut().find(|(weg, _)| *weg == r.backend()) {
            Some((_, a)) => *a |= weg_an,
            None => wege.push((r.backend(), weg_an)),
        }
    }
    (karten, wege)
}

/// Ob in diesem Prozess schon einmal ein Rechenwerk abgeschaltet wurde.
///
/// ⚑ **Sie steht hier und nicht in der Ablage.** Ein Schalter, den
/// jemand umgelegt hat, gilt fuer diesen Prozess; die Ablage sagt, was
/// eingestellt ist, und das ist dieselbe Trennung wie zwischen
/// gespeichertem und wirksamem Kernbudget.
static ABGESCHALTET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// ── Vorrang ─────────────────────────────────────────────────────────

/// **Stellt den laufenden Faden hinter die Oberflaeche**, fuer Arbeit, auf
/// die niemand wartet: das Vorladen des Modells beim Start.
///
/// 📌 **Warum:** Der Lader liest auf allen Kernen, und in derselben Zeit
/// zeichnet das Fenster sein Vorschaltbild. Mit gleichem Vorrang ruckelte
/// das Bild (Projektinhaber, 2026-09-29). Faeden, die dieser Faden danach
/// startet, erben den Vorrang, also auch die des Laders.
///
/// Unter macOS die Dienstklasse „utility“, unter Linux ein hoeherer
/// Nice-Wert, sonst nichts. Scheitert es, laedt es eben mit vollem Vorrang.
///
/// ⚠️ **Nur in einem eigenen Faden aufrufen**, nie in einem aus einem
/// Vorrat: Der Vorrang bleibt am Faden, und unter Linux laesst er sich ohne
/// Rechte nicht zuruecknehmen.
pub fn im_hintergrund_rechnen() {
    #[cfg(target_os = "macos")]
    // SICHERHEIT: wirkt nur auf den aufrufenden Faden, ohne Zeiger.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
    #[cfg(target_os = "linux")]
    // SICHERHEIT: Unter Linux meint `0` den aufrufenden Faden, ohne Zeiger.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
    }
}

/// ⚑ **Was fehlt, bevor ein Rechenwerk mitrechnen darf.**
///
/// Der Satz steht hier und nicht im Fenster, aus demselben Grund wie
/// die Beschriftungen der Felder: **Ein gesperrter Regler, der nicht
/// sagt warum, ist eine Zierde**, und Zierde in einer Freigabemaske
/// beruhigt ueber eine Stelle, die nichts leistet.
///
/// 📌 **Bis zum 2026-09-16 stand hier ein einziger Satz fuer alle
/// Rechenwerke, und er war seit dem 2026-09-14 falsch** (Fund 380). Er
/// sagte „noch kein Rechenweg ueber dieses Geraet", waehrend `metal`
/// auf Apple-Silizium die gebuendelten Matrizen der Vorbereitung
/// wirklich rechnet. **Die Begruendung aus der Zeit, als es keinen
/// Rechenweg gab, galt fuer ihren Fall und nicht fuer den zweiten.**
/// Deshalb nennt der Satz jetzt den Rechenweg des Geraets, und ob er
/// ueberhaupt faellt, entscheidet [`crate::rechenwege::vorhanden`].
pub(crate) fn gesperrt_weil(r: Option<Rueckseite>, sprache: crate::einstellungen::Sprache) -> String {
    let Some(r) = r else {
        return sprache.waehlen(OHNE_RUECKSEITE.0, OHNE_RUECKSEITE.1).to_string();
    };
    sprache.waehlen_wert(
        format!(
            "Der Rechenweg {} führt über dieses Gerät, und er rechnet in diesem \
             Programm nicht. Die Rückseiten für NVIDIA und AMD reichen heute an die \
             Referenzkernel weiter, gerechnet wird also auf der CPU. Es fehlen echte \
             Ganzzahlkerne, INT8-GEMM über dp4a bei NVIDIA, i8mm bei ARM und VNNI bei \
             x86, und ein Konformitätslauf auf genau dieser Architektur. Ohne ihn würde \
             ein Miner mit abweichendem Kernel bestraft, ohne etwas falsch gemacht zu \
             haben. Sobald beides steht, wirkt dieser Regler ohne weiteres Zutun.",
            r.anzeige()
        ),
        format!(
            "The {} compute path serves this device, and it does not compute in this \
             program. The NVIDIA and AMD back ends currently hand through to the \
             reference kernels, so the work happens on the CPU. What is missing are real \
             integer kernels, INT8 GEMM via dp4a on NVIDIA, i8mm on ARM and VNNI on x86, \
             and a conformance run on exactly this architecture. Without it a miner with \
             a deviating kernel would be penalised without having done anything wrong. \
             Once both exist, this slider takes effect with no further work.",
            r.anzeige()
        ),
    )
}

/// Warum ein Geraet ohne jeden Rechenweg gesperrt ist.
///
/// ⚑ **Das ist etwas anderes als ein Rechenweg, der hier nicht
/// rechnet**, und der Unterschied ist der zwischen „noch nicht gebaut"
/// und „gibt es nicht". Wer den ersten Satz an einer eingebauten
/// Intel-Grafik liest, wartet auf etwas, das niemand vorhat.
pub const OHNE_RUECKSEITE: (&str, &str) = (
    "Für dieses Gerät gibt es in Myelith keinen Rechenweg. Gerechnet wird \
     auf der CPU, und daran ändert dieser Regler nichts.",
    "Myelith has no compute path for this device. The work happens on the \
     CPU, and this slider does not change that.",
);

/// Warum ein Betriebsmittel ohne erkanntes Ende gesperrt ist.
pub const OHNE_ENDE: (&str, &str) = (
    "Nicht erkannt: Diese Maschine sagt nicht, wie viel sie hiervon hat, \
     und ein Regler ohne Ende ist keiner.",
    "Not detected: this machine does not say how much of this it has, \
     and a slider without an end is not a slider.",
);

/// ⚑ **Wie die rechte Endstellung jedes Reglers heisst.**
///
/// **Ganz rechts heisst „ohne Grenze", und dort steht jeder Regler,
/// solange niemand ihn bewegt hat** (Auftrag des Projektinhabers,
/// 2026-09-16). 📌 Bis dahin lag diese Stellung ganz **links**, bei
/// null. Sie bedeutete dasselbe und las sich wie das Gegenteil: Wer
/// einen Regler am linken Anschlag sieht, liest „nichts", nicht
/// „alles".
pub const OHNE_GRENZE: (&str, &str) = ("ohne Grenze", "no limit");

/// Wie die linke Endstellung eines Rechenwerks heisst.
///
/// ⚑ **Bei einem Rechenwerk ist null eine Einstellung und kein
/// Stillstand**, anders als bei den Kernen: Ein Rechner ohne
/// freigegebene GPU rechnet weiter, er rechnet nur auf der CPU.
pub const RECHNET_NICHT: (&str, &str) = ("rechnet nicht", "not used");

/// **Die Regler der Freigabemaske** fuer eine erhobene Maschine.
///
/// ⚑ **Ein Zug und keine Methode am Typ**, weil `Hardware` im Dispatcher
/// steht und die Regler hier an Einstellungen und Sprache haengen. Wer
/// `regler` ruft, bindet diesen Zug ein.
pub trait Freigabemaske {
    /// Die Regler dieser Maschine, mit dem, was heute freigegeben ist.
    fn regler(&self, e: &crate::einstellungen::Einstellungen) -> Vec<Regler>;
}

impl Freigabemaske for Hardware {
    /// Die Regler dieser Maschine, mit dem, was heute freigegeben ist.
    ///
    /// ⚑ **Eine Quelle fuer die ganze Freigabemaske.** Die festen
    /// Betriebsmittel kommen aus [`crate::einstellungen::FELDER`], die
    /// Rechenwerke aus dem Scan, die Werte aus
    /// [`crate::einstellungen::Einstellungen::wert`]. Das Fenster
    /// zaehlt nichts davon selbst auf.
    fn regler(&self, e: &crate::einstellungen::Einstellungen) -> Vec<Regler> {
        use crate::einstellungen::{Feldwert, FELDER, RECHENWERK_PRAEFIX};

        let zahl = |w: Feldwert| match w {
            Feldwert::Zahl(n) => Some(n),
            _ => None,
        };

        // 📌 **In der eingestellten Sprache, seit dem 2026-09-10.** Die
        // Beschriftungen kamen roh aus `FELDER`, also immer auf Deutsch,
        // und ein englisches Fenster trug in der Freigabemaske deutsche
        // Saetze. **Uebersetzt wird an einer Stelle, und diese hier
        // hatte sie nicht gefragt.**
        let sprache = e.oberflaeche.sprache;
        let ohne_grenze = sprache.waehlen(OHNE_GRENZE.0, OHNE_GRENZE.1).to_string();
        // ⚑ **Einmal gefragt, nicht je Geraet.** Welche Rechenwege
        // rechnen, haengt an der Uebersetzung und an der Maschine und
        // nicht am einzelnen Geraet; `metal::verfuegbar` baut beim
        // ersten Ruf ein Geraet auf und prueft es gegen die CPU.
        let rechnende = crate::rechenwege::vorhanden();
        let mut aus = Vec::new();
        for f in FELDER.iter().filter(|f| f.freigabe) {
            let f = f.in_sprache(sprache);
            let (einheit, hoechstens) = masse(self, f.name);
            aus.push(Regler {
                name: f.name.to_string(),
                titel: f.titel.to_string(),
                einheit,
                hoechstens,
                // ⚑ **Eins und nicht null.** Eine Grenze von null Kernen
                // waere kein enger gestellter Klient, sondern einer, der
                // nicht antwortet; wer gar nichts hergeben will,
                // schliesst das Programm.
                mindestens: 1,
                wert: e.wert(f.name).ok().and_then(zahl),
                links: None,
                rechts: ohne_grenze.clone(),
                hinweis: f.hinweis.to_string(),
                // ⚑ **Ein Regler ohne Ende ist keiner.** Was die
                // Maschine nicht nennt, laesst sich nicht anteilig
                // hergeben, und eine geratene Obergrenze liesse den
                // Nutzer etwas zusagen, das es nicht gibt.
                sperrgrund: hoechstens
                    .is_none()
                    .then(|| sprache.waehlen(OHNE_ENDE.0, OHNE_ENDE.1).to_string()),
            });
        }

        for r in &self.rechenwerke {
            // ⚑ **Zwei Fragen, und beide muessen Ja sein.** Fuehrt ueber
            // dieses Geraet ueberhaupt ein Rechenweg, und rechnet der
            // hier? Die erste beantwortet der Scan, die zweite die
            // Kernkiste. **Ein Ja auf die erste allein war bis zum
            // 2026-09-16 der Sperrgrund fuer alle** und traf damit auch
            // das Geraet, das wirklich rechnet.
            let rechnet = r.rueckseite.is_some_and(|b| rechnende.contains(&b.backend()));
            aus.push(Regler {
                name: format!("{RECHENWERK_PRAEFIX}{}", r.kennung),
                titel: r.beschriftung(),
                einheit: Einheit::Prozent,
                hoechstens: Some(100),
                // ⚑ **Null und nicht eins.** „Dieses Geraet nicht
                // benutzen" ist eine Einstellung, die jemand wirklich
                // trifft; der Rechenpfad laeuft dann auf der CPU weiter.
                mindestens: 0,
                wert: e
                    .wert(&format!("{RECHENWERK_PRAEFIX}{}", r.kennung))
                    .ok()
                    .and_then(zahl),
                links: Some(sprache.waehlen(RECHNET_NICHT.0, RECHNET_NICHT.1).to_string()),
                rechts: ohne_grenze.clone(),
                hinweis: beschreibung(r, self.speicher_bytes, sprache, rechnet),
                sperrgrund: (!rechnet).then(|| gesperrt_weil(r.rueckseite, sprache)),
            });
        }
        aus
    }
}

/// Einheit und Hoechstwert eines festen Betriebsmittels.
fn masse(h: &Hardware, name: &str) -> (Einheit, Option<u64>) {
    match name {
        "kap.kerne" => (Einheit::Kerne, Some(h.kerne as u64)),
        "kap.speicher" => (Einheit::Gib, h.speicher_bytes.map(|b| b / GIB)),
        // ⚑ **Das Ende ist, was frei ist.** Die ganze Platte waere
        // gelogen, denn sie gehoert nicht Myelith; was schon belegt
        // ist, steht ohnehin nicht mehr zur Freigabe.
        "kap.platte" => (Einheit::Gib, h.platte.as_ref().map(|p| p.frei_bytes / GIB)),
        _ => (Einheit::Gib, None),
    }
}

/// Ein Satz darueber, was ein Rechenwerk ist.
///
/// ⚑ **`rechnet` entscheidet, ob der Satz sagt, was der Anteil
/// bewirkt.** Bei einem gesperrten Werk steht das im Sperrgrund,
/// und zweimal dasselbe in zwei Saetzen unter demselben Regler
/// waere eine Wiederholung, die irgendwann auseinanderlaeuft.
pub fn beschreibung(
    r: &Rechenwerk,
    speicher_der_maschine: Option<u64>,
    sprache: crate::einstellungen::Sprache,
    rechnet: bool,
) -> String {
    let gib = |b: u64| format!("{:.0} GiB", b as f64 / GIB as f64);
    let kerne = match r.kerne {
        Some(k) => format!("{k} {}, ", sprache.waehlen("Kerne", "cores")),
        None => String::new(),
    };
    let bauart = if r.gemeinsamer_speicher {
        let speicher = match speicher_der_maschine {
            Some(b) => format!(
                "{} ({})",
                sprache.waehlen("gemeinsamer Speicher mit der CPU", "memory shared with the CPU"),
                gib(b)
            ),
            None => sprache
                .waehlen("gemeinsamer Speicher mit der CPU", "memory shared with the CPU")
                .to_string(),
        };
        sprache.waehlen_wert(
            format!("Eingebaut, {kerne}{speicher}. Der Anteil gilt der Rechenzeit dieses Werks."),
            format!("Integrated, {kerne}{speicher}. The share applies to this unit's compute time."),
        )
    } else {
        let speicher = match r.speicher_bytes {
            Some(b) => format!("{} {}", gib(b), sprache.waehlen("eigener Speicher", "of its own memory")),
            None => sprache
                .waehlen("eigener Speicher nicht erkannt", "own memory not detected")
                .to_string(),
        };
        sprache.waehlen_wert(
            format!(
                "Eigenständig, {kerne}{speicher}. Der Anteil gilt Rechenzeit und Speicher \
                 dieses Werks."
            ),
            format!(
                "Discrete, {kerne}{speicher}. The share applies to this unit's compute time \
                 and memory."
            ),
        )
    };
    if !rechnet {
        return bauart;
    }
    // ⚑ **Was der Anteil heute wirklich tut, und was er noch nicht
    // tut.** Abschaltbar ist das Werk, teilbar ist es nicht: Eine
    // Quote auf die Rechenzeit einer GPU braucht einen Planer, und
    // den gibt es nicht. **Der Satz sagt beides**, denn ein Regler,
    // der mehr verspricht als er haelt, ist schlimmer als ein
    // gesperrter.
    let anteil = sprache.waehlen(
        "Ganz links rechnet dieses Werk nicht mit, und der Rechenpfad läuft \
         vollständig auf der CPU weiter; jeder Wert darüber lässt es rechnen. Eine \
         Teilquote wirkt örtlich noch nicht, sie steht als Freigabe da: Ein Anteil an \
         der Rechenzeit einer GPU braucht einen Planer, den es noch nicht gibt.",
        "At the far left this unit does not take part and the compute path runs \
         entirely on the CPU; any value above lets it compute. A partial share has no \
         local effect yet, it is recorded as a pledge: a share of a GPU's compute time \
         needs a scheduler that does not exist yet.",
    );
    format!("{bauart} {anteil}")
}

#[cfg(test)]
mod proben {
    use super::*;
    use crate::einstellungen::Einstellungen;

    /// Ein Werk mit allem, was der Scan hergeben kann.
    fn werk(kennung: &str, name: &str, r: Option<Rueckseite>) -> Rechenwerk {
        Rechenwerk {
            kennung: kennung.into(),
            name: name.into(),
            speicher_bytes: Some(16 * GIB),
            frei_bytes: None,
            gemeinsamer_speicher: false,
            kerne: None,
            rueckseite: r,
            pci: None,
        }
    }

    /// ⛔️ **Zwei Karten desselben Herstellers, eine aus**: Die andere
    /// rechnet weiter, und keine ueberstimmt die andere.
    #[test]
    fn zwei_karten_eines_herstellers_werden_einzeln_geschaltet() {
        let mut a = werk("a", "Karte a", Some(Rueckseite::Cuda));
        a.pci = Some("0000:3b:00.0".into());
        let mut b = werk("b", "Karte b", Some(Rueckseite::Cuda));
        b.pci = Some("0000:5e:00.0".into());
        for reihenfolge in [vec![a.clone(), b.clone()], vec![b.clone(), a.clone()]] {
            let (karten, wege) = schaltplan(&reihenfolge, |k| k != "a");
            assert!(karten.contains(&("0000:3b:00.0".into(), false)), "{karten:?}");
            assert!(karten.contains(&("0000:5e:00.0".into(), true)), "{karten:?}");
            assert_eq!(wege, vec![("cuda", true)]);
        }
    }

    /// ⚑ **Ohne Adresse wird der Weg geschaltet**: Metal aus heisst aus;
    /// zwei Karten ohne Adresse, eine an, heisst an.
    #[test]
    fn ohne_adresse_wird_der_weg_geschaltet() {
        let m = werk("m", "GPU", Some(Rueckseite::Metal));
        assert_eq!(schaltplan(std::slice::from_ref(&m), |_| false), (vec![], vec![("metal", false)]));
        assert_eq!(schaltplan(std::slice::from_ref(&m), |_| true), (vec![], vec![("metal", true)]));
        let x = werk("x", "Karte x", Some(Rueckseite::Rocm));
        let y = werk("y", "Karte y", Some(Rueckseite::Rocm));
        assert_eq!(schaltplan(&[x.clone(), y.clone()], |k| k == "y").1, vec![("rocm", true)]);
        assert_eq!(schaltplan(&[x, y], |_| false).1, vec![("rocm", false)]);
        // Ein Werk ohne Rechenweg schaltet nichts.
        assert_eq!(schaltplan(&[werk("n", "n", None)], |_| false), (vec![], vec![]));
    }

    fn maschine(werke: Vec<Rechenwerk>) -> Hardware {
        Hardware { kerne: 8, speicher_bytes: Some(32 * GIB), speicher_frei_bytes: None, platte: None, rechenwerke: werke }
    }

    /// ⚑ **Der Scan raet nicht.** Was er nicht ermitteln kann, ist
    /// `None`, und daraus wird ein gesperrter Regler mit Begruendung
    /// statt eines Reglers mit erfundenem Ende.
    #[test]
    fn ein_regler_ohne_ende_ist_gesperrt() {
        let leer = Hardware {
            kerne: 4,
            speicher_bytes: None,
            speicher_frei_bytes: None,
            platte: None,
            rechenwerke: Vec::new(),
        };
        let r = leer.regler(&Einstellungen::default());
        let kerne = r.iter().find(|r| r.name == "kap.kerne").expect("Kerne");
        assert_eq!(kerne.hoechstens, Some(4));
        assert!(kerne.sperrgrund.is_none(), "der Kernregler ist gesperrt, obwohl er wirkt");

        for name in ["kap.speicher", "kap.platte"] {
            let g = r.iter().find(|r| r.name == name).expect(name);
            assert!(g.hoechstens.is_none());
            assert!(
                g.sperrgrund.is_some(),
                "{name} hat kein Ende und ist trotzdem nicht gesperrt"
            );
        }
    }

    /// ⚑ **Ohne Zutun steht jeder Regler ganz rechts auf „ohne
    /// Grenze".**
    ///
    /// Das ist der Auftrag des Projektinhabers vom 2026-09-16, und er
    /// hat zwei Haelften, die beide hier stehen: Der **Wert** ist nicht
    /// gesetzt (also ohne Grenze), und das **rechte Ende** traegt genau
    /// dieses Wort. Eine Vorgabe ohne Beschriftung waere nicht zu sehen.
    #[test]
    fn ohne_zutun_steht_jeder_regler_ganz_rechts() {
        let h = maschine(vec![werk("eins", "Karte eins", Some(Rueckseite::Cuda))]);
        for r in h.regler(&Einstellungen::default()) {
            assert_eq!(r.wert, None, "`{}` ist ohne Zutun begrenzt", r.name);
            assert_eq!(r.rechts, "ohne Grenze", "`{}` benennt sein rechtes Ende nicht", r.name);
        }
    }

    /// ⚑ **Die beiden linken Enden sind verschieden, und das ist der
    /// Unterschied zwischen einer Grenze und einem Anteil.**
    ///
    /// Null Kerne waeren ein Stillstand; ein Rechenwerk bei null rechnet
    /// einfach nicht mit, und der Rechenpfad laeuft auf der CPU weiter.
    #[test]
    fn eine_grenze_faengt_bei_eins_an_ein_rechenwerk_bei_null() {
        let h = maschine(vec![werk("eins", "Karte eins", Some(Rueckseite::Rocm))]);
        let r = h.regler(&Einstellungen::default());

        for name in ["kap.kerne", "kap.speicher", "kap.platte"] {
            let g = r.iter().find(|r| r.name == name).expect(name);
            assert_eq!(g.mindestens, 1, "{name} laesst sich auf null stellen");
            assert_eq!(g.links, None, "{name} benennt ein linkes Ende, das keinen Namen hat");
        }

        let w = r.iter().find(|r| r.name == "kap.rechenwerk.eins").expect("Werk");
        assert_eq!(w.mindestens, 0, "ein Rechenwerk laesst sich nicht abschalten");
        assert_eq!(w.links.as_deref(), Some("rechnet nicht"));
    }

    /// ⚑ **Ein Rechenwerk ist genau dann frei, wenn sein Rechenweg hier
    /// rechnet**, und das ist Fund 380.
    ///
    /// Bis zum 2026-09-16 war **jedes** Werk gesperrt, mit dem Satz
    /// „noch kein Rechenweg ueber dieses Geraet". Seit dem 2026-09-14
    /// rechnet `metal` die gebuendelten Matrizen der Vorbereitung; der
    /// Satz war also an der einen Maschine falsch, an der er
    /// ueberhaupt etwas zu sagen hatte.
    ///
    /// ⚑ **Geprueft wird gegen dieselbe Liste, die der Rechenpfad
    /// fragt**, und nicht gegen eine Annahme ueber dieses
    /// Betriebssystem: Ob `metal` rechnet, haengt an Geraet, Shader und
    /// einer Selbstpruefung gegen die CPU, und keines davon steht in
    /// einem `cfg`.
    #[test]
    fn frei_ist_ein_werk_genau_dann_wenn_sein_rechenweg_rechnet() {
        let h = maschine(vec![
            werk("apfel", "Apple M5 Pro", Some(Rueckseite::Metal)),
            werk("gruen", "NVIDIA GeForce RTX 4090", Some(Rueckseite::Cuda)),
            werk("rot", "AMD Radeon RX 7900 XTX", Some(Rueckseite::Rocm)),
            werk("blau", "Intel UHD Graphics", None),
        ]);
        let rechnende = crate::rechenwege::vorhanden();
        let r = h.regler(&Einstellungen::default());

        for (kennung, rueckseite) in [
            ("apfel", Some(Rueckseite::Metal)),
            ("gruen", Some(Rueckseite::Cuda)),
            ("rot", Some(Rueckseite::Rocm)),
            ("blau", None),
        ] {
            let g = r
                .iter()
                .find(|g| g.name == format!("kap.rechenwerk.{kennung}"))
                .expect(kennung);
            let rechnet = rueckseite.is_some_and(|b| rechnende.contains(&b.backend()));
            assert_eq!(
                g.sperrgrund.is_none(),
                rechnet,
                "`{kennung}` ist frei={} , aber sein Rechenweg rechnet={rechnet}",
                g.sperrgrund.is_none()
            );
        }

        // ⚑ **cuda und rocm reichen an die Referenzkernel weiter**, sie
        // koennen also nie frei sein. Waere das eines Tages anders,
        // beisst diese Zeile, und dann gehoert der Rechenweg in die
        // Liste und diese Zeile weg.
        assert!(!rechnende.contains(&"cuda"), "cuda meldet einen eigenen Rechenpfad");
        assert!(!rechnende.contains(&"rocm"), "rocm meldet einen eigenen Rechenpfad");
    }

    /// ⚑ **Ein Geraet ohne Rechenweg wird anders begruendet als eines,
    /// dessen Rechenweg noch nicht gebaut ist.**
    ///
    /// Der Unterschied ist der zwischen „noch nicht" und „gar nicht",
    /// und er entscheidet, ob jemand darauf wartet.
    #[test]
    fn der_sperrgrund_unterscheidet_noch_nicht_von_gar_nicht() {
        let h = maschine(vec![
            werk("gruen", "NVIDIA GeForce RTX 4090", Some(Rueckseite::Cuda)),
            werk("blau", "Intel UHD Graphics", None),
        ]);
        let r = h.regler(&Einstellungen::default());
        let grund = |k: &str| {
            r.iter()
                .find(|g| g.name == format!("kap.rechenwerk.{k}"))
                .and_then(|g| g.sperrgrund.clone())
                .unwrap_or_else(|| panic!("{k} ist nicht gesperrt"))
        };

        let gebaut = grund("gruen");
        // ⚑ Der Grund sagt, **was fehlt**, nicht nur dass etwas fehlt.
        assert!(gebaut.contains("CUDA"), "der Rechenweg des Geraets steht nicht im Grund: {gebaut}");
        assert!(gebaut.contains("Konformitätslauf") && gebaut.contains("Ganzzahlkerne"), "{gebaut}");

        let keiner = grund("blau");
        assert!(keiner.contains("keinen Rechenweg"), "{keiner}");
        assert!(
            !keiner.contains("Konformitätslauf"),
            "ein Geraet ohne Rechenweg laesst auf einen Prueflauf warten: {keiner}"
        );
    }

    /// ⚑ **Jedes gefundene Rechenwerk bekommt einen eigenen Regler.**
    #[test]
    fn jedes_rechenwerk_bekommt_einen_eigenen_regler() {
        let h = Hardware {
            kerne: 8,
            speicher_bytes: Some(32 * GIB),
            speicher_frei_bytes: None,
            platte: None,
            rechenwerke: vec![
                werk("eins", "Karte eins", Some(Rueckseite::Cuda)),
                Rechenwerk {
                    kennung: "zwei".into(),
                    name: "Karte zwei".into(),
                    speicher_bytes: None,
                    frei_bytes: None,
                    gemeinsamer_speicher: true,
                    kerne: Some(16),
                    rueckseite: Some(Rueckseite::Metal),
                    pci: None,
                },
            ],
        };
        let mut e = Einstellungen::default();
        e.setzen("kap.rechenwerk.zwei", "60").expect("setzen");

        let r = h.regler(&e);
        let werke: Vec<_> = r.iter().filter(|r| r.einheit == Einheit::Prozent).collect();
        assert_eq!(werke.len(), 2, "nicht jede Karte hat einen Regler");
        for w in &werke {
            assert_eq!(w.hoechstens, Some(100));
        }
        // ⚑ **Ohne Eintrag ganz rechts, mit Eintrag dort, wo er steht.**
        assert_eq!(werke[0].wert, None, "eine ungesetzte Freigabe ist nicht ohne Grenze");
        assert_eq!(werke[1].wert, Some(60), "die gesetzte Freigabe kommt nicht durch");

        // Und die Beschreibung nennt den Bezug des Anteils.
        assert!(werke[0].hinweis.contains("16 GiB eigener Speicher"), "{}", werke[0].hinweis);
        assert!(werke[1].hinweis.contains("gemeinsamer Speicher"), "{}", werke[1].hinweis);
        assert!(werke[1].hinweis.contains("16 Kerne"), "{}", werke[1].hinweis);
    }

    /// ⚑ **Ein Schalter, der nur in eine Richtung wirkt, ist keiner.**
    ///
    /// Der Durchlauf ueber die Rechenwerke kostet einen Unterprozess
    /// und laeuft deshalb nur, wenn er etwas aendern kann. **Die
    /// Abkuerzung hatte beim Schreiben ein Loch:** Wer ein Werk auf
    /// null zieht und gleich darauf wieder hinauf, hat danach keine
    /// Null mehr in der Ablage, und genau der Durchlauf, der das Werk
    /// zurueckholt, waere ausgefallen.
    ///
    /// 📌 **Alles darueber steht in einer einzigen Pruefung**, denn die
    /// Marke gilt fuer den ganzen Prozess und `cargo test` laeuft
    /// nebenlaeufig. Zwei Pruefungen daran waeren einzeln gruen und
    /// zusammen rot, und das ist Fund 378.
    #[test]
    fn aus_und_wieder_an_laeuft_beide_male_durch() {
        let mut e = Einstellungen::default();
        // Ohne Eintrag gibt es nichts abzuschalten und nichts zu scannen.
        assert!(!anwenden(&e), "der Durchlauf lief, obwohl nichts eingestellt ist");

        e.setzen("kap.rechenwerk.probe", "0").expect("auf null");
        assert!(anwenden(&e), "eine Null schaltet nichts ab");

        // ⚑ **Und wieder hinauf.** Jetzt steht keine Null mehr da, und
        // trotzdem muss der Durchlauf laufen, sonst bliebe das Werk aus.
        e.setzen("kap.rechenwerk.probe", "aus").expect("zurueck");
        assert!(anwenden(&e), "das Zurueckdrehen erreicht den Rechenpfad nicht");

        // Und danach ist wieder Ruhe.
        assert!(!anwenden(&e), "der Durchlauf laeuft weiter, obwohl nichts mehr aus ist");
    }

}
