//! Session-Kontrakte — Whitepaper Kap. 8.2.
//!
//! Ein Agent, der zahlen kann, macht aus einem Rechenfehler einen
//! Vermögensschaden. Die Antwort des Protokolls ist nicht, den Fall
//! auszuschließen, sondern seine Auswirkung zu begrenzen: Jede
//! Agenten-Session läuft unter einem Kontrakt mit Gesamtbudget,
//! Einzeltransaktionslimit, Empfängerliste und Zeitfenster.
//!
//! ⚑ **Ein Kontrakt ist kein Programm, sondern ein Sprengradius.** Was
//! der Agent tun soll, steht im Prompt. Was im schlimmsten Fall
//! passieren kann, steht hier. Deshalb ist dies eine feste Struktur und
//! keine Sprache: Ein Parser im Konsenspfad wäre eine Angriffsfläche,
//! ein Auswerter eine zweite, und beide brauchten ein Kostenmodell, das
//! es in diesem Protokoll nicht gibt. Neue Grenzenarten kommen über
//! Governance, nicht über eine Nutzereingabe.
//!
//! ⚑ **Und eine Sprache, die den Inhalt lesen könnte, risse auf, was
//! Kap. 8.3 strukturell schließt.** „Erlaube, wenn im Verwendungszweck
//! 'Rechnung' steht" machte angreiferkontrollierten Text zum
//! Steuerfluss. Geprüft wird ausschließlich gegen das, was der Inhaber
//! festgelegt hat, und gegen das, was der Konsens ohnehin sieht.
//!
//! ## Was hier steht und was nicht
//!
//! Der Kontrakt ist **unveränderlich**: Seine Adresse ist der Hash
//! seiner Kodierung. Der Verbrauch ist **Zustand unter dieser Adresse**
//! ([`Sitzungszustand`]), sonst änderte sich die Adresse bei jeder
//! Ausgabe.
//!
//! ## Die Grenze dieser Konstruktion, benannt statt verschwiegen
//!
//! ⚑ **„Nicht lesbar" und „nicht änderbar" sind nicht gleich stark, und
//! nur eines ist eine Sicherheitseigenschaft.** Eine Ablehnung kostet
//! nichts, also kann ein Agent jede Grenze abtasten: Einzellimit und
//! Restbudget in etwa zwanzig abgelehnten Versuchen, die Empfängerliste
//! durch Aufzählen. **Geheimhaltung der Zahl ist damit nahezu wertlos,
//! Unveränderlichkeit trägt die ganze Eigenschaft.**
//!
//! Beides ist trotzdem gebaut: [`Befund::fuer_agenten`] gibt genau ein
//! Bit heraus. Das verhindert das Durchsickern über den Fehlerkanal, es
//! verhindert nicht das Abtasten. Wer das verwechselt, hält einen Test
//! für bestanden, der aus dem falschen Grund grün ist.
//!
//! ## Zeit wird in Epochen gemessen, nicht in Sekunden
//!
//! Das Zeitfenster steht in [`EpochId`], weil eine Wanduhr im
//! Konsenspfad nicht existiert: Zwei Knoten mit verschiedenen Uhren
//! kämen zu verschiedenen Zuständen. Die Epoche ist die einzige Zeit,
//! die alle gleich sehen.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::hash::Hash;
use crate::ids::{Address, EpochId, SitzungId};

/// Trennzeichen für die Kontraktadresse.
pub const DST_SITZUNGSKONTRAKT: &[u8] = b"MYELITH_SITZUNGSKONTRAKT_v1";

/// Höchstzahl der Empfänger in einer Liste.
///
/// Der Zustand eines Kontrakts geht in die Zustandsverpflichtung ein,
/// und die Prüfung läuft über die Liste. Beides muss beschränkt sein,
/// sonst hängen Speicher und Prüfkosten an einer Nutzereingabe.
pub const MAX_EMPFAENGER: usize = 256;

/// Was ein Vorhaben ausgibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, BorshSerialize, BorshDeserialize)]
pub enum Waehrung {
    /// Inferenz-Credits (vTFE), durch Burn erworben.
    Credits,
    /// MYL-Kleinstbeträge.
    Myl,
}


/// Höchstzahl der Sprossen einer Zeugenleiter.
pub const MAX_ZEUGENSTUFEN: usize = 8;

/// Eine Sprosse der Zeugenleiter: ab welchem Betrag wie viele
/// unabhängige Gateways ein externes Werkzeugergebnis bezeugt haben
/// müssen.
///
/// Das ist die Zahl, die Design-Entscheidung 1 offengelassen hat.
/// ⚑ **Sie steht bewusst nicht im Protokoll, sondern im Kontrakt**,
/// weil eine feste Zahl entweder für kleine Beträge zu teuer oder für
/// große zu billig wäre. Wer mehr riskiert, verlangt mehr Zeugen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Zeugenstufe {
    /// Ab diesem Betrag gilt die Sprosse.
    pub ab_betrag: u64,
    /// So viele unabhängige Gateways.
    pub zeugen: u32,
}

/// Die Grenzen für eine Währung.
///
/// Die drei Zahlen sind Obergrenzen. `u64::MAX` heißt „keine Grenze",
/// null heißt „nichts erlaubt"; beides ist ein gültiger Kontrakt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Grenzen {
    /// Gesamtbudget über die Laufzeit der Session (Kap. 8.2, Punkt 1).
    pub budget: u64,
    /// Höchstbetrag je Einzelvorgang, unabhängig vom Restbudget
    /// (Kap. 8.2, Punkt 2).
    pub einzellimit: u64,
    /// Ab diesem Betrag ist bestätigte Auslieferung nötig (Kap. 8.2,
    /// letzter Absatz, Anschluss an Kap. 6.4). `u64::MAX` schaltet die
    /// Kopplung ab.
    pub schwelle: u64,
    /// Wie viele Gateways ein externes Ergebnis bezeugen müssen,
    /// gestaffelt nach Betrag.
    ///
    /// Aufsteigend nach `ab_betrag`, und die Zeugenzahl darf dabei
    /// nicht fallen. Leer heißt: keine Anforderung.
    pub zeugenleiter: Vec<Zeugenstufe>,
}

