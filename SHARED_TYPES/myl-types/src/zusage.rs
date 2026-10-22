//! Die Kapazitätszusage: was ein Knoten zu halten anbietet.
//!
//! # Wozu, und warum sie mehr ist als Bequemlichkeit
//!
//! Der Wunsch war ein Schalter, mit dem ein Miner Hardware zu- und
//! abschaltet (Festlegung des Projektinhabers, 2026-08-30). **Der
//! Schalter ist die Eingabe, aus der sich ergibt, wie groß die
//! Wissensdatenbank überhaupt werden darf.**
//!
//! Die Last je speicherndem Knoten ist `W · f / N`, und nichts bindet
//! den Umfang der Daten an die Zahl der Knoten: W wächst mit der
//! Nutzung, N mit dem Ertrag. Ohne Schranke wächst W, bis Knoten gehen,
//! und dann fällt N. **Die Summe der Zusagen ist die Schranke**, gegen
//! die eine Aufnahme geprüft wird.
//!
//! Zugleich ist sie die Voraussetzung für die Zuteilung: Wem das
//! Protokoll ein Teil zuweist, muss vorher gesagt haben, dass er Platz
//! hat.
//!
//! # ⚑ Sie gilt ab der nächsten Epoche, nie ab sofort
//!
//! [`Kapazitaetszusage::ab_epoche`] muss **echt größer** als die
//! laufende Epoche sein. Damit ist „mitten im Auftrag abschalten" durch
//! Konstruktion ausgeschlossen: Wer abschalten will, sagt für die
//! nächste Epoche weniger zu, und bis dahin gilt, was er zugesagt hat.
//! Ein Rückzug innerhalb der laufenden Epoche bleibt ein Ausfall und
//! wird als solcher behandelt.
//!
//! Nebenbei erledigt diese Regel den Wiedereinspielungsangriff: Eine
//! alte, höhere Zusage trägt ihre eigene Epoche im signierten Teil und
//! ist damit für jede spätere Epoche wertlos.
//!
//! # ⚑ Null ist erlaubt, und das ist keine Nachlässigkeit
//!
//! Eine Zusage über null Bytes ist die **Abmeldung**. Wer sie verböte,
//! machte den Beitritt zu einer Falle: Ein Knoten käme aus der
//! Speicherpflicht nur noch durch Verschwinden, und Verschwinden ist im
//! Protokoll ein Ausfall mit Folgen. Der geordnete Ausstieg muss
//! ausdrücklich möglich sein.
//!
//! # ⚑ Warum nur Speicher, obwohl der Schalter vier Größen nennt
//!
//! CPU, GPU und Arbeitsspeicher stehen nicht darin, und das ist eine
//! Entscheidung, keine Auslassung.
//!
//! - **CPU und GPU** sind über die verifizierte Arbeit bereits bezahlt,
//!   und zwar nachgewiesen. Ein zweites Feld dafür bekäme im Protokoll
//!   heute keinen Leser.
//! - **Arbeitsspeicher** ist Voraussetzung, keine eigene Größe: Wer zu
//!   wenig hat, hält seine Zuteilung nicht und verliert sie. Das ist
//!   bereits eingepreist.
//!
//! **Ein Feld, das niemand liest, ist in diesem Repositorium ein
//! benannter Fehler** (Fund 98: `--upstream` wurde gesetzt,
//! durchgereicht und beworben, und gelesen hat es nie jemand). Sobald
//! die Zuteilung Rechenkapazität wirklich auswertet, gehören die Felder
//! dazu; vorher nicht.
//!
//! # ⚑ Seit dem 2026-10-21: je Netzmodell Ebenen und Ebenenzeit
//!
//! Die Zuteilung wertet sie jetzt aus (Stufe 4 des Dispatchers,
//! `myl_scheduler::ebenenverteilung`): Ein Pod verteilt seine Ebenen nach
//! Kapazität und gemessener Zeit seiner Mitglieder. Je Modell der Netzliste
//! ein [`Modellzusage`], das Modell genannt bei seinem **Abdruck** und nicht
//! beim Namen (Festlegung E3: Namen ändern sich, der Abdruck nicht).
//!
//! ⚑ **Die Zeit ist eine Frist.** Wer schneller ansagt, als er rechnet,
//! bekommt mehr Ebenen und damit mehr Gewichtsarbeit; das wäre ein Anreiz
//! zu lügen. Deshalb gilt die angesagte Zeit als Zusage: Rechnet ein Shard
//! seinen Bereich deutlich langsamer ([`FRIST_FAKTOR`]), ist das ein
//! Verstoß wie ein Ausfall.
//!
//! # Was sie nicht ist
//!
//! **Keine Zusicherung, sondern eine Obergrenze.** Zugesagt heißt: bis
//! hierhin darf zugeteilt werden. Bezahlt wird davon nichts; bezahlt
//! wird, was nachgewiesen ist. Wer viel zusagt und nichts hält, verdient
//! nichts, verzerrt aber das Budget des Netzes. **Deshalb gehört das
//! Budget aus der nachgewiesenen Kapazität berechnet und nicht aus der
//! zugesagten**, und deshalb steht unten eine Obergrenze, die grobe
//! Vertipper abfängt.

