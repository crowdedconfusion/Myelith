//! Der Sitzungskontrakt als Zugangsschlüssel (Stufe 2).
//!
//! # Die Entscheidung dahinter (G2, 2026-08-29)
//!
//! **Der Zugangsschlüssel ist ein Session-Kontrakt und kein
//! Datenbankmerkmal.** Ein API-Schlüssel wäre eine Zeile in einer
//! Tabelle, die der Gateway-Betreiber führt; ein Kontrakt steht im
//! Konsens, und **wer ihn widerruft, widerruft ihn für alle Gateways
//! zugleich**. Das ist der Unterschied zwischen einem Betreiber, dem
//! man vertrauen muss, und einer Regel, die jeder nachrechnen kann.
//!
//! # ⚑ Der Agent ist eine Adresse, und eine Adresse gibt keinen Schlüssel her
//!
//! `Sitzungskontrakt::agent` ist eine [`Address`], also `SHA-256` über
//! einen öffentlichen Schlüssel. **Aus einem Hash folgt kein Urbild**,
//! also kann das Gateway mit dem Kontrakt allein keine Unterschrift
//! prüfen.
//!
//! Dieselbe Stelle hat dieses Projekt schon zweimal getroffen: Fund 109
//! (`PoIBundle` nannte einen Pod, den die Zuteilung nicht kannte) und
//! Glied 2 von Punkt 40 (`verify_bundle_signature` hatte die Schlüssel
//! nicht). **Die Lösung ist dieselbe:** Der Anfragende bringt seinen
//! öffentlichen Schlüssel mit.
//!
//! ⚑ **Verglichen wird er an genau einer Stelle**, und die stand schon
//! da: [`myl_types::sitzung::pruefe`] hält `vorhaben.handelnder` gegen
//! `kontrakt.agent`, und `handelnder` wird aus dem **mitgebrachten
//! Schlüssel** abgeleitet. Ein falscher Schlüssel ergibt eine andere
//! Adresse und fällt dort durch.
//!
//! ⚑ **Der erste Entwurf hatte den Vergleich zusätzlich hier**, und die
//! Gegenprobe hat ihn als tot entlarvt: Ausgebaut blieben alle Tests
//! grün, weil `pruefe` ihn ohnehin trägt. **Eine Prüfung, die nichts
//! auswählt, ist schlimmer als keine**, denn sie sieht nach Schutz aus
//! und kann mit ihrer Zwillingsprüfung auseinanderlaufen.
//!
//! # ⚑ Genau ein Bit nach draussen, und das ist die halbe Stufe
//!
//! Kap. 8.2 verlangt, dass die Grenzen eines Kontrakts für den Agenten
//! nicht lesbar sind, und `Agentenbefund` setzt das im Ledger um.
//! **Am Gateway ist dieselbe Regel schärfer**, denn hier fragt nicht
//! ein Agent unter seinem eigenen Kontrakt, sondern **irgendwer**.
//!
//! Antwortete die Tür verschieden auf „diesen Kontrakt gibt es nicht",
//! „er ist widerrufen" und „sein Budget ist leer", dann wäre sie ein
//! **Auskunftsdienst über fremde Kontrakte**: Wer Adressen durchprobiert,
//! erführe, welche existieren und wie es um sie steht. Das ist das
//! Abtasten, gegen das Stufe 2 gebaut ist.
//!
//! # ⚑ Und deshalb wird immer gleich viel gearbeitet
//!
//! Eine gleiche Antwort genügt nicht, wenn der Weg dorthin verschieden
//! lang ist. Eine Unterschrift zu prüfen kostet eine Paarung; einen
//! Kontrakt nachzuschlagen kostet fast nichts. **Wer bei unbekanntem
//! Kontrakt die Prüfung überspränge, verriete über die Zeit, was er
//! über den Inhalt verschweigt.**
//!
//! [`pruefe_zugang`] prüft die Unterschrift deshalb **immer**, auch
//! gegen einen Kontrakt, den es nicht gibt: dann gegen den mitgelieferten
//! Schlüssel, dessen Ergebnis danach ohnehin verworfen wird. Die Arbeit
//! ist dieselbe, die Antwort ist dieselbe.
//!
//! **Was das nicht leistet:** Es ist keine Konstantzeit im
//! kryptografischen Sinn. Die Verzweigungen dieses Moduls sind
//! ausgeglichen; was `blst` intern tut, ist nicht Gegenstand dieser
//! Zusicherung, und das gehört gesagt statt behauptet.

use myl_types::bls::{BlsPublicKey, BlsSignature};
use myl_types::hash::Hash;
use myl_types::ids::{Address, EpochId, SitzungId};
use myl_types::sitzung::{pruefe, Sitzungskontrakt, Sitzungszustand, Vorhaben, Waehrung};