impl Grenzen {
    /// Grenzen, die nichts erlauben. Der sichere Ausgangspunkt.
    pub fn gesperrt() -> Self {
        Self {
            budget: 0,
            einzellimit: 0,
            schwelle: u64::MAX,
            zeugenleiter: Vec::new(),
        }
    }

    /// Wie viele Zeugen dieser Betrag verlangt.
    ///
    /// Die höchste Sprosse, deren `ab_betrag` erreicht ist; ohne
    /// Sprosse null. Die Leiter ist auf [`MAX_ZEUGENSTUFEN`] begrenzt,
    /// die Suche also von vornherein billig.
    pub fn zeugen_fuer(&self, betrag: u64) -> u32 {
        self.zeugenleiter
            .iter()
            .filter(|s| betrag >= s.ab_betrag)
            .map(|s| s.zeugen)
            .next_back()
            .unwrap_or(0)
    }

    /// Prüft die Normalform der Zeugenleiter.
    fn pruefe_leiter(&self) -> Result<(), KontraktFehler> {
        if self.zeugenleiter.len() > MAX_ZEUGENSTUFEN {
            return Err(KontraktFehler::ZuVieleZeugenstufen {
                hatte: self.zeugenleiter.len(),
            });
        }
        for paar in self.zeugenleiter.windows(2) {
            if paar[1].ab_betrag <= paar[0].ab_betrag {
                return Err(KontraktFehler::ZeugenleiterNichtSteigend);
            }
            if paar[1].zeugen < paar[0].zeugen {
                return Err(KontraktFehler::ZeugenzahlFaellt);
            }
        }
        Ok(())
    }
}

/// Fehler beim Anlegen eines Kontrakts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KontraktFehler {
    /// Die Empfängerliste ist nicht aufsteigend sortiert.
    EmpfaengerNichtSortiert,
    /// Eine Adresse kommt mehrfach vor.
    EmpfaengerDoppelt,
    /// Mehr als [`MAX_EMPFAENGER`] Einträge.
    ZuVieleEmpfaenger {
        /// Wie viele übergeben wurden.
        hatte: usize,
    },
    /// Das Zeitfenster endet, bevor es beginnt.
    FensterVerkehrt {
        /// Beginn.
        ab: EpochId,
        /// Ende.
        bis: EpochId,
    },
    /// Mehr als [`MAX_ZEUGENSTUFEN`] Sprossen.
    ZuVieleZeugenstufen {
        /// Wie viele übergeben wurden.
        hatte: usize,
    },
    /// Die Sprossen sind nicht streng aufsteigend nach Betrag.
    ZeugenleiterNichtSteigend,
    /// Eine höhere Sprosse verlangt weniger Zeugen als eine niedrigere.
    ///
    /// ⚑ **Das ist die Form eines Versehens**, und zugleich die Form
    /// dessen, was ein Angreifer sich wünschte: Je mehr auf dem Spiel
    /// steht, desto weniger Belege.
    ZeugenzahlFaellt,
}

impl std::fmt::Display for KontraktFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmpfaengerNichtSortiert => write!(f, "Empfängerliste nicht sortiert"),
            Self::EmpfaengerDoppelt => write!(f, "Empfänger doppelt"),
            Self::ZuVieleEmpfaenger { hatte } => {
                write!(f, "{hatte} Empfänger, höchstens {MAX_EMPFAENGER}")
            }
            Self::FensterVerkehrt { ab, bis } => {
                write!(f, "Fenster von {} bis {} ist leer", ab.0, bis.0)
            }
            Self::ZuVieleZeugenstufen { hatte } => {
                write!(f, "{hatte} Zeugenstufen, höchstens {MAX_ZEUGENSTUFEN}")
            }
            Self::ZeugenleiterNichtSteigend => {
                write!(f, "Zeugenleiter nicht streng aufsteigend")
            }
            Self::ZeugenzahlFaellt => write!(f, "höhere Sprosse verlangt weniger Zeugen"),
        }
    }
}

impl std::error::Error for KontraktFehler {}

/// Der unveränderliche Teil einer Session.
///
/// ⚑ **Die Empfängerliste ist aufsteigend sortiert und
/// duplikatfrei**, und das ist keine Ordnungsliebe. Die Adresse ist der
/// Hash der Kodierung; ohne diese Normalform hätte dieselbe Menge von
/// Empfängern je nach Reihenfolge verschiedene Adressen, und zwei
/// Kontrakte mit identischer Bedeutung wären zwei verschiedene Objekte.
/// Dieselbe Injektivitätsfrage wie beim Merkle-Baum.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Sitzungskontrakt {
    /// Wer den Kontrakt gesetzt hat und ihn widerrufen darf.
    pub inhaber: Address,
    /// Unter welchem Schlüssel der Agent handeln darf.
    ///
    /// Keine Grenze, sondern die Bindung: Ohne sie könnte jeder unter
    /// fremdem Kontrakt zahlen.
    pub agent: Address,
    /// Grenzen für Inferenz-Credits.
    pub credits: Grenzen,
    /// Grenzen für MYL.
    pub myl: Grenzen,
    /// An wen überhaupt gezahlt werden darf (Kap. 8.2, Punkt 3).
    ///
    /// Leer heißt: an niemanden. Das ist die sichere Lesart einer
    /// Positivliste.
    pub empfaenger: Vec<Address>,
    /// Erste Epoche, in der die Session gilt.
    pub gueltig_ab: EpochId,
    /// Letzte Epoche, in der die Session gilt (Kap. 8.2, Punkt 4).
    pub gueltig_bis: EpochId,
    /// Höchstzahl der Schritte, die ein Agent unter diesem Kontrakt tun
    /// darf (Kap. 8.4, Abbruchbedingungen).
    ///
    /// ⚑ **Nicht dasselbe wie das Budget, obwohl beides begrenzt.** Das
    /// Budget begrenzt, was ausgegeben wird; die Schrittzahl begrenzt,
    /// wie lange gearbeitet wird. Ein Agent, der in einer Schleife
    /// nachschlägt, ohne je zu zahlen, verbraucht kein Budget und läuft
    /// trotzdem endlos.
    ///
    /// Null heißt: kein Schritt erlaubt. Wie überall hier ist die
    /// sperrende Zahl die sichere.
    pub max_schritte: u32,
}