use crate::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use crate::ids::{EpochId, MinerId};
use crate::uebergang::Rolle;
use borsh::{BorshDeserialize, BorshSerialize};

/// Trennstring der Kapazitätszusage.
///
/// ⚑ **`v2` seit dem 2026-10-21**: Der signierte Kern trägt jetzt die
/// Modelle. Eine Unterschrift über den alten Kern gilt nie für den neuen.
pub const DST_KAPAZITAETSZUSAGE: &[u8] = b"MYELITH_KAPAZITAETSZUSAGE_v2";

/// Höchstens so viele Modelle in einer Zusage (gegen aufgeblähte Blöcke).
pub const MODELLE_HOECHSTENS: usize = 8;

/// Um so viel darf ein Shard seine zugesagte Zeit überschreiten, bevor es
/// ein Verstoß ist: das Doppelte. Messrauschen und ein längerer Kontext
/// als bei der Messung liegen darunter.
pub const FRIST_FAKTOR: u64 = 2;

/// Was ein Knoten für ein Modell der Netzliste anbietet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Modellzusage {
    /// Der Abdruck des Artefakts auf der Netzliste.
    pub abdruck: [u8; 32],
    /// Wie viele Ebenen er höchstens hält, den Kopf in Ebenen nach Bytes
    /// mitgezählt.
    pub ebenen_hoechstens: u32,
    /// Mikrosekunden je Ebene und Token im Decode (Median), ≥ 1.
    pub ebenenzeit_us: u64,
    /// ⚑ **Teilablage** (Festlegung E6, Weg a): Hält er nur einen Bereich
    /// `[von, bis)` auf der Platte, steht er hier, und die Verteilung teilt
    /// ihm nur Ebenen darin zu. `None`: das ganze Artefakt.
    pub gehalten: Option<(u32, u32)>,
}

impl Modellzusage {
    /// Die Frist für `ebenen` Ebenen dieses Knotens, in Mikrosekunden.
    pub fn frist_us(&self, ebenen: u32) -> u64 {
        self.ebenenzeit_us.saturating_mul(ebenen as u64).saturating_mul(FRIST_FAKTOR)
    }
}

/// Obergrenze einer einzelnen Zusage: ein Pebibyte.
///
/// **Gegen Vertipper und Überlauf, nicht gegen Angreifer.** Wer
/// absichtlich zu viel zusagt, verdient daran nichts; die Schranke soll
/// verhindern, dass eine verrutschte Einheit die Budgetrechnung des
/// ganzen Netzes über den Haufen wirft. Ein Pebibyte je Knoten ist weit
/// jenseits dessen, was ein einzelner Halter je beiträgt, und weit
/// diesseits eines Überlaufs.
pub const HOECHSTZUSAGE_BYTES: u64 = 1 << 50;

/// Was ein Knoten für eine Epoche und die folgenden zu halten anbietet.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kapazitaetszusage {
    /// Wer zusagt.
    pub halter: MinerId,
    /// Ab welcher Epoche sie gilt. Echt größer als die laufende.
    pub ab_epoche: EpochId,
    /// Angebotener Speicher in Bytes. Null ist die Abmeldung.
    pub speicher_bytes: u64,
    /// Je Modell der Netzliste, was er rechnen kann (seit dem 2026-10-21).
    pub modelle: Vec<Modellzusage>,
    /// Unterschrift des Halters in der Rolle [`Rolle::Store`].
    pub signature: BlsSignature,
}

