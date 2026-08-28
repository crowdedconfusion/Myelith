//! Die Parameter-Registry (Punkt 1.1, Whitepaper Kap. 10.3).
//!
//! Alle Governance-Parameter an einem Ort, jeder mit seinem
//! **Änderbarkeits-Rang**.
//!
//! ## Was die Registry ist und was nicht
//!
//! Sie ist die maßgebliche Liste dessen, **worüber überhaupt abgestimmt
//! werden darf**, und der Ort, an dem der Rang steht. Sie ist **nicht**
//! die Stelle, an der die Werte im Betrieb gelesen werden: `myl-tokenomics`
//! rechnet weiter mit seinen eigenen Konstanten, `myl-consensus` mit
//! seinen. Diese Trennung ist Absicht, und sie hat einen Preis, der hier
//! genannt sein soll.
//!
//! **Der Preis:** Ein Wert steht damit an zwei Orten, und die Registry
//! kann davonlaufen. Genau dieses Muster hat das Projekt schon zweimal
//! bezahlt (A7: totes Stimmgewicht; Fund 44: die ganzzahligen
//! EMA-Konstanten lagen ungenutzt neben der Gleitkommarechnung).
//!
//! **Warum trotzdem so:** Die Alternative wäre, dass jedes Crate zur
//! Laufzeit eine Registry befragt, und damit hinge der Inferenz- und
//! Konsenspfad an einem veränderlichen Zustand, den er heute nicht hat.
//! Der Ausweg ist kein Vertrauen, sondern ein Test:
//! [`ParameterRegistry::vorgabe`] wird in `tests/gleichstand.rs` **gegen
//! die Konstanten der anderen Crates geprüft**. Läuft einer der beiden
//! Orte davon, schlägt der Test fehl. Ohne diesen Test wäre die Registry
//! genau die Art Artefakt, gegen die dieses Projekt seine Regeln
//! geschrieben hat.
//!
//! ## ⚑ Fund 48: „Gesamtangebot" ist ein Verfassungsrang ohne Gegenstand
//!
//! Kap. 10.3 nennt drei nicht änderbare Festlegungen: **Gesamtangebot**,
//! Burn-and-Mint-Prinzip, Determinismus-Pflicht der Runtime. Die ersten
//! beiden vertragen sich nicht:
//!
//! Burn-and-Mint hat **kein Gesamtangebot**. Der Umlauf ergibt sich aus
//! Prägung minus Verbrennung, und Anhang B.8.3 rechnet ausdrücklich
//! durch, was ein Emissionsdeckel bewirkt, mit dem Ergebnis: „Ein Deckel
//! wirkt damit nicht als Knappheitsgarantie, sondern als
//! Kapazitätsbremse." Der einzige Deckel, den das Protokoll kennt, ist
//! `M_max` je Epoche, und der steht in dieser Komponente als
//! **änderbarer** Parameter.
//!
//! Es gibt also drei mögliche Lesarten, und keine ist im Papier belegt:
//! (a) „Gesamtangebot" meint `M_max`, dann widerspricht Kap. 10.3 dem
//! Rest des Papiers; (b) es meint „die Regel, dass der Umlauf aus
//! Burn-and-Mint
//! folgt" — dann ist es dasselbe wie der zweite Punkt; (c) es ist ein
//! Rest aus einem früheren Entwurf mit fester Obergrenze.
//!
//! **Hier umgesetzt als (b)**, also als eigener Verfassungsschalter
//! [`Parameter::GesamtangebotFestgelegtDurchBurnAndMint`], der aussagt:
//! Es gibt keine andere Quelle von MYL als die Prägung gegen
//! verifizierte Arbeit. Das ist die einzige Lesart, die etwas
//! Durchsetzbares ergibt. **Die Entscheidung gehört dem Projektinhaber**
//! ist festgelegt; ändert sie sich, ändert sich diese Zeile.

use std::collections::{BTreeMap, BTreeSet};

use myl_types::hash::Hash;

/// Der Änderbarkeits-Rang eines Parameters (Kap. 10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Aenderbarkeit {
    /// Per Abstimmung änderbar.
    Aenderbar,
    /// Verfassungsrang: nicht änderbar, auch nicht einstimmig.
    ///
    /// Ein Vorschlag dazu wird von [`crate::pruefe_vorschlag`]
    /// zurückgewiesen, **bevor** er zur Abstimmung kommt. Kap. 10.3
    /// verlangt das nicht ausdrücklich; die Design-Entscheidung 2 des
    /// dieser Komponente tut es, mit der Begründung, dass eine rein
    /// prozessuale Regel nur so stark ist wie die Disziplin der
    /// Beteiligten.
    Verfassungsrang,
}