impl Sitzungskontrakt {
    /// Legt einen Kontrakt an und prüft die Normalform.
    ///
    /// Die Empfängerliste wird **nicht** stillschweigend sortiert: Wer
    /// eine unsortierte Liste übergibt, hat sich etwas anderes gedacht
    /// als das Ergebnis, und eine Adresse, die von der Bibliothek
    /// verändert wurde, ist nicht mehr die, die der Nutzer gesehen hat.
    #[allow(clippy::too_many_arguments)]
    pub fn neu(
        inhaber: Address,
        agent: Address,
        credits: Grenzen,
        myl: Grenzen,
        empfaenger: Vec<Address>,
        gueltig_ab: EpochId,
        gueltig_bis: EpochId,
        max_schritte: u32,
    ) -> Result<Self, KontraktFehler> {
        if empfaenger.len() > MAX_EMPFAENGER {
            return Err(KontraktFehler::ZuVieleEmpfaenger { hatte: empfaenger.len() });
        }
        for paar in empfaenger.windows(2) {
            match paar[0].as_bytes().cmp(paar[1].as_bytes()) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => return Err(KontraktFehler::EmpfaengerDoppelt),
                std::cmp::Ordering::Greater => {
                    return Err(KontraktFehler::EmpfaengerNichtSortiert)
                }
            }
        }
        if gueltig_bis.0 < gueltig_ab.0 {
            return Err(KontraktFehler::FensterVerkehrt { ab: gueltig_ab, bis: gueltig_bis });
        }
        credits.pruefe_leiter()?;
        myl.pruefe_leiter()?;
        Ok(Self {
            inhaber,
            agent,
            credits,
            myl,
            empfaenger,
            gueltig_ab,
            gueltig_bis,
            max_schritte,
        })
    }

    /// Die Adresse: Hash über Trennzeichen und kanonische Kodierung.
    ///
    /// Borsh längenpräfixiert die Empfängerliste selbst, und alle
    /// übrigen Felder haben feste Breite; eine eigene Feldkodierung wie
    /// bei den Manifesten ist hier deshalb nicht nötig.
    pub fn adresse(&self) -> SitzungId {
        let mut daten = Vec::with_capacity(DST_SITZUNGSKONTRAKT.len() + 256);
        daten.extend_from_slice(DST_SITZUNGSKONTRAKT);
        daten.extend_from_slice(
            &borsh::to_vec(self).expect("Kontrakt ist stets serialisierbar"),
        );
        SitzungId::new(Hash::sha256(&daten).0)
    }

    /// Die Grenzen für eine Währung.
    pub fn grenzen(&self, waehrung: Waehrung) -> &Grenzen {
        match waehrung {
            Waehrung::Credits => &self.credits,
            Waehrung::Myl => &self.myl,
        }
    }

    /// Wie viele Gateways ein externes Ergebnis bezeugen müssen, wenn
    /// darunter dieser Betrag in dieser Währung ausgegeben wird.
    ///
    /// Das ist die Zahl, die [`beobachte`](../../myl_agent) als
    /// `verlangt` bekommt: Design-Entscheidung 1 legte fest, dass sie
    /// aus dem Kontrakt kommt und nicht aus dem Protokoll.
    pub fn zeugen_fuer(&self, waehrung: Waehrung, betrag: u64) -> u32 {
        self.grenzen(waehrung).zeugen_fuer(betrag)
    }

    /// Steht diese Adresse auf der Positivliste?
    ///
    /// Binäre Suche, weil die Liste sortiert ist.
    pub fn zahlt_an(&self, empfaenger: &Address) -> bool {
        self.empfaenger
            .binary_search_by(|a| a.as_bytes().cmp(empfaenger.as_bytes()))
            .is_ok()
    }
}

/// Der veränderliche Teil: was schon verbraucht ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, BorshSerialize, BorshDeserialize)]
pub struct Sitzungszustand {
    /// Bereits ausgegebene Credits.
    pub verbraucht_credits: u64,
    /// Bereits ausgegebene MYL-Kleinstbeträge.
    pub verbraucht_myl: u64,
    /// Vom Inhaber vorzeitig beendet.
    pub widerrufen: bool,
}

impl Sitzungszustand {
    /// Frischer Zustand: nichts verbraucht, nicht widerrufen.
    pub fn neu() -> Self {
        Self::default()
    }

    /// Was in dieser Währung schon ausgegeben ist.
    pub fn verbraucht(&self, waehrung: Waehrung) -> u64 {
        match waehrung {
            Waehrung::Credits => self.verbraucht_credits,
            Waehrung::Myl => self.verbraucht_myl,
        }
    }
}

/// Was ein Agent vorhat.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Vorhaben {
    /// Unter welchem Kontrakt.
    pub sitzung: SitzungId,
    /// Wer die Transaktion einreicht.
    pub handelnder: Address,
    /// Womit gezahlt wird.
    pub waehrung: Waehrung,
    /// Wie viel.
    pub betrag: u64,
    /// An wen.
    pub empfaenger: Address,
    /// Ob das zugrundeliegende Segment im Modus bestätigter
    /// Auslieferung gerechnet wurde (Kap. 6.4).
    pub bestaetigt_ausgeliefert: bool,
}