/// Warum eine Zusage nicht angenommen wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zusagefehler {
    /// Sie gilt ab der laufenden Epoche oder früher.
    ///
    /// Eine Zusage wirkt erst zur nächsten Epochengrenze; alles andere
    /// wäre ein Rückzug mitten im Auftrag.
    NichtInDerZukunft {
        /// Die genannte Epoche.
        ab_epoche: EpochId,
        /// Die laufende.
        jetzt: EpochId,
    },
    /// Mehr als [`HOECHSTZUSAGE_BYTES`].
    ZuGross {
        /// Der genannte Wert.
        bytes: u64,
    },
    /// Die Unterschrift stimmt nicht, oder sie stammt von einem anderen.
    Unterschrift,
    /// Mehr als [`MODELLE_HOECHSTENS`] Modelle.
    ZuVieleModelle,
    /// Ein Modell steht zweimal darin.
    ModellDoppelt,
    /// Null Ebenen, null Zeit oder ein leerer gehaltener Bereich.
    ModellUngueltig,
    /// Ein Modell, das dieses Netz nicht rechnet (seit dem 2026-10-22).
    ModellNichtImNetz,
    /// Mehr Ebenen, als das Modell samt Kopf hat.
    ZuVieleEbenen,
    /// Ein gehaltener Bereich, der über das Modell hinausgeht.
    GehaltenAusserhalb,
}

impl Kapazitaetszusage {
    /// Eine unsignierte Zusage. [`Self::signiere`] gehört dazu.
    pub fn neu(halter: MinerId, ab_epoche: EpochId, speicher_bytes: u64) -> Self {
        Self {
            halter,
            ab_epoche,
            speicher_bytes,
            modelle: Vec::new(),
            signature: BlsSignature([0u8; crate::bls::BLS_SIG_LEN]),
        }
    }

    /// Mit einem Modell mehr; vor dem Unterschreiben.
    pub fn mit_modell(mut self, m: Modellzusage) -> Self {
        self.modelle.push(m);
        self
    }

    /// Was er für das Modell mit diesem Abdruck zusagt.
    pub fn modell(&self, abdruck: &[u8; 32]) -> Option<&Modellzusage> {
        self.modelle.iter().find(|m| &m.abdruck == abdruck)
    }

    /// Die Bytes, über die unterschrieben wird.
    ///
    /// Trennstring, Rollenbyte, dann der Kern. **Die Rolle wird
    /// mitsigniert**, damit eine Zusage in keiner anderen Rolle gilt.
    pub fn signierbotschaft(&self) -> Vec<u8> {
        let kern = (self.halter, self.ab_epoche, self.speicher_bytes, &self.modelle);
        let rumpf = borsh::to_vec(&kern).expect("feste Feldbreiten sind stets serialisierbar");
        let mut msg =
            Vec::with_capacity(DST_KAPAZITAETSZUSAGE.len() + 1 + rumpf.len());
        msg.extend_from_slice(DST_KAPAZITAETSZUSAGE);
        msg.push(Rolle::Store.byte());
        msg.extend_from_slice(&rumpf);
        msg
    }

    /// Unterschreibt die Zusage.
    pub fn signiere(&mut self, sk: &BlsSecretKey) -> Result<(), crate::bls::BlsError> {
        self.signature = sk.sign(&self.signierbotschaft())?;
        Ok(())
    }

    /// Stammt sie von dem, der darin steht?
    ///
    /// ⚑ **Zwei Fragen in einem Schritt**, nach dem Muster von Fund 96:
    /// Die Unterschrift muss stimmen **und** der Schlüssel muss zu
    /// [`Self::halter`] gehören. Nur das erste zu prüfen hieße, jeden
    /// Beliebigen im Namen eines anderen zusagen zu lassen.
    pub fn ist_vom_halter(&self, pk: &BlsPublicKey) -> bool {
        MinerId::aus_schluessel(pk) == self.halter
            && pk.verify(&self.signierbotschaft(), &self.signature)
    }