/// Der Wert eines Parameters.
///
/// Ganzzahlig oder als Bruch, nie als Gleitkommazahl: Diese Werte gehen
/// in Ledger-Zustandsübergänge ein und müssen auf jedem Knoten bitgleich
/// nachrechenbar sein.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wert {
    /// Eine ganze Zahl (Beträge, Anzahlen, Sekunden).
    Ganzzahl(u64),
    /// Ein Bruch `zaehler/nenner` (Raten, Anteile).
    Bruch { zaehler: u64, nenner: u64 },
    /// Ein Schalter (die Verfassungsprinzipien).
    Schalter(bool),
    /// Eine Menge von Hashes (die Kernel-Whitelist).
    Hashmenge(BTreeSet<Hash>),
}

impl Wert {
    /// Der Wert als Bruch, falls er einer ist.
    pub fn als_bruch(&self) -> Option<(u64, u64)> {
        match self {
            Self::Bruch { zaehler, nenner } => Some((*zaehler, *nenner)),
            _ => None,
        }
    }

    /// Der Wert als Ganzzahl, falls er eine ist.
    pub fn als_ganzzahl(&self) -> Option<u64> {
        match self {
            Self::Ganzzahl(n) => Some(*n),
            _ => None,
        }
    }

    /// Kurzname der Art, für Fehlermeldungen.
    fn art(&self) -> &'static str {
        match self {
            Self::Ganzzahl(_) => "Ganzzahl",
            Self::Bruch { .. } => "Bruch",
            Self::Schalter(_) => "Schalter",
            Self::Hashmenge(_) => "Hashmenge",
        }
    }
}