/// Das Ergebnis der Prüfung, für den Knoten und den Client des
/// Inhabers.
///
/// ⚑ **Nicht für den Agenten.** Was diesem gezeigt wird, liefert
/// [`Befund::fuer_agenten`], und das ist ein Bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Befund {
    /// Erlaubt.
    Erlaubt,
    /// Das Vorhaben zeigt auf einen anderen Kontrakt als den geprüften.
    FalscheSitzung,
    /// Der Einreicher ist nicht der Agent dieses Kontrakts.
    FalscherHandelnder,
    /// Der Inhaber hat die Session beendet.
    Widerrufen,
    /// Die Session hat noch nicht begonnen.
    NochNichtGueltig {
        /// Laufende Epoche.
        jetzt: EpochId,
        /// Beginn.
        ab: EpochId,
    },
    /// Das Zeitfenster ist abgelaufen.
    Abgelaufen {
        /// Laufende Epoche.
        jetzt: EpochId,
        /// Ende.
        bis: EpochId,
    },
    /// Betrag null.
    NullBetrag,
    /// Der Empfänger steht nicht auf der Positivliste.
    EmpfaengerNichtGelistet,
    /// Über dem Einzeltransaktionslimit.
    EinzellimitUeberschritten {
        /// Die Grenze.
        limit: u64,
    },
    /// Das Restbudget reicht nicht.
    BudgetErschoepft {
        /// Was noch da ist.
        rest: u64,
    },
    /// Über der Schwelle, aber ohne bestätigte Auslieferung.
    BestaetigungNoetig {
        /// Ab welchem Betrag.
        schwelle: u64,
    },
}

/// Was der Agent erfährt.
///
/// ⚑ **Genau ein Bit, und mehr ist keine Höflichkeit, sondern ein
/// Leck.** Kap. 8.2 verlangt, dass die Grenzen für den Agenten nicht
/// lesbar sind; der Fehlerkanal ist der Weg, über den sie es sonst
/// würden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agentenbefund {
    /// Durchgelassen.
    Erlaubt,
    /// Abgelehnt, ohne Grund.
    Abgelehnt,
}

impl Befund {
    /// Erlaubt?
    pub fn erlaubt(&self) -> bool {
        matches!(self, Self::Erlaubt)
    }

    /// Die Sicht des Agenten: ein Bit, ohne Zahlen.
    pub fn fuer_agenten(&self) -> Agentenbefund {
        match self {
            Self::Erlaubt => Agentenbefund::Erlaubt,
            _ => Agentenbefund::Abgelehnt,
        }
    }
}

impl std::fmt::Display for Befund {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Erlaubt => write!(f, "erlaubt"),
            Self::FalscheSitzung => write!(f, "Vorhaben zeigt auf einen anderen Kontrakt"),
            Self::FalscherHandelnder => write!(f, "Einreicher ist nicht der Agent"),
            Self::Widerrufen => write!(f, "Session widerrufen"),
            Self::NochNichtGueltig { jetzt, ab } => {
                write!(f, "Epoche {}, Session beginnt erst {}", jetzt.0, ab.0)
            }
            Self::Abgelaufen { jetzt, bis } => {
                write!(f, "Epoche {}, Session endete {}", jetzt.0, bis.0)
            }
            Self::NullBetrag => write!(f, "Betrag null"),
            Self::EmpfaengerNichtGelistet => write!(f, "Empfänger nicht gelistet"),
            Self::EinzellimitUeberschritten { limit } => {
                write!(f, "über dem Einzellimit {limit}")
            }
            Self::BudgetErschoepft { rest } => write!(f, "nur noch {rest} übrig"),
            Self::BestaetigungNoetig { schwelle } => {
                write!(f, "ab {schwelle} nur mit bestätigter Auslieferung")
            }
        }
    }
}