    /// Prüft Form und Unterschrift gegen die laufende Epoche.
    pub fn pruefen(
        &self,
        pk: &BlsPublicKey,
        jetzt: EpochId,
    ) -> Result<(), Zusagefehler> {
        if self.ab_epoche <= jetzt {
            return Err(Zusagefehler::NichtInDerZukunft {
                ab_epoche: self.ab_epoche,
                jetzt,
            });
        }
        if self.speicher_bytes > HOECHSTZUSAGE_BYTES {
            return Err(Zusagefehler::ZuGross {
                bytes: self.speicher_bytes,
            });
        }
        if self.modelle.len() > MODELLE_HOECHSTENS {
            return Err(Zusagefehler::ZuVieleModelle);
        }
        for (i, m) in self.modelle.iter().enumerate() {
            if self.modelle[..i].iter().any(|v| v.abdruck == m.abdruck) {
                return Err(Zusagefehler::ModellDoppelt);
            }
            if m.ebenen_hoechstens == 0 || m.ebenenzeit_us == 0 || m.gehalten.is_some_and(|(v, b)| v >= b) {
                return Err(Zusagefehler::ModellUngueltig);
            }
        }
        if !self.ist_vom_halter(pk) {
            return Err(Zusagefehler::Unterschrift);
        }
        Ok(())
    }

    /// **Prüft dazu gegen das Podmodell des Netzes** (6.6, 2026-10-22): nur
    /// dieses Modell, höchstens alle Ebenen samt Kopf, ein gehaltener
    /// Bereich innerhalb der Ebenen.
    ///
    /// ⚑ **Vor der Unterschrift**, wie die Form: Eine falsch geformte
    /// Zusage soll ihren Grund nennen und nicht als „Unterschrift“ scheitern.
    pub fn pruefen_im_netz(
        &self,
        pk: &BlsPublicKey,
        jetzt: EpochId,
        podmodell: &crate::podmodell::Podmodell,
    ) -> Result<(), Zusagefehler> {
        for m in &self.modelle {
            if m.abdruck != podmodell.abdruck {
                return Err(Zusagefehler::ModellNichtImNetz);
            }
            if m.ebenen_hoechstens > podmodell.ebenen_samt_kopf() {
                return Err(Zusagefehler::ZuVieleEbenen);
            }
            if m.gehalten.is_some_and(|(_, bis)| bis > podmodell.ebenen) {
                return Err(Zusagefehler::GehaltenAusserhalb);
            }
        }
        self.pruefen(pk, jetzt)
    }