/// Die Governance-Parameter des Protokolls.
///
/// Aufgenommen ist, was im Whitepaper oder in einer
/// Design-Entscheidung ausdrücklich als Governance-Parameter benannt ist.
/// Die Fundstelle steht jeweils in der Dokumentation; **ein Parameter
/// ohne Fundstelle gehört nicht in diese Liste**, sonst entscheidet die
/// Registry über Dinge, über die niemand entschieden hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Parameter {
    // ---- änderbar, Kap. 10.3 wörtlich ----
    /// Stichprobenrate `p` der Stufe-2-Prüfung (Kap. 5.5, 10.3).
    Stichprobenrate,
    /// Subventionsrate `s` der Anlaufphase (Kap. 5.7, 10.3).
    Subventionsrate,
    /// Zugelassene Kernel-Versionen (Kap. 10.3).
    KernelWhitelist,
    /// Auslastungsziel `u*` der Preisbildung (Kap. 5.4, 10.3).
    Auslastungsziel,
    /// Streitfrist in Sekunden (Kap. 6.6, 10.3; Entwurf: 7 Tage).
    ///
    /// **In Sekunden und nicht in Epochen**, weil „7 Epochen" ohne
    /// festgelegte Epochenlänge keine Frist ist. Genau daran hing
    /// ⚑ Fund 50: `myl_consensus::epoch_close::DEFAULT_DISPUTE_EPOCHS`
    /// stand auf 7 mit dem Kommentar „entspricht der Design-Entscheidung
    /// 7 Tage", was Tages-Epochen unterstellt, während der Rest des
    /// Projekts mit Stunden-Epochen rechnet. Ein Faktor 24 in der Zeit,
    /// die für die Aufdeckung eines Betrugs bleibt.
    Streitfrist,

    // ---- änderbar, an anderer Stelle als Governance-Parameter benannt ----
    /// Preis-Sensitivität `κ` (Kap. 5.4).
    PreisSensitivitaet,
    /// Anteil `γ` eingeschleuster Kontrollsegmente (Kap. 6.7).
    Kontrollsegmentanteil,
    /// Größe des Vorrats an Kontrollsegmenten (⚑ Fund 58).
    ///
    /// **Eine Sicherheitsschranke, keine Puffergröße.** Der Vorrat ist
    /// endlich, der Auftragsstrom nicht; wird öfter eingeschleust, als
    /// der Vorrat verschiedene Segmente hält, wiederholen sich
    /// Segment-Ids. Echte Arbeit wiederholt sich nie, also ist jedes
    /// zweite Auftreten kein Verdacht, sondern ein **Beweis**, und ein
    /// Miner braucht dafür nur ein Gedächtnis. Gemessen am 2026-08-25:
    /// Bei γ = 2 % über 100 000 Aufträge erkennt er bei einem Vorrat von
    /// 64 rund **96,8 %** der Kontrollen, **ohne einen einzigen
    /// Fehlalarm** — er rechnet genau die ehrlich und manipuliert den
    /// Rest.
    ///
    /// **Warum das hier steht und nicht als Konstante im Quelltext:**
    /// Die Schranke ist γ × Auftragsrate × Beobachtungsfenster, und alle
    /// drei Größen bewegen sich im Betrieb. Eine Zahl, die sich mit dem
    /// Netz ändern muss, aber nur mit einer neuen Version änderbar ist,
    /// wird irgendwann falsch und bleibt es.
    ///
    /// Der Wert ist **änderbar**, das Unterschreiten der Schranke nicht:
    /// [`crate::Invariante::VorratTraegtEinschleusung`] weist jeden
    /// Vorschlag darunter zurück, bevor abgestimmt wird. Dieselbe
    /// Bauart wie `S_min`.
    Kontrollsegmentvorrat,
    /// Beobachtungsfenster des Wiederholungsunterscheiders, in Aufträgen
    /// (⚑ Fund 58).
    ///
    /// Über wie viele Aufträge hinweg dem Angreifer unterstellt wird,
    /// dass er Segment-Ids vergleichen kann. **Eine Annahme über den
    /// Angreifer, keine Messung:** Ein Gedächtnis kostet ihn nur
    /// Speicher, 100 000 Ids sind 3,2 MB. Die Zahl sagt deshalb nicht,
    /// was er kann, sondern wogegen das Protokoll sich zu schützen
    /// verpflichtet.
    ///
    /// **Ohne sie ist die Schranke nicht hinschreibbar.** „Der Vorrat
    /// muss größer sein als die Zahl der Einschleusungen" ist für einen
    /// unbegrenzten Auftragsstrom von keinem endlichen Vorrat
    /// erfüllbar; erst ein Fenster macht die Bedingung entscheidbar.
    /// Wer es senkt, senkt die Schranke — deshalb ist es ein Parameter
    /// mit Fundstelle und kein Rechenhilfsmittel.
    Kontrollsegmentfenster,
    /// Obergrenze der Trainingsvergütung als Anteil der Inferenzvergütung
    /// (Kap. 5.6, Entwurf: 70 %).
    TrainingsverguetungsAnteil,
    /// Präge-Obergrenze `M_max` je Epoche (Kap. 5.2, Anhang B.8.3).
    PraegeObergrenze,
    /// Glättungsfaktor `α` der Burn-EMA (Kap. 5.2, Entwurf: 2/31).
    ///
    /// Als Governance-Parameter aufgenommen wegen **Fund 47**: Ein α > 1
    /// führte in `myl-tokenomics` zu einem Umlauf, und die Behebung dort
    /// verweist ausdrücklich hierher.
    EmaGlaettung,
    /// Redundanzfaktor `r`, Pods je Segment (Kap. 4.4, Entwurf: 2).
    Redundanzfaktor,
    /// Shard-Zahl `k` je Pipeline (Kap. 4.1: „pro Modellversion
    /// konfigurierbar und unterliegt der Governance").
    Shardzahl,
    /// Größe des Validatoren-Komitees (Design-Entscheidung 2026-08-13,
    /// Entwurf: 21).
    Komiteegroesse,
    /// Blockzeit-Zielwert in Millisekunden (Design-Entscheidung
    /// 2026-08-13, Entwurf: 2 s).
    Blockzeit,
    /// Der geglättete Burn `B̄_e` der laufenden Epoche, in Kleinstbeträgen.
    ///
    /// **Kein Governance-Parameter im üblichen Sinn**, sondern der
    /// Zustand, den die EMA fortschreibt. Er steht in der Registry, weil
    /// [`crate::Invariante::PraegedeckelNichtBindend`] ihn braucht: Ohne
    /// den laufenden Wert lässt sich nicht sagen, ob ein vorgeschlagenes
    /// `M_max` bindend wäre.
    ///
    /// Änderbar ist er, weil er sich mit jeder Epoche ändert; ein
    /// Vorschlag, ihn zu setzen, ist keine Abstimmung, sondern die
    /// Fortschreibung durch den Epochenabschluss.
    GeglaetteterBurn,
    /// Untergrenze des Credit-Preises, Fixed-Point mit 32 Nachkommabits.
    ///
    /// Seit Fund 46 wird der Preis beschnitten statt umzulaufen, aber
    /// **null war zulässig**, und ein Preis von null heißt kostenlose
    /// Inferenz für alle. Der Startwert ist ein Kleinstbetrag: praktisch
    /// null, strukturell nicht null.
    ///
    /// Eine **inhaltlich** begründete Untergrenze, etwa aus den
    /// Realkosten, ist eine wirtschaftliche Entscheidung und offen;
    /// dieser Parameter hält nur die Null aus dem Weg.
    PreisUntergrenze,
    /// Stichprobenrate für **Trainingssegmente** (Kap. 5.5).
    ///
    /// **Eigener Parameter statt eines Faktors auf die Inferenzrate.**
    /// Die beiden werden aus verschiedenen Gründen bewegt: Die
    /// Inferenzrate sinkt planmäßig mit wachsendem Netz (Kap. 5.7), die
    /// Trainingsrate hängt am Schadensverhältnis und hat mit der
    /// Netzgröße nichts zu tun. Ein Faktor würde sie bei jeder
    /// planmäßigen Senkung mitziehen, und genau das soll nicht geschehen.
    ///
    /// Es bleibt die Invariante, dass sie **nie unter** der Inferenzrate
    /// liegt; sonst kehrt sich die Begründung aus Kap. 5.5 um.
    TrainingsStichprobenrate,
    /// Bezugsgröße des Arbeitsanteils im Stimmgewicht (Fund 51).
    ///
    /// Die vTFE-Menge, die einen Bonus in Höhe des Stakes wert ist.
    /// `myl-consensus` führt sie als Startparameter; hier steht sie,
    /// damit der Gleichstands-Test sie hält. Sie veraltet mit jeder
    /// Durchsatz-Optimierung, und genau das ist einmal unbemerkt
    /// geschehen.
    Arbeitsbezug,
    /// Höchstfaktor des Stimmgewichts auf den Stake.
    Hoechstfaktor,
    /// Anteil der Gesamtstimmkraft, der sich beteiligen muss, damit
    /// eine Abstimmung überhaupt zählt (Promille).
    ///
    /// Ohne Quorum entscheidet, wer gerade wach ist. Mit einem zu hohen
    /// blockiert Abwesenheit jede Änderung; das ist die Abwägung, und
    /// die Zahl gehört deshalb hierher und nicht in den Quelltext.
    Abstimmungsquorum,
    /// Anteil der abgegebenen Ja- und Nein-Stimmen, der für einen
    /// Vorschlag stimmen muss (Promille).
    ///
    /// # ⚑ Dieser Parameter kann sich selbst ändern
    ///
    /// Er ist änderbar, und er entscheidet über Änderungen. Ohne
    /// Untergrenze könnte eine knappe Mehrheit ihn auf null setzen und
    /// danach alles beschließen, wofür sie sonst keine Mehrheit hätte,
    /// **einschließlich der Rücknahme jeder anderen Grenze**. Zwei
    /// Abstimmungen, und die Governance gehört einer Minderheit.
    ///
    /// [`crate::invarianten::Invariante::AbstimmungBleibtBindend`] hält
    /// ihn deshalb bei mindestens 500 Promille. Änderbar bleibt er,
    /// aber nicht unter die Hälfte: Eine Minderheit kann nie gewinnen.
    Abstimmungsmehrheit,
    /// Zahl der Epochen, die eine Abstimmung offen steht.
    Abstimmungsfenster,
    /// ⚑ **Welche Signaturverfahren gerade gelten** (0, 1 oder 2).
    ///
    /// Der Schalter für den Wechsel auf ein quantensicheres Verfahren.
    /// Drei Stufen und nicht zwei, weil ein Sprung von „nur klassisch"
    /// auf „nur quantensicher" jeden Validator ungültig machte, der
    /// seinen zweiten Schlüssel noch nicht veröffentlicht hat, und damit
    /// die Kette anhielte. Die Folge ist einbahnig: Ein Rückschritt
    /// öffnete das gebrochene Verfahren wieder, und genau dann, wenn
    /// jemand es gebrochen hat.
    ///
    /// **Änderbar, aber nur um einen Schritt nach vorn.** Die zweite
    /// Bedingung, dass alle Validatoren bereit sind, prüft der Konsens:
    /// Die Registry kennt Parameter, nicht Validatoren.
    Signaturstufe,
    /// Länge einer Epoche in Sekunden.
    ///
    /// **⚑ Fund 50: Dieser Parameter fehlte, und das war teuer.** Er
    /// steht in keinem Kapitel und in keiner Design-Entscheidung, wird
    /// aber überall gebraucht, sobald eine Frist oder eine Rate „je
    /// Epoche" gilt. Zwei Teile des Projekts haben ihn deshalb
    /// stillschweigend verschieden angenommen, siehe
    /// [`Parameter::Streitfrist`].
    ///
    /// Der Entwurf setzt **eine Stunde**, weil Anhang B.1 („Bei
    /// Stunden-Epochen: etwa ein Tag Einkommen als Pfand") und die
    /// Stimmgewichts-Kalibrierung vom 2026-08-23 („Faktor nach einer
    /// Stunden-Epoche") beide damit rechnen. Das ist die
    /// **verbreitetste** Annahme im Projekt, nicht eine getroffene
    /// Entscheidung; sie gehört bestätigt.
    Epochenlaenge,
    /// Hinterlegter Mindest-Stake je Kapazitätseinheit, in
    /// Kleinstbeträgen (Kap. 5.5).
    MindestStake,
    /// Angenommener Betrugsgewinn `g` je Segment, in Kleinstbeträgen
    /// (Kap. 5.5). Geht mit `p` in `S_min = g/p²` ein.
    Betrugsgewinn,
    /// Kopfgeldanteil `b` am geschlachteten Betrag (Anhang B.3: 30 %).
    ///
    /// Der Rest bleibt unverteilt, ist also faktisch verbrannt. Ginge der
    /// volle Slash an den Checker, wäre eine erfolgreiche Anfechtung ein
    /// Geschäft, und ein Checker, der einen Miner zum Betrug verleiten
    /// kann, verdiente daran.
    Kopfgeldanteil,
    /// Realkostenanteil `c` am Reward (Anhang B.4, empirisch 0,6–0,8).
    ///
    /// **Mit Vorbehalt aufgenommen**, siehe
    /// [`crate::invarianten::Invariante::SelfDealing`]: `c` ist keine
    /// Protokollgröße, sondern eine Beobachtung über die Welt, und es
    /// bestimmt die Obergrenze von `s`.
    Kostenanteil,

    // ---- Verfassungsrang, Kap. 10.3 wörtlich ----
    /// Es gibt keine andere Quelle von MYL als die Prägung gegen
    /// verifizierte Arbeit (Kap. 10.3, „Gesamtangebot"; zur Lesart siehe
    /// Fund 48 in der Modul-Dokumentation).
    GesamtangebotFestgelegtDurchBurnAndMint,
    /// Credits entstehen ausschließlich durch Verbrennen von MYL
    /// (Kap. 10.3, „Burn-and-Mint-Prinzip").
    BurnAndMintPrinzip,
    /// Die Runtime rechnet vollständig ganzzahlig (Kap. 10.3,
    /// „Determinismus-Pflicht der Runtime").
    DeterminismusPflicht,
}