/// Trennstring der Zugangsbotschaft.
///
/// Eigenes Präfix aus demselben Grund wie überall: Ohne Trennung wäre
/// eine Unterschrift aus einem anderen Zusammenhang hier
/// wiederverwendbar.
pub const DST_ZUGANG: &[u8] = b"MYELITH_GATEWAY_ZUGANG_v1";

/// Was der Anfragende vorlegt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zugangsanfrage {
    /// Unter welchem Kontrakt.
    pub sitzung: SitzungId,
    /// Der öffentliche Schlüssel des Handelnden.
    ///
    /// ⚑ **Er muss mitkommen**, weil `Sitzungskontrakt::agent` eine
    /// Adresse ist und aus einem Hash kein Schlüssel folgt. Siehe den
    /// Modulkopf.
    pub schluessel: BlsPublicKey,
    /// Die laufende Nummer dieses Handelnden.
    ///
    /// ⚑ **Gegen Wiedereinspielung.** Ohne sie wäre eine einmal
    /// gesehene Anfrage beliebig oft gültig, und jede Wiederholung
    /// bekäme eine **neue** Sitzungsnummer und damit einen neuen
    /// Arbeitsauftrag. Dieselbe Regel wie im Ledger, strenge
    /// Gleichheit statt Fenster.
    pub nummer: u64,
    /// Die Unterschrift über [`zugangsbotschaft`].
    pub unterschrift: BlsSignature,
}

/// Die kanonische Botschaft, die der Handelnde unterschreibt.
///
/// **Aufbau:** `DST ‖ sitzung ‖ u64_le(nummer) ‖ u64_le(epoche) ‖
/// sha256(anfrage)`: feste Feldbreiten in fester Reihenfolge.
///
/// ⚑ **Die Anfrage geht als Hash ein und muss es.** Ohne sie wäre eine
/// Unterschrift von der Frage gelöst: Wer eine gültige Anfrage abfängt,
/// hängte einen anderen Prompt daran und liesse ihn auf fremde Kosten
/// rechnen.
///
/// ⚑ **Und die Epoche gehört dazu**, sonst gälte dieselbe Unterschrift
/// über die ganze Laufzeit des Kontrakts hinweg. Die Nummer allein
/// bindet an die Reihenfolge, nicht an die Zeit.
pub fn zugangsbotschaft(
    sitzung: SitzungId,
    nummer: u64,
    epoche: EpochId,
    anfrage: &[u8],
) -> Vec<u8> {
    let mut msg = Vec::with_capacity(DST_ZUGANG.len() + 32 + 8 + 8 + 32);
    msg.extend_from_slice(DST_ZUGANG);
    msg.extend_from_slice(sitzung.as_bytes());
    msg.extend_from_slice(&nummer.to_le_bytes());
    msg.extend_from_slice(&epoche.0.to_le_bytes());
    msg.extend_from_slice(Hash::sha256(anfrage).as_bytes());
    msg
}

/// Was die Tür nach draussen sagt.
///
/// ⚑ **Genau zwei Werte, und das ist keine Sparsamkeit.** Jeder weitere
/// wäre eine Auskunft über einen fremden Kontrakt; siehe den Modulkopf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zugangsbefund {
    /// Durchgelassen.
    Erlaubt,
    /// Abgelehnt, ohne Grund.
    Abgelehnt,
}

impl Zugangsbefund {
    /// Durchgelassen?
    pub fn erlaubt(&self) -> bool {
        matches!(self, Self::Erlaubt)
    }
}