    /// Ist das die Abmeldung?
    pub fn ist_abmeldung(&self) -> bool {
        self.speicher_bytes == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schluessel(b: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schlüssel")
    }

    /// Zusage samt passendem Schlüsselpaar.
    fn zusage(b: u8, ab: u64, bytes: u64) -> (Kapazitaetszusage, BlsPublicKey) {
        let sk = schluessel(b);
        let pk = sk.public_key().expect("Öffentlicher Schlüssel");
        let mut z = Kapazitaetszusage::neu(MinerId::aus_schluessel(&pk), EpochId(ab), bytes);
        z.signiere(&sk).expect("signieren");
        (z, pk)
    }

    fn modell(n: u8) -> Modellzusage {
        Modellzusage { abdruck: [n; 32], ebenen_hoechstens: 20, ebenenzeit_us: 900, gehalten: None }
    }

    /// ⚑ **Die Modelle sind unterschrieben** (2026-10-21): Wer nachträglich
    /// Ebenen oder Zeit ändert, bricht die Unterschrift; doppelte, leere oder
    /// zu viele Modelle werden abgewiesen, vor der Unterschrift.
    #[test]
    fn die_modelle_sind_unterschrieben_und_geprueft() {
        let sk = schluessel(9);
        let pk = sk.public_key().expect("pk");
        let neu = |m: Vec<Modellzusage>| {
            let mut z = Kapazitaetszusage::neu(MinerId::aus_schluessel(&pk), EpochId(11), 1024);
            z.modelle = m;
            z.signiere(&sk).expect("signieren");
            z
        };
        let z = neu(vec![modell(1), modell(2)]);
        assert_eq!(z.pruefen(&pk, EpochId(10)), Ok(()));
        assert_eq!(z.modell(&[2; 32]).map(|m| m.ebenen_hoechstens), Some(20));
        let mut gefaelscht = z.clone();
        gefaelscht.modelle[0].ebenenzeit_us = 1;
        assert_eq!(gefaelscht.pruefen(&pk, EpochId(10)), Err(Zusagefehler::Unterschrift));
        assert_eq!(neu(vec![modell(1), modell(1)]).pruefen(&pk, EpochId(10)), Err(Zusagefehler::ModellDoppelt));
        let leer = Modellzusage { ebenen_hoechstens: 0, ..modell(3) };
        assert_eq!(neu(vec![leer]).pruefen(&pk, EpochId(10)), Err(Zusagefehler::ModellUngueltig));
        let falsch = Modellzusage { gehalten: Some((5, 5)), ..modell(3) };
        assert_eq!(neu(vec![falsch]).pruefen(&pk, EpochId(10)), Err(Zusagefehler::ModellUngueltig));
        let viele: Vec<Modellzusage> = (0..=MODELLE_HOECHSTENS as u8).map(modell).collect();
        assert_eq!(neu(viele).pruefen(&pk, EpochId(10)), Err(Zusagefehler::ZuVieleModelle));
        assert_eq!(modell(1).frist_us(10), 18_000, "zehn Ebenen zu 900 µs, doppelt");
    }

    /// ⚑ **Gegen das Podmodell** (6.6): ein fremdes Modell, mehr Ebenen als
    /// das Modell samt Kopf, ein gehaltener Bereich hinter der letzten
    /// Ebene; genau die Grenze geht noch.
    #[test]
    fn die_zusage_wird_gegen_das_podmodell_geprueft() {
        let sk = schluessel(8);
        let pk = sk.public_key().expect("pk");
        let podmodell = crate::podmodell::Podmodell { abdruck: [1; 32], ebenen: 28, kopf_platz: 20, kopf_zeit: 5 };
        let neu = |m: Modellzusage| {
            let mut z = Kapazitaetszusage::neu(MinerId::aus_schluessel(&pk), EpochId(11), 0).mit_modell(m);
            z.signiere(&sk).expect("signieren");
            z.pruefen_im_netz(&pk, EpochId(10), &podmodell)
        };
        assert_eq!(neu(Modellzusage { ebenen_hoechstens: 48, ..modell(1) }), Ok(()), "alle samt Kopf");
        assert_eq!(neu(modell(2)), Err(Zusagefehler::ModellNichtImNetz));
        assert_eq!(neu(Modellzusage { ebenen_hoechstens: 49, ..modell(1) }), Err(Zusagefehler::ZuVieleEbenen));
        assert_eq!(neu(Modellzusage { gehalten: Some((20, 28)), ..modell(1) }), Ok(()));
        assert_eq!(neu(Modellzusage { gehalten: Some((20, 29)), ..modell(1) }), Err(Zusagefehler::GehaltenAusserhalb));
    }

    #[test]
    fn signieren_und_pruefen() {
        let (z, pk) = zusage(1, 11, 2 * 1024 * 1024 * 1024 * 1024);
        assert_eq!(z.pruefen(&pk, EpochId(10)), Ok(()));
        assert!(z.ist_vom_halter(&pk));
    }

    /// ⚑ **Sie gilt ab der nächsten Epoche, nie ab sofort.**
    ///
    /// Damit ist „mitten im Auftrag abschalten" durch Konstruktion
    /// ausgeschlossen. Der Test fährt beide Seiten der Grenze ab, sonst
    /// bliebe offen, ob die Prüfung die Epoche beurteilt oder alles
    /// ablehnt.
    #[test]
    fn eine_zusage_gilt_erst_ab_der_naechsten_epoche() {
        for (ab, erlaubt) in [(9u64, false), (10, false), (11, true), (99, true)] {
            let (z, pk) = zusage(2, ab, 1024);
            let r = z.pruefen(&pk, EpochId(10));
            assert_eq!(
                r.is_ok(),
                erlaubt,
                "ab_epoche {ab} bei laufender 10: {r:?}"
            );
        }
    }

    /// ⚑ **Null ist die Abmeldung, kein Fehler.**
    ///
    /// Wer sie verböte, machte den Beitritt zu einer Falle: Ein Knoten
    /// käme aus der Speicherpflicht nur noch durch Verschwinden, und
    /// Verschwinden ist im Protokoll ein Ausfall mit Folgen.
    #[test]
    fn null_ist_die_abmeldung_und_kein_fehler() {
        let (z, pk) = zusage(3, 11, 0);
        assert_eq!(z.pruefen(&pk, EpochId(10)), Ok(()));
        assert!(z.ist_abmeldung());
    }

    /// ⚑ **Fund-96-Muster: Unterschrift **und** Identität.**
    ///
    /// Hier unterschreibt Schlüssel A eine Zusage, die B als Halter
    /// nennt. Die Unterschrift ist echt; sie gehört nur nicht zu dem,
    /// der darin steht. Wer nur die Unterschrift prüft, lässt jeden
    /// Beliebigen im Namen eines anderen zusagen.
    #[test]
    fn eine_zusage_im_namen_eines_anderen_gilt_nicht() {
        let sk_a = schluessel(4);
        let pk_a = sk_a.public_key().expect("pk");
        let pk_b = schluessel(5).public_key().expect("pk");

        let mut z = Kapazitaetszusage::neu(MinerId::aus_schluessel(&pk_b), EpochId(11), 4096);
        z.signiere(&sk_a).expect("signieren");

        // Die Unterschrift selbst ist gültig, gegen A geprüft.
        assert!(pk_a.verify(&z.signierbotschaft(), &z.signature));
        // Sie trägt die Zusage trotzdem nicht.
        assert!(!z.ist_vom_halter(&pk_a), "fremder Halter kam durch");
        assert!(!z.ist_vom_halter(&pk_b), "fremde Unterschrift kam durch");
        assert_eq!(z.pruefen(&pk_b, EpochId(10)), Err(Zusagefehler::Unterschrift));
    }

    /// ⚑ **Eine alte, höhere Zusage lässt sich nicht wiedereinspielen.**
    ///
    /// Sie trägt ihre Epoche im signierten Teil. Wer sie später erneut
    /// einreicht, reicht eine Zusage für eine vergangene Epoche ein, und
    /// die gilt nicht.
    #[test]
    fn eine_alte_zusage_laesst_sich_nicht_wiedereinspielen() {
        let (alt, pk) = zusage(6, 5, 999_999_999);
        assert_eq!(alt.pruefen(&pk, EpochId(4)), Ok(()));
        assert_eq!(
            alt.pruefen(&pk, EpochId(20)),
            Err(Zusagefehler::NichtInDerZukunft {
                ab_epoche: EpochId(5),
                jetzt: EpochId(20),
            })
        );
    }

    /// ⚑ **Die Rolle wird mitsigniert.**
    ///
    /// Dieselben Kernbytes mit einem anderen Rollenbyte ergeben eine
    /// andere Botschaft. Eine in einer anderen Rolle abgegebene
    /// Unterschrift gilt hier nicht, und zwar durch Konstruktion.
    #[test]
    fn eine_unterschrift_aus_einer_anderen_rolle_gilt_nicht() {
        let sk = schluessel(7);
        let pk = sk.public_key().expect("pk");
        let z = Kapazitaetszusage::neu(MinerId::aus_schluessel(&pk), EpochId(11), 4096);

        // Dieselben Kernbytes, aber als Checker unterschrieben.
        let mut fremd = z.signierbotschaft();
        let stelle = DST_KAPAZITAETSZUSAGE.len();
        assert_eq!(fremd[stelle], Rolle::Store.byte());
        fremd[stelle] = Rolle::Checker.byte();

        let mut z2 = z.clone();
        z2.signature = sk.sign(&fremd).expect("signieren");
        assert!(
            !z2.ist_vom_halter(&pk),
            "eine Checker-Unterschrift trug eine Speicherzusage"
        );
    }

    #[test]
    fn eine_masslose_zusage_wird_abgelehnt() {
        let (z, pk) = zusage(8, 11, HOECHSTZUSAGE_BYTES + 1);
        assert_eq!(
            z.pruefen(&pk, EpochId(10)),
            Err(Zusagefehler::ZuGross {
                bytes: HOECHSTZUSAGE_BYTES + 1
            })
        );
        let (gerade_noch, pk2) = zusage(9, 11, HOECHSTZUSAGE_BYTES);
        assert_eq!(gerade_noch.pruefen(&pk2, EpochId(10)), Ok(()));
    }

    /// Über die Leitung und zurück, unverändert.
    #[test]
    fn borsh_haelt_die_zusage() {
        let (z, _) = zusage(10, 11, 123_456);
        let bytes = borsh::to_vec(&z).expect("serialisieren");
        let zurueck: Kapazitaetszusage = borsh::from_slice(&bytes).expect("lesen");
        assert_eq!(z, zurueck);
    }

    /// Das Rollenbyte der neuen Rolle darf die alten nicht verschieben.
    #[test]
    fn die_neue_rolle_verschiebt_keine_alte() {
        assert_eq!(Rolle::Shard.byte(), 1);
        assert_eq!(Rolle::PodMitglied.byte(), 2);
        assert_eq!(Rolle::Validator.byte(), 3);
        assert_eq!(Rolle::Checker.byte(), 4);
        assert_eq!(Rolle::Store.byte(), 5);
    }
}