impl Parameter {
    /// Alle Parameter, in fester Reihenfolge.
    ///
    /// **Ein neuer Parameter darf mitten hinein.** `Parameter` leitet
    /// `Ord` aus der Deklarationsreihenfolge ab, und die
    /// [`ParameterRegistry`] hält ihre Werte in einer `BTreeMap`; eine
    /// eingeschobene Variante verschiebt damit die Sortierung. Das ist
    /// hier folgenlos, und der Grund gehört aufgeschrieben, damit ihn
    /// niemand neu prüfen muss: Der Typ wird **nirgends serialisiert**
    /// (keine Borsh-, keine Serde-Ableitung), es gibt **keinen
    /// kanonischen Hash** über die Registry, und jede Prüfung läuft
    /// über diese Liste statt über die Kartenreihenfolge. Käme eines
    /// der drei hinzu, wäre das Einschieben eine Protokolländerung.
    pub fn alle() -> [Parameter; 33] {
        use Parameter::*;
        [
            Stichprobenrate,
            Subventionsrate,
            KernelWhitelist,
            Auslastungsziel,
            Streitfrist,
            PreisSensitivitaet,
            Kontrollsegmentanteil,
            Kontrollsegmentvorrat,
            Kontrollsegmentfenster,
            TrainingsverguetungsAnteil,
            PraegeObergrenze,
            EmaGlaettung,
            Redundanzfaktor,
            Shardzahl,
            Komiteegroesse,
            Abstimmungsquorum,
            Abstimmungsmehrheit,
            Abstimmungsfenster,
            Blockzeit,
            GeglaetteterBurn,
            PreisUntergrenze,
            TrainingsStichprobenrate,
            Arbeitsbezug,
            Hoechstfaktor,
            Epochenlaenge,
            MindestStake,
            Betrugsgewinn,
            Kopfgeldanteil,
            Kostenanteil,
            GesamtangebotFestgelegtDurchBurnAndMint,
            BurnAndMintPrinzip,
            DeterminismusPflicht,
            Signaturstufe,
        ]
    }