/// Prüft, ob eine Anfrage durchgelassen wird.
///
/// `kontrakt` und `zustand` sind `None`, wenn es die Sitzung nicht gibt.
///
/// **Die Prüfkette, und sie läuft immer ganz durch:**
///
/// 1. Der Schlüssel passt zur Agentenadresse des Kontrakts.
/// 2. Die Unterschrift gilt über [`zugangsbotschaft`].
/// 3. `myl_types::sitzung::pruefe` sagt `Erlaubt`.
///
/// ⚑ **Kein früher Ausstieg.** Jeder Zweig rechnet dieselbe Paarung,
/// auch der, dessen Ergebnis schon feststeht; siehe den Modulkopf.
pub fn pruefe_zugang(
    kontrakt: Option<&Sitzungskontrakt>,
    zustand: Option<&Sitzungszustand>,
    jetzt: EpochId,
    anfrage: &Zugangsanfrage,
    rumpf: &[u8],
) -> Zugangsbefund {
    // ⚑ **Kein früher Ausstieg, und zwar strukturell.** Beide Teile
    // werden unbedingt gerechnet und erst danach verbunden; es gibt
    // keinen Zweig, den man entfernen könnte, um Arbeit zu sparen.
    // Eine gleiche Antwort genügt nicht, wenn der Weg dorthin
    // verschieden lang ist.
    let botschaft = zugangsbotschaft(anfrage.sitzung, anfrage.nummer, jetzt, rumpf);
    let unterschrift_gilt = anfrage
        .schluessel
        .verify(&botschaft, &anfrage.unterschrift);

    // ⚑ **Ohne Kontrakt wird gegen einen leeren gerechnet**, nicht
    // gesprungen. Sein Befund ist immer `FalscherHandelnder`, weil
    // seine Agentenadresse null ist; das Ergebnis steht fest, die
    // Arbeit ist dieselbe.
    let leer = leerer_kontrakt();
    let leer_zustand = Sitzungszustand::neu();
    let (k, z) = match (kontrakt, zustand) {
        (Some(k), Some(z)) => (k, z),
        _ => (&leer, &leer_zustand),
    };
    let kontraktbefund = pruefe(k, z, jetzt, &vorhaben_fuer(k, anfrage));

    if unterschrift_gilt && kontraktbefund.erlaubt() {
        Zugangsbefund::Erlaubt
    } else {
        Zugangsbefund::Abgelehnt
    }
}

/// Was im Rumpf einer Anfrage steht, wenn ein Zugang verlangt wird.
///
/// # ⚑ Warum eine Hülle und keine Kopfzeilen
///
/// Zugangsdaten in HTTP-Kopfzeilen wären das Übliche. Hier wären sie
/// die falsche Wahl: Das Gateway zerlegt HTTP **von Hand**, und jede
/// weitere Kopfzeile, die etwas entscheidet, ist eine weitere Stelle,
/// an der eine handgeschriebene Zerlegung auf fremde Eingaben trifft.
///
/// **Eine Borsh-Hülle ist ein einziger Wert mit einer einzigen
/// Kodierung.** Sie geht durch dieselbe Kanonizitätsprüfung wie jeder
/// andere Protokolltyp: Liest sie sich vollständig, muss `to_vec` genau
/// diese Bytes wieder ergeben. Zwei Kopfzeilen mit demselben Namen
/// haben diese Eigenschaft nicht, und genau daran hängt die
/// Schmuggelklasse.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Anfragehuelle {
    /// Unter welchem Kontrakt, mit Schlüssel, Nummer und Unterschrift.
    pub zugang: ZugangsanfrageRoh,
    /// Die eigentliche Anfrage.
    pub rumpf: Vec<u8>,
}

/// [`Zugangsanfrage`] in ihrer Drahtform.
///
/// ⚑ **Getrennt vom Arbeitstyp, weil `BlsPublicKey` und `BlsSignature`
/// rohe Bytes sind.** Ein Punkt, der nicht auf der Kurve liegt, ist
/// über die Leitung darstellbar und im Arbeitstyp nicht erwünscht; die
/// Trennung sagt, wo die Prüfung sitzt.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct ZugangsanfrageRoh {
    pub sitzung: SitzungId,
    pub schluessel: BlsPublicKey,
    pub nummer: u64,
    pub unterschrift: BlsSignature,
}

impl From<ZugangsanfrageRoh> for Zugangsanfrage {
    fn from(r: ZugangsanfrageRoh) -> Self {
        Self {
            sitzung: r.sitzung,
            schluessel: r.schluessel,
            nummer: r.nummer,
            unterschrift: r.unterschrift,
        }
    }
}

impl From<&Zugangsanfrage> for ZugangsanfrageRoh {
    fn from(a: &Zugangsanfrage) -> Self {
        Self {
            sitzung: a.sitzung,
            schluessel: a.schluessel,
            nummer: a.nummer,
            unterschrift: a.unterschrift,
        }
    }
}

/// Woher das Gateway Kontrakte kennt.
///
/// ⚑ **Aus der Kette und nicht aus einer eigenen Tabelle.** Ein
/// Gateway, das seine Zugangsdaten selbst führt, ist wieder ein
/// Betreiber, dem man vertrauen muss; ein Kontrakt im Konsens gilt für
/// alle Gateways zugleich, und wer ihn widerruft, widerruft ihn
/// überall.
///
/// **Als Merkmal und nicht als Feld**, damit ein Test ohne Kette
/// auskommt und der Knoten später die echte Quelle einsetzt, ohne dass
/// diese Kiste ihn kennen muss.
pub trait Kontraktquelle {
    /// Kontrakt und Zustand zu einer Sitzung, falls es sie gibt.
    fn nachschlagen(&self, sitzung: SitzungId) -> Option<(Sitzungskontrakt, Sitzungszustand)>;
}