/// Prüft ein Vorhaben gegen Kontrakt und Zustand.
///
/// **Total, ganzzahlig, ohne eingabeabhängige Allokation.** Diese
/// Funktion läuft im Konsenspfad, also gelten dieselben Regeln wie
/// überall sonst dort: Jede Eingabe hat ein Ergebnis, kein Zweig kann
/// abstürzen, und die Kosten hängen nur an der Länge der
/// Empfängerliste, die durch [`MAX_EMPFAENGER`] beschränkt ist.
///
/// **Die Reihenfolge der Prüfungen verrät nichts**, weil der Agent
/// ohnehin nur [`Agentenbefund`] sieht. Sie ist danach geordnet, was
/// grundsätzlicher ist: erst wer und wann, dann was und wie viel.
pub fn pruefe(
    kontrakt: &Sitzungskontrakt,
    zustand: &Sitzungszustand,
    jetzt: EpochId,
    vorhaben: &Vorhaben,
) -> Befund {
    if kontrakt.adresse() != vorhaben.sitzung {
        return Befund::FalscheSitzung;
    }
    if vorhaben.handelnder != kontrakt.agent {
        return Befund::FalscherHandelnder;
    }
    if zustand.widerrufen {
        return Befund::Widerrufen;
    }
    if jetzt.0 < kontrakt.gueltig_ab.0 {
        return Befund::NochNichtGueltig { jetzt, ab: kontrakt.gueltig_ab };
    }
    if jetzt.0 > kontrakt.gueltig_bis.0 {
        return Befund::Abgelaufen { jetzt, bis: kontrakt.gueltig_bis };
    }
    if vorhaben.betrag == 0 {
        return Befund::NullBetrag;
    }
    if !kontrakt.zahlt_an(&vorhaben.empfaenger) {
        return Befund::EmpfaengerNichtGelistet;
    }

    let grenzen = kontrakt.grenzen(vorhaben.waehrung);
    if vorhaben.betrag > grenzen.einzellimit {
        return Befund::EinzellimitUeberschritten { limit: grenzen.einzellimit };
    }
    // Sättigend, weil ein Verbrauch über dem Budget nicht vorkommen
    // darf und ein Unterlauf hier das Budget aufblähen würde.
    let rest = grenzen.budget.saturating_sub(zustand.verbraucht(vorhaben.waehrung));
    if vorhaben.betrag > rest {
        return Befund::BudgetErschoepft { rest };
    }
    if vorhaben.betrag >= grenzen.schwelle && !vorhaben.bestaetigt_ausgeliefert {
        return Befund::BestaetigungNoetig { schwelle: grenzen.schwelle };
    }
    Befund::Erlaubt
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{from_slice, to_vec};

    fn adr(byte: u8) -> Address {
        Address::new([byte; 32])
    }

    fn grenzen(budget: u64, einzel: u64, schwelle: u64) -> Grenzen {
        Grenzen { budget, einzellimit: einzel, schwelle, zeugenleiter: Vec::new() }
    }

    fn leiter(stufen: &[(u64, u32)]) -> Vec<Zeugenstufe> {
        stufen
            .iter()
            .map(|(ab_betrag, zeugen)| Zeugenstufe { ab_betrag: *ab_betrag, zeugen: *zeugen })
            .collect()
    }

    /// Ein brauchbarer Kontrakt: Agent 2 darf für Inhaber 1 bis 1000
    /// Credits ausgeben, höchstens 300 auf einmal, ab 250 nur mit
    /// bestätigter Auslieferung, an die Empfänger 10 und 20, in den
    /// Epochen 5 bis 9.
    fn kontrakt() -> Sitzungskontrakt {
        Sitzungskontrakt::neu(
            adr(1),
            adr(2),
            grenzen(1_000, 300, 250),
            grenzen(500, 100, 50),
            vec![adr(10), adr(20)],
            EpochId(5),
            EpochId(9),16,
        )
        .expect("gültiger Kontrakt")
    }

    fn vorhaben(k: &Sitzungskontrakt, betrag: u64) -> Vorhaben {
        Vorhaben {
            sitzung: k.adresse(),
            handelnder: adr(2),
            waehrung: Waehrung::Credits,
            betrag,
            empfaenger: adr(10),
            bestaetigt_ausgeliefert: false,
        }
    }

    #[test]
    fn ein_gewoehnlicher_vorgang_geht_durch() {
        let k = kontrakt();
        let z = Sitzungszustand::neu();
        assert_eq!(pruefe(&k, &z, EpochId(7), &vorhaben(&k, 100)), Befund::Erlaubt);
    }

    #[test]
    fn adresse_ist_deterministisch_und_trennt_kontrakte() {
        let a = kontrakt();
        let b = kontrakt();
        assert_eq!(a.adresse(), b.adresse());

        let mehr = Sitzungskontrakt::neu(
            adr(1),
            adr(2),
            grenzen(1_001, 300, 250), // eine Einheit mehr Budget
            grenzen(500, 100, 50),
            vec![adr(10), adr(20)],
            EpochId(5),
            EpochId(9),16,
        )
        .expect("gültig");
        assert_ne!(a.adresse(), mehr.adresse());
    }

    /// ⚑ **Der Kern der Phase, als Test.** Ein Agent kann einen
    /// Kontrakt nicht ändern. Er kann nur einen *anderen* bauen, und
    /// ein anderer hat eine andere Adresse. Das Vorhaben zeigt aber auf
    /// die Adresse, unter der das Ledger den echten Kontrakt führt,
    /// also läuft die Prüfung ins Leere.
    ///
    /// **Geprüft wird Unveränderlichkeit, nicht Geheimhaltung.** Das
    /// ist der Unterschied zwischen einem Test, der aus dem richtigen
    /// Grund grün ist, und einem, der es aus dem falschen ist.
    #[test]
    fn ein_agent_kann_die_grenzen_nicht_weiten() {
        let echt = kontrakt();
        let z = Sitzungszustand::neu();
        let adresse_im_ledger = echt.adresse();

        // Der Agent baut sich einen Kontrakt mit hundertfachem Budget
        // und ohne Empfängerbindung.
        let gefaelscht = Sitzungskontrakt::neu(
            adr(1),
            adr(2),
            grenzen(100_000, 100_000, u64::MAX),
            grenzen(100_000, 100_000, u64::MAX),
            vec![adr(10), adr(20), adr(99)],
            EpochId(0),
            EpochId(u64::MAX),16,
        )
        .expect("gültig, nur eben nicht der Kontrakt des Inhabers");

        assert_ne!(gefaelscht.adresse(), adresse_im_ledger);

        // Der Versuch, unter der echten Sitzungsadresse mit den
        // gefälschten Grenzen zu zahlen.
        let v = Vorhaben {
            sitzung: adresse_im_ledger,
            handelnder: adr(2),
            waehrung: Waehrung::Credits,
            betrag: 50_000,
            empfaenger: adr(99),
            bestaetigt_ausgeliefert: true,
        };
        assert_eq!(pruefe(&gefaelscht, &z, EpochId(7), &v), Befund::FalscheSitzung);

        // Und gegen den echten Kontrakt greifen dessen Grenzen.
        assert_eq!(
            pruefe(&echt, &z, EpochId(7), &v),
            Befund::EmpfaengerNichtGelistet
        );
    }

    /// ⚑ Gegenprobe zur Injektivität, dieselbe Frage wie beim
    /// Merkle-Baum: Ohne die Normalform hätte dieselbe Menge von
    /// Empfängern zwei Adressen. Deshalb prüft `neu` die Sortierung,
    /// statt sie herzustellen.
    #[test]
    fn dieselbe_menge_in_anderer_reihenfolge_ergaebe_eine_andere_adresse() {
        let sortiert = Sitzungskontrakt {
            inhaber: adr(1),
            agent: adr(2),
            credits: grenzen(1_000, 300, 250),
            myl: Grenzen::gesperrt(),
            empfaenger: vec![adr(10), adr(20)],
            gueltig_ab: EpochId(5),
            gueltig_bis: EpochId(9),
            max_schritte: 16,
        };
        let vertauscht = Sitzungskontrakt {
            empfaenger: vec![adr(20), adr(10)],
            ..sortiert.clone()
        };
        assert_ne!(sortiert.adresse(), vertauscht.adresse());

        // Und genau darum lässt der Konstruktor die zweite nicht zu.
        let gebaut = Sitzungskontrakt::neu(
            adr(1),
            adr(2),
            grenzen(1_000, 300, 250),
            Grenzen::gesperrt(),
            vec![adr(20), adr(10)],
            EpochId(5),
            EpochId(9),16,
        );
        assert_eq!(gebaut, Err(KontraktFehler::EmpfaengerNichtSortiert));
    }

    #[test]
    fn die_empfaengerliste_muss_in_normalform_sein() {
        let bauen = |liste: Vec<Address>| {
            Sitzungskontrakt::neu(
                adr(1),
                adr(2),
                Grenzen::gesperrt(),
                Grenzen::gesperrt(),
                liste,
                EpochId(0),
                EpochId(1),16,
            )
        };
        assert_eq!(bauen(vec![adr(9), adr(3)]), Err(KontraktFehler::EmpfaengerNichtSortiert));
        assert_eq!(bauen(vec![adr(3), adr(3)]), Err(KontraktFehler::EmpfaengerDoppelt));
        assert!(bauen(vec![adr(3), adr(9)]).is_ok());
        assert!(bauen(Vec::new()).is_ok());

        let zu_viele: Vec<Address> = (0..=MAX_EMPFAENGER)
            .map(|i| {
                let mut b = [0u8; 32];
                b[0..8].copy_from_slice(&(i as u64).to_be_bytes());
                Address::new(b)
            })
            .collect();
        assert_eq!(
            bauen(zu_viele),
            Err(KontraktFehler::ZuVieleEmpfaenger { hatte: MAX_EMPFAENGER + 1 })
        );
    }

    #[test]
    fn ein_leeres_fenster_wird_abgelehnt() {
        let k = Sitzungskontrakt::neu(
            adr(1),
            adr(2),
            Grenzen::gesperrt(),
            Grenzen::gesperrt(),
            Vec::new(),
            EpochId(9),
            EpochId(8),16,
        );
        assert_eq!(k, Err(KontraktFehler::FensterVerkehrt { ab: EpochId(9), bis: EpochId(8) }));

        // Eine einzige Epoche ist ein gültiges Fenster.
        assert!(Sitzungskontrakt::neu(
            adr(1),
            adr(2),
            Grenzen::gesperrt(),
            Grenzen::gesperrt(),
            Vec::new(),
            EpochId(9),
            EpochId(9),16,
        )
        .is_ok());
    }

    #[test]
    fn ein_leerer_kontrakt_zahlt_an_niemanden() {
        let k = Sitzungskontrakt::neu(
            adr(1),
            adr(2),
            grenzen(1_000, 1_000, u64::MAX),
            Grenzen::gesperrt(),
            Vec::new(),
            EpochId(0),
            EpochId(10),16,
        )
        .expect("gültig");
        assert!(!k.zahlt_an(&adr(10)));
        let v = Vorhaben { sitzung: k.adresse(), ..vorhaben(&kontrakt(), 1) };
        assert_eq!(
            pruefe(&k, &Sitzungszustand::neu(), EpochId(5), &v),
            Befund::EmpfaengerNichtGelistet
        );
    }

    #[test]
    fn das_fenster_greift_an_beiden_raendern() {
        let k = kontrakt();
        let z = Sitzungszustand::neu();
        assert_eq!(
            pruefe(&k, &z, EpochId(4), &vorhaben(&k, 10)),
            Befund::NochNichtGueltig { jetzt: EpochId(4), ab: EpochId(5) }
        );
        assert_eq!(pruefe(&k, &z, EpochId(5), &vorhaben(&k, 10)), Befund::Erlaubt);
        assert_eq!(pruefe(&k, &z, EpochId(9), &vorhaben(&k, 10)), Befund::Erlaubt);
        assert_eq!(
            pruefe(&k, &z, EpochId(10), &vorhaben(&k, 10)),
            Befund::Abgelaufen { jetzt: EpochId(10), bis: EpochId(9) }
        );
    }

    #[test]
    fn nur_der_agent_darf_handeln_und_nur_der_inhaber_widerrufen() {
        let k = kontrakt();
        let z = Sitzungszustand::neu();
        let fremd = Vorhaben { handelnder: adr(3), ..vorhaben(&k, 10) };
        assert_eq!(pruefe(&k, &z, EpochId(7), &fremd), Befund::FalscherHandelnder);

        // Auch der Inhaber selbst ist nicht der Agent.
        let inhaber = Vorhaben { handelnder: adr(1), ..vorhaben(&k, 10) };
        assert_eq!(pruefe(&k, &z, EpochId(7), &inhaber), Befund::FalscherHandelnder);

        let widerrufen = Sitzungszustand { widerrufen: true, ..Sitzungszustand::neu() };
        assert_eq!(pruefe(&k, &widerrufen, EpochId(7), &vorhaben(&k, 10)), Befund::Widerrufen);
    }

    #[test]
    fn einzellimit_und_budget_greifen_getrennt() {
        let k = kontrakt();
        let leer = Sitzungszustand::neu();
        // Bestätigt, damit hier nur die Grenzen wirken und nicht die
        // Schwelle: 300 und 301 liegen beide über den 250 aus dem
        // Kontrakt.
        let gross = |b: u64| Vorhaben { bestaetigt_ausgeliefert: true, ..vorhaben(&k, b) };
        assert_eq!(pruefe(&k, &leer, EpochId(7), &gross(300)), Befund::Erlaubt);
        assert_eq!(
            pruefe(&k, &leer, EpochId(7), &gross(301)),
            Befund::EinzellimitUeberschritten { limit: 300 }
        );

        // 900 verbraucht, 100 übrig: 100 geht, 101 nicht — und beides
        // liegt unter dem Einzellimit.
        let fast_leer = Sitzungszustand { verbraucht_credits: 900, ..Sitzungszustand::neu() };
        assert_eq!(pruefe(&k, &fast_leer, EpochId(7), &vorhaben(&k, 100)), Befund::Erlaubt);
        assert_eq!(
            pruefe(&k, &fast_leer, EpochId(7), &vorhaben(&k, 101)),
            Befund::BudgetErschoepft { rest: 100 }
        );
    }

    /// Ein Verbrauch über dem Budget darf nicht vorkommen; kommt er
    /// doch vor, ergibt die Subtraktion null und nicht `u64::MAX`.
    #[test]
    fn ein_ueberzogener_verbrauch_laeuft_nicht_unter() {
        let k = kontrakt();
        let ueberzogen = Sitzungszustand { verbraucht_credits: 5_000, ..Sitzungszustand::neu() };
        assert_eq!(
            pruefe(&k, &ueberzogen, EpochId(7), &vorhaben(&k, 1)),
            Befund::BudgetErschoepft { rest: 0 }
        );
    }

    #[test]
    fn die_schwelle_greift_ab_dem_betrag_nicht_erst_darueber() {
        let k = kontrakt(); // Schwelle 250
        let z = Sitzungszustand::neu();
        assert_eq!(pruefe(&k, &z, EpochId(7), &vorhaben(&k, 249)), Befund::Erlaubt);
        assert_eq!(
            pruefe(&k, &z, EpochId(7), &vorhaben(&k, 250)),
            Befund::BestaetigungNoetig { schwelle: 250 }
        );
        let bestaetigt = Vorhaben { bestaetigt_ausgeliefert: true, ..vorhaben(&k, 250) };
        assert_eq!(pruefe(&k, &z, EpochId(7), &bestaetigt), Befund::Erlaubt);
    }

    #[test]
    fn null_ist_kein_betrag() {
        let k = kontrakt();
        assert_eq!(
            pruefe(&k, &Sitzungszustand::neu(), EpochId(7), &vorhaben(&k, 0)),
            Befund::NullBetrag
        );
    }

    /// ⚑ Beide Währungen gelten, und sie haben **getrennte** Grenzen.
    /// Bis zum 2026-08-28 wies der Kontrakt jedes MYL-Vorhaben ab, weil
    /// es im Ledger keine Überweisung gab (Fund 83).
    #[test]
    fn myl_und_credits_haben_getrennte_grenzen() {
        let k = kontrakt(); // Credits 1000/300/250, MYL 500/100/50
        let z = Sitzungszustand::neu();

        // 200 in MYL liegt über dem dortigen Einzellimit von 100.
        let v = Vorhaben { waehrung: Waehrung::Myl, ..vorhaben(&k, 200) };
        assert_eq!(
            pruefe(&k, &z, EpochId(7), &v),
            Befund::EinzellimitUeberschritten { limit: 100 }
        );
        // Dieselben 200 gehen in Credits durch: Einzellimit 300, und
        // die Schwelle von 250 ist nicht erreicht.
        assert_eq!(pruefe(&k, &z, EpochId(7), &vorhaben(&k, 200)), Befund::Erlaubt);

        // Und der Verbrauch zählt je Währung getrennt.
        let verbraucht = Sitzungszustand { verbraucht_myl: 450, ..Sitzungszustand::neu() };
        let klein = Vorhaben { waehrung: Waehrung::Myl, ..vorhaben(&k, 60) };
        assert_eq!(
            pruefe(&k, &verbraucht, EpochId(7), &klein),
            Befund::BudgetErschoepft { rest: 50 }
        );
        assert_eq!(pruefe(&k, &verbraucht, EpochId(7), &vorhaben(&k, 60)), Befund::Erlaubt);
    }

    /// ⚑ **Der Fehlerkanal ist der Weg, über den die Grenzen sonst
    /// lesbar würden.** Jeder Befund außer `Erlaubt` wird für den
    /// Agenten zu genau einem Bit, ohne Zahl.
    #[test]
    fn fuer_agenten_gibt_genau_ein_bit_heraus() {
        let alle = [
            Befund::Erlaubt,
            Befund::FalscheSitzung,
            Befund::FalscherHandelnder,
            Befund::Widerrufen,
            Befund::NochNichtGueltig { jetzt: EpochId(1), ab: EpochId(2) },
            Befund::Abgelaufen { jetzt: EpochId(3), bis: EpochId(2) },
            Befund::NullBetrag,
            Befund::EmpfaengerNichtGelistet,
            Befund::EinzellimitUeberschritten { limit: 300 },
            Befund::BudgetErschoepft { rest: 42 },
            Befund::BestaetigungNoetig { schwelle: 250 },
        ];
        for b in alle {
            let erwartet = if b == Befund::Erlaubt {
                Agentenbefund::Erlaubt
            } else {
                Agentenbefund::Abgelehnt
            };
            assert_eq!(b.fuer_agenten(), erwartet, "{b:?}");
            assert_eq!(b.erlaubt(), b == Befund::Erlaubt, "{b:?}");
        }
    }

    /// Gegenprobe dazu: Der **reiche** Befund trägt die Zahlen sehr
    /// wohl, denn der Client des Inhabers muss erklären können, warum
    /// abgelehnt wurde. Zwei Kanäle, und nur einer führt zum Agenten.
    #[test]
    fn der_reiche_befund_traegt_die_zahlen() {
        let k = kontrakt();
        let z = Sitzungszustand { verbraucht_credits: 950, ..Sitzungszustand::neu() };
        let b = pruefe(&k, &z, EpochId(7), &vorhaben(&k, 60));
        assert_eq!(b, Befund::BudgetErschoepft { rest: 50 });
        assert!(b.to_string().contains("50"));
        assert_eq!(b.fuer_agenten(), Agentenbefund::Abgelehnt);
    }

    #[test]
    fn borsh_ist_ein_rundweg() {
        let k = kontrakt();
        let zurueck: Sitzungskontrakt = from_slice(&to_vec(&k).expect("ser")).expect("de");
        assert_eq!(k, zurueck);
        assert_eq!(k.adresse(), zurueck.adresse());

        let z = Sitzungszustand { verbraucht_credits: 7, verbraucht_myl: 9, widerrufen: true };
        let z2: Sitzungszustand = from_slice(&to_vec(&z).expect("ser")).expect("de");
        assert_eq!(z, z2);

        let v = vorhaben(&k, 5);
        let v2: Vorhaben = from_slice(&to_vec(&v).expect("ser")).expect("de");
        assert_eq!(v, v2);
    }

    /// ⚑ Design-Entscheidung 1 ließ offen, wie viele Gateways
    /// übereinstimmen müssen, und legte fest, dass die Zahl aus dem
    /// Kontrakt kommt. Hier ist sie: eine Leiter, keine Konstante.
    #[test]
    fn die_zeugenleiter_staffelt_nach_betrag() {
        let mut g = grenzen(1_000_000, 1_000_000, u64::MAX);
        g.zeugenleiter = leiter(&[(0, 1), (1_000, 2), (100_000, 5)]);
        let k = Sitzungskontrakt::neu(
            adr(1),
            adr(2),
            g,
            Grenzen::gesperrt(),
            vec![adr(10)],
            EpochId(0),
            EpochId(10),16,
        )
        .expect("gültig");

        assert_eq!(k.zeugen_fuer(Waehrung::Credits, 0), 1);
        assert_eq!(k.zeugen_fuer(Waehrung::Credits, 999), 1);
        assert_eq!(k.zeugen_fuer(Waehrung::Credits, 1_000), 2);
        assert_eq!(k.zeugen_fuer(Waehrung::Credits, 99_999), 2);
        assert_eq!(k.zeugen_fuer(Waehrung::Credits, 100_000), 5);
        assert_eq!(k.zeugen_fuer(Waehrung::Credits, u64::MAX), 5);

        // Ohne Leiter verlangt nichts etwas.
        assert_eq!(k.zeugen_fuer(Waehrung::Myl, u64::MAX), 0);
    }

    /// Eine Leiter, die ganz oben anfängt, verlangt unterhalb nichts.
    /// Das ist kein Versehen, sondern der Sinn einer Staffelung.
    #[test]
    fn unterhalb_der_ersten_sprosse_verlangt_die_leiter_nichts() {
        let mut g = Grenzen::gesperrt();
        g.zeugenleiter = leiter(&[(500, 3)]);
        assert_eq!(g.zeugen_fuer(499), 0);
        assert_eq!(g.zeugen_fuer(500), 3);
    }

    #[test]
    fn eine_leiter_die_faellt_wird_abgelehnt() {
        let bauen = |stufen: &[(u64, u32)]| {
            let mut g = Grenzen::gesperrt();
            g.zeugenleiter = leiter(stufen);
            Sitzungskontrakt::neu(
                adr(1),
                adr(2),
                g,
                Grenzen::gesperrt(),
                Vec::new(),
                EpochId(0),
                EpochId(1),16,
            )
        };
        assert!(bauen(&[(0, 1), (10, 3)]).is_ok());
        assert!(bauen(&[(0, 3), (10, 3)]).is_ok(), "gleich bleiben darf sie");
        assert_eq!(bauen(&[(10, 1), (0, 3)]), Err(KontraktFehler::ZeugenleiterNichtSteigend));
        assert_eq!(bauen(&[(0, 1), (0, 3)]), Err(KontraktFehler::ZeugenleiterNichtSteigend));
        assert_eq!(bauen(&[(0, 5), (10, 2)]), Err(KontraktFehler::ZeugenzahlFaellt));

        let zu_viele: Vec<(u64, u32)> =
            (0..=MAX_ZEUGENSTUFEN).map(|i| (i as u64, i as u32)).collect();
        assert_eq!(
            bauen(&zu_viele),
            Err(KontraktFehler::ZuVieleZeugenstufen { hatte: MAX_ZEUGENSTUFEN + 1 })
        );
    }

    /// Die Leiter geht in die Adresse ein wie jedes andere Feld: Wer
    /// die Zeugenzahl senkt, hat einen anderen Kontrakt.
    #[test]
    fn eine_andere_leiter_ist_ein_anderer_kontrakt() {
        let bauen = |stufen: &[(u64, u32)]| {
            let mut g = grenzen(1_000, 300, u64::MAX);
            g.zeugenleiter = leiter(stufen);
            Sitzungskontrakt::neu(
                adr(1),
                adr(2),
                g,
                Grenzen::gesperrt(),
                vec![adr(10)],
                EpochId(0),
                EpochId(10),16,
            )
            .expect("gültig")
        };
        assert_ne!(bauen(&[(0, 3)]).adresse(), bauen(&[(0, 1)]).adresse());
        assert_eq!(bauen(&[(0, 3)]).adresse(), bauen(&[(0, 3)]).adresse());
    }

    #[test]
    fn die_positivliste_findet_nur_was_darin_steht() {
        let k = kontrakt();
        assert!(k.zahlt_an(&adr(10)));
        assert!(k.zahlt_an(&adr(20)));
        assert!(!k.zahlt_an(&adr(11)));
        assert!(!k.zahlt_an(&adr(0)));
        assert!(!k.zahlt_an(&adr(255)));
    }
}