    /// Der Änderbarkeits-Rang.
    ///
    /// **Am Typ festgemacht, nicht am Datensatz.** Stünde der Rang als
    /// Feld in der Registry, ließe er sich mit einem Vorschlag ändern,
    /// der den Rang selbst zum Gegenstand hat — und damit wäre der
    /// Verfassungsrang eine Vereinbarung statt einer Schranke.
    pub fn rang(&self) -> Aenderbarkeit {
        use Parameter::*;
        match self {
            GesamtangebotFestgelegtDurchBurnAndMint
            | BurnAndMintPrinzip
            | DeterminismusPflicht => Aenderbarkeit::Verfassungsrang,
            _ => Aenderbarkeit::Aenderbar,
        }
    }

    /// Kurzname für Fehlermeldungen und Protokolle.
    pub fn name(&self) -> &'static str {
        use Parameter::*;
        match self {
            Stichprobenrate => "Stichprobenrate p",
            Subventionsrate => "Subventionsrate s",
            KernelWhitelist => "Kernel-Whitelist",
            Auslastungsziel => "Auslastungsziel u*",
            Streitfrist => "Streitfrist",
            PreisSensitivitaet => "Preis-Sensitivität kappa",
            Kontrollsegmentanteil => "Kontrollsegmentanteil gamma",
            Kontrollsegmentvorrat => "Kontrollsegment-Vorrat",
            Kontrollsegmentfenster => "Kontrollsegment-Beobachtungsfenster",
            TrainingsverguetungsAnteil => "Trainingsvergütungs-Anteil",
            PraegeObergrenze => "Präge-Obergrenze M_max",
            EmaGlaettung => "EMA-Glättung alpha",
            Redundanzfaktor => "Redundanzfaktor r",
            Shardzahl => "Shardzahl k",
            Komiteegroesse => "Komiteegröße",
            Abstimmungsquorum => "Abstimmungsquorum",
            Abstimmungsmehrheit => "Abstimmungsmehrheit",
            Abstimmungsfenster => "Abstimmungsfenster",
            Signaturstufe => "Signaturstufe",
            Blockzeit => "Blockzeit",
            Epochenlaenge => "Epochenlänge",
            GeglaetteterBurn => "geglätteter Burn B_e",
            PreisUntergrenze => "Preis-Untergrenze",
            TrainingsStichprobenrate => "Trainings-Stichprobenrate",
            Arbeitsbezug => "Arbeitsbezug des Stimmgewichts",
            Hoechstfaktor => "Höchstfaktor des Stimmgewichts",
            MindestStake => "Mindest-Stake S",
            Betrugsgewinn => "Betrugsgewinn g",
            Kopfgeldanteil => "Kopfgeldanteil b",
            Kostenanteil => "Realkostenanteil c",
            GesamtangebotFestgelegtDurchBurnAndMint => "Gesamtangebot (Burn-and-Mint)",
            BurnAndMintPrinzip => "Burn-and-Mint-Prinzip",
            DeterminismusPflicht => "Determinismus-Pflicht der Runtime",
        }
    }
}