/// Die Zugangsstelle: Takt und Kontrakt in der richtigen Reihenfolge.
///
/// # ⚑ Die Reihenfolge **ist** die Aussage
///
/// 1. [`crate::takt::Takt::darf_pruefen`] **vor** der Unterschrift.
///    Eine Paarung kostet mehr als das Byte, das sie auslöst; ohne
///    Deckel davor ist die Prüfung selbst der Angriff.
/// 2. Nachschlagen und [`pruefe_zugang`].
/// 3. [`crate::takt::Takt::darf_anfragen`] **nach** der Unterschrift.
///    Davor wäre die Sitzungsnummer eine Behauptung, und jeder könnte
///    die Rate eines fremden Kontrakts aufbrauchen: eine Sperre, die
///    man gegen andere richten kann, ist eine Waffe und keine Grenze.
///
/// **Beide Grenzen einzeln umzudrehen ergibt eine Lücke**, und keine
/// von beiden fällt beim Lesen auf. Deshalb stehen sie hier zusammen
/// und nicht an zwei Stellen.
pub struct Zugangsstelle<Q: Kontraktquelle> {
    quelle: Q,
    takt: crate::takt::Takt,
}

impl<Q: Kontraktquelle> Zugangsstelle<Q> {
    /// Neu, über einer Quelle.
    pub fn neu(quelle: Q) -> Self {
        Self {
            quelle,
            takt: crate::takt::Takt::neu(),
        }
    }

    /// Darf diese Anfrage durch?
    ///
    /// `jetzt` ist die Konsensepoche, `jetzt_ms` die Uhr des Betreibers.
    /// **Zwei Zeiten, weil es zwei sind:** Die Gültigkeit eines
    /// Kontrakts hängt an der Epoche, die Rate an der Wanduhr.
    pub fn durchlassen(
        &mut self,
        anfrage: &Zugangsanfrage,
        rumpf: &[u8],
        jetzt: EpochId,
        jetzt_ms: u64,
    ) -> Zugangsbefund {
        // (1) Vor der teuren Arbeit.
        if !self.takt.darf_pruefen(jetzt_ms) {
            return Zugangsbefund::Abgelehnt;
        }
        // (2) Nachschlagen und prüfen.
        let gefunden = self.quelle.nachschlagen(anfrage.sitzung);
        let befund = match &gefunden {
            Some((k, z)) => pruefe_zugang(Some(k), Some(z), jetzt, anfrage, rumpf),
            None => pruefe_zugang(None, None, jetzt, anfrage, rumpf),
        };
        if !befund.erlaubt() {
            return Zugangsbefund::Abgelehnt;
        }
        // (3) Erst jetzt zählt der Kontrakt.
        if !self.takt.darf_anfragen(anfrage.sitzung, jetzt_ms) {
            return Zugangsbefund::Abgelehnt;
        }
        Zugangsbefund::Erlaubt
    }

    /// Wie viele Prüfungen im laufenden Fenster noch frei sind.
    ///
    /// Für die Betriebsbeobachtung: Ein Gateway, dessen Vorrat ständig
    /// bei null steht, wird abgetastet.
    pub fn freie_pruefungen(&self) -> u32 {
        self.takt.freie_pruefungen()
    }
}

/// Wie sich ein Anfragender ausgewiesen hat.
///
/// ⚑ **Der Unterschied wird vermerkt, nicht versteckt.** Er steht im
/// Beleg, damit ein Nutzer nachher sieht, welche Zusicherung er hatte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum Ausweisweg {
    /// Eine Vollmacht als Bearer-Token.
    ///
    /// Läuft in jedem Harness. **Der Prompt ist nicht gebunden**, das
    /// Gateway könnte im Rahmen der Vorbehalte Anfragen erfinden; was
    /// es dabei abrechnet, fällt in der Abrechnung auf.
    Vollmacht,
    /// Eine Unterschrift über die Anfrage selbst.
    ///
    /// Braucht einen Myelith-Klienten. **Der Prompt ist gebunden**, das
    /// Gateway kann keine Anfrage erfinden.
    Unterschrift,
}