/// Fehler beim Umgang mit der Registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryFehler {
    /// Der neue Wert hat eine andere Art als der bisherige.
    ///
    /// Ein Vorschlag, der aus einer Rate einen Schalter macht, ist kein
    /// Parametervorschlag, sondern eine Protokolländerung.
    ArtPasstNicht {
        parameter: Parameter,
        erwartet: &'static str,
        bekommen: &'static str,
    },
}

impl std::fmt::Display for RegistryFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArtPasstNicht { parameter, erwartet, bekommen } => write!(
                f,
                "{}: erwartet {}, bekommen {}",
                parameter.name(),
                erwartet,
                bekommen
            ),
        }
    }
}

impl std::error::Error for RegistryFehler {}

/// Der Satz gültiger Parameterwerte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterRegistry {
    werte: BTreeMap<Parameter, Wert>,
}

impl ParameterRegistry {
    /// Die Vorgabewerte des Entwurfs.
    ///
    /// **Jeder Wert hier hat eine Fundstelle** im Whitepaper oder in einer
    /// datierten Design-Entscheidung; `tests/gleichstand.rs` prüft sie
    /// gegen die Konstanten der anderen Crates, damit die beiden Orte
    /// nicht auseinanderlaufen.
    pub fn vorgabe() -> Self {
        use Parameter::*;
        let mut werte = BTreeMap::new();
        // Kap. 5.5: p = 2 %.
        werte.insert(Stichprobenrate, Wert::Bruch { zaehler: 2, nenner: 100 });
        // Kap. 5.7 / Anhang B.4: Start-Subvention s = 0,5.
        werte.insert(Subventionsrate, Wert::Bruch { zaehler: 1, nenner: 2 });
        // Leer bis zum Genesis-Manifest (GOVERNANCE 3.4).
        werte.insert(KernelWhitelist, Wert::Hashmenge(BTreeSet::new()));
        // Kap. 5.4: Auslastungsziel 0,7.
        werte.insert(Auslastungsziel, Wert::Bruch { zaehler: 7, nenner: 10 });
        // Design-Entscheidung 2026-08-13: 7 Tage.
        werte.insert(Streitfrist, Wert::Ganzzahl(7 * 24 * 60 * 60));
        // Kap. 5.4: kappa = 0,1.
        werte.insert(PreisSensitivitaet, Wert::Bruch { zaehler: 1, nenner: 10 });
        // Kap. 6.7: 1 bis 3 % des Volumens; Entwurf 2 %.
        werte.insert(Kontrollsegmentanteil, Wert::Bruch { zaehler: 2, nenner: 100 });
        // Fund 58: der gemessene Vorrat ohne erkannte Kontrolle und das
        // Fenster, über das gemessen wurde. Beide stehen in
        // `myl-verifier`, damit sie nicht zweimal dastehen.
        werte.insert(
            Kontrollsegmentvorrat,
            Wert::Ganzzahl(myl_verifier::VORRAT_VORGABE),
        );
        werte.insert(
            Kontrollsegmentfenster,
            Wert::Ganzzahl(myl_verifier::BEOBACHTUNGSFENSTER_VORGABE),
        );
        // Kap. 5.6: höchstens 70 % (myl-tokenomics: TRAINING_CAP_BPS).
        werte.insert(TrainingsverguetungsAnteil, Wert::Bruch { zaehler: 7_000, nenner: 10_000 });
        // Anhang B.8.3: ohne bindenden Deckel im Entwurf.
        werte.insert(PraegeObergrenze, Wert::Ganzzahl(u64::MAX));
        // myl-tokenomics: EMA_ALPHA_NUM/DEN = 2/31 (30-Epochen-Fenster).
        werte.insert(EmaGlaettung, Wert::Bruch { zaehler: 2, nenner: 31 });
        // Kap. 4.4: r = 2.
        werte.insert(Redundanzfaktor, Wert::Ganzzahl(2));
        // Kap. 4.1: k = 8.
        werte.insert(Shardzahl, Wert::Ganzzahl(8));
        // Design-Entscheidung 2026-08-13: 21 Validatoren.
        werte.insert(Komiteegroesse, Wert::Ganzzahl(21));
        // ⚑ Entwurfswerte, keine getroffene Entscheidung: Kap. 10.2
        // legt das Stimmgewicht fest, aber nicht das Verfahren
        // (Design-Entscheidung 1, offen). Die Invariante hält die
        // strukturelle Grenze, diese drei Zahlen sind Politik.
        werte.insert(Abstimmungsquorum, Wert::Ganzzahl(crate::abstimmung::QUORUM_VORGABE));
        werte.insert(
            Abstimmungsmehrheit,
            Wert::Ganzzahl(crate::abstimmung::MEHRHEIT_VORGABE),
        );
        werte.insert(
            Abstimmungsfenster,
            Wert::Ganzzahl(crate::abstimmung::FENSTER_VORGABE),
        );
        // Heute gilt nur BLS12-381, und es gibt kein zweites Verfahren.
        // Der Parameter steht trotzdem, damit der Schalter eine Stellung
        // hat und die Invariante etwas zu bewachen.
        werte.insert(
            Signaturstufe,
            Wert::Ganzzahl(myl_types::pq::Signaturstufe::NurKlassisch.zahl()),
        );
        // Design-Entscheidung 2026-08-13: 2 s.
        werte.insert(Blockzeit, Wert::Ganzzahl(2_000));
        // Anhang B.1 und die Stimmgewichts-Kalibrierung: eine Stunde.
        werte.insert(Epochenlaenge, Wert::Ganzzahl(60 * 60));
        // Startzustand: noch kein Burn beobachtet.
        werte.insert(GeglaetteterBurn, Wert::Ganzzahl(0));
        // Ein Kleinstbetrag: praktisch null, strukturell nicht null.
        werte.insert(PreisUntergrenze, Wert::Ganzzahl(1));
        // Kap. 5.5: erhöhte Rate für Trainingssegmente; Entwurf 10 %.
        werte.insert(TrainingsStichprobenrate, Wert::Bruch { zaehler: 10, nenner: 100 });
        // myl-consensus: Referenzknoten, Stunden-Epoche (Fund 51).
        werte.insert(
            Arbeitsbezug,
            Wert::Ganzzahl(myl_consensus::voting_weight::ARBEITSBEZUG_VORGABE),
        );
        werte.insert(
            Hoechstfaktor,
            Wert::Ganzzahl(myl_consensus::voting_weight::HOECHSTFAKTOR_VORGABE),
        );
        // Anhang B.1: g = 0,5 MYL, S_min = 1250 MYL je Kapazitätseinheit.
        werte.insert(MindestStake, Wert::Ganzzahl(1_250 * myl_tokenomics::UNITS_PER_MYL));
        werte.insert(Betrugsgewinn, Wert::Ganzzahl(myl_tokenomics::UNITS_PER_MYL / 2));
        // Anhang B.3: b = 30 % des geschlachteten Betrags.
        werte.insert(
            Kopfgeldanteil,
            Wert::Bruch {
                zaehler: myl_tokenomics::slashing::KOPFGELD_ZAEHLER,
                nenner: myl_tokenomics::slashing::KOPFGELD_NENNER,
            },
        );
        // Anhang B.4: c empirisch 0,6 bis 0,8; Entwurf 0,7.
        werte.insert(Kostenanteil, Wert::Bruch { zaehler: 7, nenner: 10 });
        // Kap. 10.3, Verfassungsrang.
        werte.insert(GesamtangebotFestgelegtDurchBurnAndMint, Wert::Schalter(true));
        werte.insert(BurnAndMintPrinzip, Wert::Schalter(true));
        werte.insert(DeterminismusPflicht, Wert::Schalter(true));
        Self { werte }
    }