impl<Q: Kontraktquelle> Zugangsstelle<Q> {
    /// Darf diese Anfrage mit einer **Vollmacht** durch?
    ///
    /// ⚑ **Dieselbe Reihenfolge wie beim anderen Weg:** Deckel auf
    /// Prüfungen vor der teuren Arbeit, Zähler je Kontrakt danach. Eine
    /// Vollmacht kostet bis zu [`crate::vollmacht::MAX_BLOECKE`]
    /// Paarungen und ist damit **teurer** als eine einzelne
    /// Unterschrift; der Deckel davor zählt sie deshalb nach Blöcken
    /// und nicht als eine.
    pub fn durchlassen_mit_vollmacht(
        &mut self,
        vollmacht: &crate::vollmacht::Vollmacht,
        rahmen: &crate::vollmacht::Anfragerahmen,
        jetzt_ms: u64,
    ) -> Zugangsbefund {
        // ⚑ **Je Block eine Prüfung.** Wer eine lange Kette schickt,
        // verbraucht mehr vom Vorrat, und genau das soll er.
        let bloecke = vollmacht.bloecke.len().max(1) as u32;
        for _ in 0..bloecke {
            if !self.takt.darf_pruefen(jetzt_ms) {
                return Zugangsbefund::Abgelehnt;
            }
        }

        let Some((kontrakt, zustand)) = self.quelle.nachschlagen(rahmen.sitzung) else {
            return Zugangsbefund::Abgelehnt;
        };
        if zustand.widerrufen {
            return Zugangsbefund::Abgelehnt;
        }
        if rahmen.jetzt.0 < kontrakt.gueltig_ab.0 || rahmen.jetzt.0 > kontrakt.gueltig_bis.0 {
            return Zugangsbefund::Abgelehnt;
        }
        if vollmacht.pruefen(&kontrakt.agent, rahmen).is_err() {
            return Zugangsbefund::Abgelehnt;
        }

        if !self.takt.darf_anfragen(rahmen.sitzung, jetzt_ms) {
            return Zugangsbefund::Abgelehnt;
        }
        Zugangsbefund::Erlaubt
    }
}

/// Das Vorhaben, gegen das der Kontrakt geprüft wird.
///
/// ⚑ **Ein Schritt, kein Betrag.** Stufe 2 fragt nur nach dem
/// **Zugang**; was die Anfrage kostet, steht erst fest, wenn sie
/// gerechnet ist, und wer bezahlt, ist offene Entscheidung G6.
/// Geprüft wird deshalb gegen den kleinsten möglichen Betrag: Er
/// unterschreitet kein Einzellimit und erschöpft kein Budget, prüft
/// aber Gültigkeit, Widerruf, Zeitfenster und Handelnden.
///
/// **Ein Betrag von null ginge nicht:** `pruefe` weist ihn mit
/// `NullBetrag` ab, und zwar zu Recht, denn ein Vorhaben über nichts
/// ist keines.
fn vorhaben_fuer(kontrakt: &Sitzungskontrakt, anfrage: &Zugangsanfrage) -> Vorhaben {
    Vorhaben {
        sitzung: kontrakt.adresse(),
        handelnder: Address::aus_schluessel(&anfrage.schluessel),
        waehrung: Waehrung::Credits,
        betrag: 1,
        // Der Empfänger ist das Gateway selbst; in Stufe 2 zahlt noch
        // niemand, aber die Positivliste des Kontrakts entscheidet
        // trotzdem mit, wem er überhaupt begegnen darf.
        empfaenger: kontrakt.empfaenger.first().copied().unwrap_or(Address::new([0u8; 32])),
        bestaetigt_ausgeliefert: false,
        // ⚑ **Die Nummer aus dem Zugangsausweis, und das ist kein
        // Zufall:** Sie ist derselbe Zähler, gegen den die Tür schon
        // die Wiedereinspielung sperrt. Die Kette führt seit dem
        // 2026-09-03 denselben Riegel gegen die zweite Abbuchung.
        //
        // **Dieses Vorhaben wird nie gebucht**, es dient nur der
        // Prüfung; die Nummer steht trotzdem hier, damit sie nicht
        // stillschweigend null ist und der Riegel gegen einen
        // Anfangswert liefe, den niemand gesetzt hat.
        nummer: anfrage.nummer.saturating_add(1),
    }
}