    /// Der Wert eines Parameters.
    ///
    /// Kann nicht fehlen: [`Self::vorgabe`] setzt jeden Parameter, und
    /// [`Self::mit`] ändert nur bestehende. Der Test
    /// `jeder_parameter_hat_einen_wert` hält das fest.
    pub fn wert(&self, p: Parameter) -> &Wert {
        self.werte
            .get(&p)
            .expect("die Registry trägt jeden Parameter; siehe vorgabe()")
    }

    /// Eine Kopie mit einem geänderten Wert.
    ///
    /// Prüft **nur die Art**, nicht den Rang und nicht die Invarianten:
    /// Das tut [`crate::pruefe_vorschlag`]. Diese Trennung erlaubt es,
    /// einen Vorschlag probeweise anzuwenden und den **entstehenden
    /// Zustand** zu prüfen, statt den Parameter für sich allein zu
    /// beurteilen. Invarianten wie `s < c/(1−c)` verbinden zwei
    /// Parameter; wer nur den geänderten ansieht, kann sie nicht prüfen.
    pub fn mit(&self, p: Parameter, neu: Wert) -> Result<Self, RegistryFehler> {
        let bisher = self.wert(p);
        if std::mem::discriminant(bisher) != std::mem::discriminant(&neu) {
            return Err(RegistryFehler::ArtPasstNicht {
                parameter: p,
                erwartet: bisher.art(),
                bekommen: neu.art(),
            });
        }
        let mut werte = self.werte.clone();
        werte.insert(p, neu);
        Ok(Self { werte })
    }
}

impl Default for ParameterRegistry {
    fn default() -> Self {
        Self::vorgabe()
    }
}