/// Ein Kontrakt, den es nicht gibt, für den Gleichlauf der Zweige.
///
/// ⚑ **Seine Agentenadresse ist null**, also fällt jede Anfrage an ihm
/// mit `FalscherHandelnder` durch. Das Ergebnis steht fest; gerechnet
/// wird es trotzdem, damit der Weg gleich lang ist.
fn leerer_kontrakt() -> Sitzungskontrakt {
    Sitzungskontrakt {
        inhaber: Address::new([0u8; 32]),
        agent: Address::new([0u8; 32]),
        credits: myl_types::sitzung::Grenzen::gesperrt(),
        myl: myl_types::sitzung::Grenzen::gesperrt(),
        empfaenger: Vec::new(),
        gueltig_ab: EpochId(0),
        gueltig_bis: EpochId(0),
        max_schritte: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::bls::BlsSecretKey;
    use myl_types::sitzung::Grenzen;

    fn geheim(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schluessel")
    }

    fn oeffentlich(b: u8) -> BlsPublicKey {
        geheim(b).public_key().expect("gueltiger Punkt")
    }

    fn kontrakt_fuer(agent: u8) -> Sitzungskontrakt {
        Sitzungskontrakt {
            inhaber: Address::aus_schluessel(&oeffentlich(200)),
            agent: Address::aus_schluessel(&oeffentlich(agent)),
            credits: Grenzen {
                budget: 10_000,
                einzellimit: 1_000,
                schwelle: u64::MAX,
                zeugenleiter: Vec::new(),
            },
            myl: Grenzen::gesperrt(),
            empfaenger: vec![Address::aus_schluessel(&oeffentlich(210))],
            gueltig_ab: EpochId(0),
            gueltig_bis: EpochId(100),
            max_schritte: 1_000,
        }
    }

    /// Eine gültig unterschriebene Anfrage des richtigen Agenten.
    fn anfrage(agent: u8, sitzung: SitzungId, nummer: u64, epoche: EpochId, rumpf: &[u8]) -> Zugangsanfrage {
        let msg = zugangsbotschaft(sitzung, nummer, epoche, rumpf);
        Zugangsanfrage {
            sitzung,
            schluessel: oeffentlich(agent),
            nummer,
            unterschrift: geheim(agent).sign(&msg).expect("unterschreiben"),
        }
    }

    /// Eine Quelle mit genau einem Kontrakt.
    struct EineQuelle(Sitzungskontrakt, Sitzungszustand);

    impl Kontraktquelle for EineQuelle {
        fn nachschlagen(&self, sitzung: SitzungId) -> Option<(Sitzungskontrakt, Sitzungszustand)> {
            if sitzung == self.0.adresse() {
                Some((self.0.clone(), self.1))
            } else {
                None
            }
        }
    }

    /// Eine Quelle, die nichts kennt.
    struct LeereQuelle;

    impl Kontraktquelle for LeereQuelle {
        fn nachschlagen(&self, _: SitzungId) -> Option<(Sitzungskontrakt, Sitzungszustand)> {
            None
        }
    }

    /// ⚑ **Die Ratengrenze steht vor der Unterschrift.**
    ///
    /// Ohne sie ist die Prüfung selbst der Angriff: ein paar hundert
    /// Bytes hinein, eine Paarung hinaus. Der Test schickt lauter
    /// unsinnige Anfragen an eine leere Quelle und zählt, wie viele
    /// Prüfungen das Gateway überhaupt anfasst.
    #[test]
    fn die_pruefgrenze_greift_vor_der_unterschrift() {
        let mut s = Zugangsstelle::neu(LeereQuelle);
        let rumpf = b"muell";
        let a = anfrage(7, SitzungId::new([9; 32]), 0, EpochId(1), rumpf);

        for _ in 0..crate::takt::PRUEFUNGEN_JE_FENSTER {
            assert_eq!(
                s.durchlassen(&a, rumpf, EpochId(1), 1_000),
                Zugangsbefund::Abgelehnt
            );
        }
        assert_eq!(s.freie_pruefungen(), 0, "die Grenze hat nicht gezaehlt");
        // Und ab hier wird gar nicht mehr geprueft.
        assert_eq!(
            s.durchlassen(&a, rumpf, EpochId(1), 1_000),
            Zugangsbefund::Abgelehnt
        );
    }

    /// ⚑ **Die Kontraktgrenze zählt erst nach der Unterschrift.**
    ///
    /// Sonst könnte jeder die Rate eines fremden Kontrakts aufbrauchen,
    /// indem er dessen Sitzungsnummer nennt. **Eine Sperre, die man
    /// gegen andere richten kann, ist eine Waffe und keine Grenze.**
    #[test]
    fn eine_fremde_sitzungsnummer_verbraucht_keine_fremde_rate() {
        let k = kontrakt_fuer(7);
        let s_id = k.adresse();
        let mut s = Zugangsstelle::neu(EineQuelle(k, Sitzungszustand::neu()));
        let rumpf = b"frage";

        // Ein Fremder nennt die Sitzungsnummer, kann aber nicht
        // unterschreiben: Agent 9 statt 7.
        for i in 0..(crate::takt::ANFRAGEN_JE_FENSTER * 3) {
            let fremd = anfrage(9, s_id, i as u64, EpochId(5), rumpf);
            assert_eq!(
                s.durchlassen(&fremd, rumpf, EpochId(5), 1_000),
                Zugangsbefund::Abgelehnt
            );
        }

        // Der echte Agent kommt trotzdem durch, volle Rate.
        for i in 0..crate::takt::ANFRAGEN_JE_FENSTER {
            let echt = anfrage(7, s_id, i as u64, EpochId(5), rumpf);
            assert_eq!(
                s.durchlassen(&echt, rumpf, EpochId(5), 1_000),
                Zugangsbefund::Erlaubt,
                "Anfrage {i} des echten Agenten wurde abgewiesen; \
                 der Fremde hat seine Rate verbraucht"
            );
        }
        // Und erst danach ist auch er am Ende.
        let noch_eine = anfrage(7, s_id, 99, EpochId(5), rumpf);
        assert_eq!(
            s.durchlassen(&noch_eine, rumpf, EpochId(5), 1_000),
            Zugangsbefund::Abgelehnt
        );
    }

    /// ⚑ **Der Kontrakt ist der Zugangsschlüssel** (G2, Stufe 2).
    #[test]
    fn ein_gueltiger_kontrakt_laesst_durch() {
        let k = kontrakt_fuer(7);
        let z = Sitzungszustand::neu();
        let rumpf = b"was ist die hauptstadt von frankreich";
        let a = anfrage(7, k.adresse(), 0, EpochId(5), rumpf);
        assert_eq!(
            pruefe_zugang(Some(&k), Some(&z), EpochId(5), &a, rumpf),
            Zugangsbefund::Erlaubt
        );
    }

    /// ⚑ **Der Schlüssel muss zur Agentenadresse passen.**
    ///
    /// Ohne diese Prüfung genügte irgendein Schlüssel: Der Anfragende
    /// unterschriebe mit seinem eigenen und käme unter fremdem Kontrakt
    /// durch. Dieselbe Stelle wie Fund 109.
    #[test]
    fn ein_fremder_schluessel_kommt_nicht_durch() {
        let k = kontrakt_fuer(7);
        let z = Sitzungszustand::neu();
        let rumpf = b"frage";
        // Agent 9 unterschreibt richtig, ist aber nicht der Agent des
        // Kontrakts.
        let a = anfrage(9, k.adresse(), 0, EpochId(5), rumpf);
        assert_eq!(
            pruefe_zugang(Some(&k), Some(&z), EpochId(5), &a, rumpf),
            Zugangsbefund::Abgelehnt
        );
    }

    /// ⚑ **Die Unterschrift bindet die Anfrage.**
    ///
    /// Ohne den Hash im Botschaftsaufbau könnte man eine abgefangene
    /// Unterschrift an einen anderen Prompt hängen und auf fremde
    /// Kosten rechnen lassen.
    #[test]
    fn eine_unterschrift_gilt_nur_fuer_ihre_anfrage() {
        let k = kontrakt_fuer(7);
        let z = Sitzungszustand::neu();
        let a = anfrage(7, k.adresse(), 0, EpochId(5), b"die eine frage");
        assert_eq!(
            pruefe_zugang(Some(&k), Some(&z), EpochId(5), &a, b"eine ganz andere frage"),
            Zugangsbefund::Abgelehnt,
            "die Unterschrift trug einen fremden Prompt"
        );
    }

    /// ⚑ **Und sie gilt nur für ihre Epoche und ihre Nummer.**
    #[test]
    fn eine_unterschrift_gilt_nur_fuer_ihre_epoche_und_nummer() {
        let k = kontrakt_fuer(7);
        let z = Sitzungszustand::neu();
        let rumpf = b"frage";
        let a = anfrage(7, k.adresse(), 3, EpochId(5), rumpf);
        assert_eq!(
            pruefe_zugang(Some(&k), Some(&z), EpochId(6), &a, rumpf),
            Zugangsbefund::Abgelehnt,
            "dieselbe Unterschrift galt in einer anderen Epoche"
        );
        let mut andere_nummer = a.clone();
        andere_nummer.nummer = 4;
        assert_eq!(
            pruefe_zugang(Some(&k), Some(&z), EpochId(5), &andere_nummer, rumpf),
            Zugangsbefund::Abgelehnt,
            "dieselbe Unterschrift galt unter einer anderen Nummer"
        );
    }

    /// Widerruf, Ablauf und Vorlauf schliessen die Tür.
    #[test]
    fn widerruf_ablauf_und_vorlauf_schliessen() {
        let k = kontrakt_fuer(7);
        let rumpf = b"frage";

        let mut widerrufen = Sitzungszustand::neu();
        widerrufen.widerrufen = true;
        let a = anfrage(7, k.adresse(), 0, EpochId(5), rumpf);
        assert_eq!(
            pruefe_zugang(Some(&k), Some(&widerrufen), EpochId(5), &a, rumpf),
            Zugangsbefund::Abgelehnt
        );

        let z = Sitzungszustand::neu();
        let spaet = anfrage(7, k.adresse(), 0, EpochId(101), rumpf);
        assert_eq!(
            pruefe_zugang(Some(&k), Some(&z), EpochId(101), &spaet, rumpf),
            Zugangsbefund::Abgelehnt,
            "ein abgelaufener Kontrakt liess durch"
        );
    }

    /// ⚑ **Der Kern der Stufe: Ablehnungen sind nicht unterscheidbar.**
    ///
    /// Antwortete die Tür verschieden auf „gibt es nicht", „widerrufen",
    /// „abgelaufen" und „falscher Schlüssel", wäre sie ein
    /// Auskunftsdienst über fremde Kontrakte. Wer Adressen durchprobiert,
    /// erführe, welche existieren.
    ///
    /// **Der Test hält den Wert fest, nicht die Absicht.**
    #[test]
    fn alle_ablehnungen_sehen_gleich_aus() {
        let k = kontrakt_fuer(7);
        let z = Sitzungszustand::neu();
        let rumpf = b"frage";
        let gut = anfrage(7, k.adresse(), 0, EpochId(5), rumpf);

        let mut widerrufen = Sitzungszustand::neu();
        widerrufen.widerrufen = true;

        let faelle: Vec<(&str, Zugangsbefund)> = vec![
            (
                "Kontrakt gibt es nicht",
                pruefe_zugang(None, None, EpochId(5), &gut, rumpf),
            ),
            (
                "widerrufen",
                pruefe_zugang(Some(&k), Some(&widerrufen), EpochId(5), &gut, rumpf),
            ),
            (
                "abgelaufen",
                pruefe_zugang(Some(&k), Some(&z), EpochId(101), &gut, rumpf),
            ),
            (
                "falscher Schluessel",
                pruefe_zugang(
                    Some(&k),
                    Some(&z),
                    EpochId(5),
                    &anfrage(9, k.adresse(), 0, EpochId(5), rumpf),
                    rumpf,
                ),
            ),
            (
                "kaputte Unterschrift",
                pruefe_zugang(
                    Some(&k),
                    Some(&z),
                    EpochId(5),
                    &Zugangsanfrage {
                        unterschrift: BlsSignature([0; 96]),
                        ..gut.clone()
                    },
                    rumpf,
                ),
            ),
        ];
        for (was, befund) in &faelle {
            assert_eq!(
                *befund,
                Zugangsbefund::Abgelehnt,
                "der Fall `{was}` wurde nicht abgelehnt"
            );
        }
        // ⚑ **Und die Gegenprobe:** Der gute Fall muss durchgehen,
        // sonst sagt die Gleichheit oben nur, dass alles abgelehnt wird.
        assert_eq!(
            pruefe_zugang(Some(&k), Some(&z), EpochId(5), &gut, rumpf),
            Zugangsbefund::Erlaubt,
            "auch der gueltige Fall wurde abgelehnt, der Test prueft dann nichts"
        );
    }

    /// ⚑ **Auch ohne Kontrakt wird geprüft, nicht gesprungen.**
    ///
    /// Eine gleiche Antwort genügt nicht, wenn der Weg dorthin
    /// verschieden lang ist. **Der Test misst keine Zeit, und das
    /// gehört gesagt:** Die Eigenschaft steht in der Struktur, nicht in
    /// dieser Zusicherung. `pruefe_zugang` rechnet beide Teile
    /// unbedingt und verbindet sie erst danach; es gibt keinen Zweig,
    /// den man entfernen könnte, um Arbeit zu sparen.
    ///
    /// Was der Test leistet: Er hält fest, dass der Fall **ohne**
    /// Kontrakt dieselbe Antwort gibt wie jeder andere Fehlschlag.
    #[test]
    fn der_zweig_ohne_kontrakt_rechnet_dieselbe_paarung() {
        let rumpf = b"frage";
        let sitzung = SitzungId::new([3; 32]);
        let gut = anfrage(7, sitzung, 0, EpochId(5), rumpf);
        // Eine gueltige Unterschrift ohne Kontrakt: abgelehnt, aber
        // geprueft.
        assert_eq!(
            pruefe_zugang(None, None, EpochId(5), &gut, rumpf),
            Zugangsbefund::Abgelehnt
        );
        // Und eine ungueltige ebenso: derselbe Weg, dasselbe Ergebnis.
        assert_eq!(
            pruefe_zugang(
                None,
                None,
                EpochId(5),
                &Zugangsanfrage {
                    unterschrift: BlsSignature([0; 96]),
                    ..gut
                },
                rumpf
            ),
            Zugangsbefund::Abgelehnt
        );
    }
}
