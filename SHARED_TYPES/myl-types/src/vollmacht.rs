//! Die Vollmacht: ein Bearer-Token mit Abschwächung (Stufe 2).
//!
//! # ⚑ Warum es das gibt, obwohl G2 „kein Datenbankmerkmal" sagt
//!
//! G2 lautet: **der Zugangsschlüssel ist ein Sitzungskontrakt und kein
//! Datenbankmerkmal.** Das ist eine Aussage darüber, **wo die Befugnis
//! lebt**, nicht darüber, **was ein Nutzer einklebt**.
//!
//! ⚑ **Der erste Entwurf der Stufe 2 hat die zweite Frage übergangen.**
//! Er verlangte je Anfrage eine BLS-Unterschrift über Sitzung, Nummer,
//! Epoche und Prompt. Das ist die schärfere Zusicherung und **in kein
//! bestehendes Harness einzukleben**: Jeder Inferenzanbieter
//! authentifiziert per `Authorization: Bearer`, und ein Wechsel heisst
//! Basis-URL und Schlüssel tauschen. Ein Klient, der BLS signiert und
//! die Konsensepoche kennt, ist kein Schlüssel mehr, sondern ein
//! Programm.
//!
//! **Beides steht deshalb nebeneinander**, und der Beleg sagt, welcher
//! Weg benutzt wurde:
//!
//! | Weg | Läuft im Harness | Gateway kann Anfragen erfinden |
//! |---|---|---|
//! | Vollmacht (Bearer) | ja | ja, im Rahmen der Vorbehalte |
//! | Unterschrift je Anfrage | nein | nein, der Prompt ist gebunden |
//!
//! ⚑ **Der Unterschied wird nicht versteckt, sondern vermerkt.** Wer die
//! schärfere Zusicherung braucht, nimmt den anderen Weg; wer bequem
//! anfangen will, nimmt diesen und sieht am Beleg, was er hat.
//!
//! # Die Bauart: Biscuits Signaturkette, nicht sein Datalog
//!
//! **Macaroons prüft ihr Aussteller** mit einem Wurzelgeheimnis, das er
//! selbst hält. Hier ist der Aussteller der **Nutzer** und der Prüfer
//! das **Gateway**, also zwei verschiedene Parteien: Ein HMAC über ein
//! gemeinsames Geheimnis scheidet aus, denn das Gateway darf den
//! Schlüssel des Nutzers nicht haben.
//!
//! **Biscuit löst genau das** mit einer Kette signierter Blöcke:
//!
//! 1. Der **Vollmachtsblock** ist mit dem Agentenschlüssel
//!    unterschrieben und nennt den **nächsten** öffentlichen Schlüssel.
//! 2. Jeder weitere Block ist mit dem Schlüssel unterschrieben, den
//!    sein Vorgänger genannt hat, und nennt wieder einen nächsten.
//! 3. Der **Nachweis** am Ende ist die Saat des zuletzt genannten
//!    Schlüssels.
//!
//! ⚑ **Abschwächen kann jeder Halter, ohne jemanden zu fragen:** Er
//! würfelt ein Schlüsselpaar, hängt einen Block an, unterschreibt ihn
//! mit dem Nachweis, den er hat, und gibt die neue Saat weiter.
//!
//! ⚑ **Wegnehmen kann niemand.** Wer den letzten Block streicht, hat
//! einen Nachweis, der nicht mehr zum zuletzt genannten Schlüssel passt;
//! um einen passenden zu bauen, bräuchte er die Saat des Vorgängers, und
//! die hat der Abschwächende weggeworfen.
//!
//! # ⚑ Was ausdrücklich nicht übernommen wird
//!
//! **Biscuits Datalog.** Ein Logikinterpreter, der vom Anfragenden
//! gelieferte Programme an der Tür auswertet, ist eine Angriffsfläche,
//! die zu dieser Stufe nicht passt: unbegrenzte Laufzeit, unbegrenzter
//! Speicher, eine eigene Zerlegung. **Die Vorbehalte hier sind ein
//! Aufzählungstyp mit vier Fällen**, jeder in konstanter Zeit prüfbar.
//!
//! Wer später mehr braucht, hängt Fälle an; wer einen Interpreter
//! braucht, hat eine andere Frage.
//!
//! # ⚑ Warum sie seit dem 2026-09-03 hier wohnt und nicht im Gateway
//!
//! **Weil die Kette sie prüfen muss.** Ein Harness hält einen
//! Bearer-Token und keinen Schlüssel; es kann also keine
//! Kettentransaktion signieren. Damit eine gerechnete Anfrage überhaupt
//! **abgebucht** werden kann, muss `sitzung_ausgeben` die Vollmacht als
//! Autorisierung des Agenten anerkennen, und `myl-ledger` darf
//! `myl-gateway` nicht kennen.
//!
//! Dieselbe Begründung wie bei [`crate::poi_botschaft`] (Fund 144) und
//! [`crate::ortsleitung`] (Fund 155): Ein Typ, den beide Seiten
//! brauchen, gehört dorthin, wo beide hinsehen. **Das Gateway reicht
//! ihn weiter**, damit kein Aufrufer bricht.
//!
//! ⚑ **Und damit ist sie Konsensvokabel.** Ihre Kodierung ist ein
//! Protokollvertrag, ihre Prüfung läuft auf jedem Validator, und
//! [`MAX_BLOECKE`] ist deshalb keine Bequemlichkeit, sondern die
//! Schranke, die den Aufwand je Transaktion begrenzt.


use crate::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use crate::hash::Hash;
use crate::ids::{Address, EpochId, SitzungId};

/// Trennstring der Blockunterschrift.
pub const DST_VOLLMACHT: &[u8] = b"MYELITH_GATEWAY_VOLLMACHT_v1";

/// Wie viele Blöcke eine Vollmacht höchstens hat.
///
/// ⚑ **Eine Schranke gegen Arbeit, die der Anfragende bestimmt.** Jeder
/// Block kostet eine Paarung; ohne Grenze wäre eine Vollmacht mit
/// tausend Blöcken eine Sekunde Rechenzeit für ein paar Kilobyte.
/// Dieselbe Klasse wie Fund 141.
///
/// **Acht sind reichlich:** Nutzer, Agent, Unteragent, und noch fünf.
pub const MAX_BLOECKE: usize = 8;

/// Was ein Vorbehalt einschränkt.
///
/// ⚑ **Ein Aufzählungstyp und keine Sprache.** Jeder Fall ist in
/// konstanter Zeit prüfbar; siehe den Modulkopf dazu, warum Biscuits
/// Datalog hier nicht übernommen wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum Vorbehalt {
    /// Gilt nur bis einschliesslich dieser Epoche.
    GueltigBis(EpochId),
    /// Gilt nur für diese Sitzung.
    NurSitzung(SitzungId),
    /// Höchstens so viele Credits je Anfrage.
    HoechstensCredits(u64),
    /// Nur für dieses Modell (Hash des Pipeline-Stands).
    NurModell(Hash),
}

/// Ein Block der Kette.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Block {
    /// Was dieser Block einschränkt.
    pub vorbehalte: Vec<Vorbehalt>,
    /// Der Schlüssel, mit dem der **nächste** Block unterschrieben sein
    /// muss.
    pub naechster: BlsPublicKey,
    /// Unterschrift über [`Block::botschaft`], mit dem Schlüssel des
    /// Vorgängers.
    pub unterschrift: BlsSignature,
}

impl Block {
    /// Was unterschrieben wird: Vorbehalte und der nächste Schlüssel.
    ///
    /// ⚑ **Der nächste Schlüssel gehört zwingend hinein.** Ohne ihn
    /// könnte ein Halter den Block behalten und einen anderen
    /// Nachfolger einsetzen, und die Kette hinge an nichts.
    pub fn botschaft(vorbehalte: &[Vorbehalt], naechster: &BlsPublicKey) -> Vec<u8> {
        let mut msg = Vec::with_capacity(DST_VOLLMACHT.len() + 48 + 64);
        msg.extend_from_slice(DST_VOLLMACHT);
        msg.extend_from_slice(&(vorbehalte.len() as u32).to_le_bytes());
        for v in vorbehalte {
            // Borsh kodiert jeden Fall eindeutig; die Länge davor
            // verhindert, dass zwei Listen dieselbe Bytefolge ergeben.
            let roh = borsh::to_vec(v).unwrap_or_default();
            msg.extend_from_slice(&(roh.len() as u32).to_le_bytes());
            msg.extend_from_slice(&roh);
        }
        msg.extend_from_slice(&naechster.0);
        msg
    }
}

/// Was beim Prüfen einer Vollmacht schiefgehen kann.
///
/// ⚑ **Nur für das Protokoll des Betreibers, nie für die Antwort.** Die
/// Tür sagt nach draussen genau ein Bit; siehe
/// `myl_gateway::zugang::Zugangsbefund`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vollmachtsfehler {
    /// Keine Blöcke: eine Kette ohne Glied.
    Leer,
    /// Mehr als [`MAX_BLOECKE`].
    ZuLang { bloecke: usize },
    /// Der Wurzelschlüssel passt nicht zur Agentenadresse des Kontrakts.
    FremderAgent,
    /// Eine Blockunterschrift gilt nicht.
    KetteGebrochen { block: usize },
    /// Der Nachweis passt nicht zum zuletzt genannten Schlüssel.
    NachweisPasstNicht,
    /// Ein Vorbehalt ist verletzt.
    VorbehaltVerletzt,
}

/// Der Rahmen, gegen den Vorbehalte geprüft werden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anfragerahmen {
    /// Die laufende Epoche.
    pub jetzt: EpochId,
    /// Unter welcher Sitzung gefragt wird.
    pub sitzung: SitzungId,
    /// Was die Anfrage höchstens kosten darf.
    pub credits: u64,
    /// Welches Modell gefragt ist.
    pub modell: Hash,
}

/// Die Vollmacht.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Vollmacht {
    /// Der öffentliche Schlüssel des Agenten.
    ///
    /// ⚑ **Er muss mitkommen**, weil `Sitzungskontrakt::agent` eine
    /// Adresse ist und aus einem Hash kein Schlüssel folgt. Dieselbe
    /// Stelle wie Fund 109.
    pub agent: BlsPublicKey,
    /// Die Kette, beginnend beim Vollmachtsblock.
    pub bloecke: Vec<Block>,
    /// Die Saat des zuletzt genannten Schlüssels.
    ///
    /// ⚑ **Sie ist der Nachweis und zugleich das Recht,
    /// abzuschwächen.** Wer sie hat, kann anhängen; wer sie nicht hat,
    /// kann nichts wegnehmen, denn dazu bräuchte er die Saat des
    /// Vorgängers, und die ist fort.
    pub nachweis: [u8; 32],
}

impl Vollmacht {
    /// Stellt eine Vollmacht aus.
    ///
    /// `saat` ist die Saat des ersten Folgeschlüssels. **Sie kommt vom
    /// Aufrufer und nicht aus einem versteckten Zufallsgenerator**,
    /// damit ein Test dieselbe Vollmacht zweimal bauen kann und
    /// niemand rät, woher die Entropie stammt.
    pub fn ausstellen(
        agent: &BlsSecretKey,
        vorbehalte: Vec<Vorbehalt>,
        saat: [u8; 32],
    ) -> Result<Self, crate::bls::BlsError> {
        let naechster = BlsSecretKey::key_gen(&saat)?.public_key()?;
        let botschaft = Block::botschaft(&vorbehalte, &naechster);
        let unterschrift = agent.sign(&botschaft)?;
        Ok(Self {
            agent: agent.public_key()?,
            bloecke: vec![Block {
                vorbehalte,
                naechster,
                unterschrift,
            }],
            nachweis: saat,
        })
    }

    /// Hängt einen Block an und gibt eine engere Vollmacht zurück.
    ///
    /// ⚑ **Ohne den Aussteller zu fragen.** Das ist der ganze Punkt der
    /// Bauart: Ein Agent kann einem Unteragenten weniger geben, als er
    /// selbst hat, und niemand muss davon wissen.
    ///
    /// **Der alte Nachweis wird verbraucht.** Wer abschwächt, gibt die
    /// neue Saat weiter und wirft die alte weg; behielte er sie, könnte
    /// der Empfänger den Block nicht entfernen, wohl aber er selbst,
    /// und das wäre kein Schaden, denn er hatte die weitere Vollmacht
    /// ohnehin.
    pub fn abschwaechen(
        mut self,
        vorbehalte: Vec<Vorbehalt>,
        saat: [u8; 32],
    ) -> Result<Self, crate::bls::BlsError> {
        let jetziger = BlsSecretKey::key_gen(&self.nachweis)?;
        let naechster = BlsSecretKey::key_gen(&saat)?.public_key()?;
        let botschaft = Block::botschaft(&vorbehalte, &naechster);
        let unterschrift = jetziger.sign(&botschaft)?;
        self.bloecke.push(Block {
            vorbehalte,
            naechster,
            unterschrift,
        });
        self.nachweis = saat;
        Ok(self)
    }

    /// Prüft die Vollmacht gegen eine Agentenadresse und einen Rahmen.
    ///
    /// **Die Kette zuerst, die Vorbehalte danach.** Ein gebrochenes
    /// Glied macht jede weitere Aussage wertlos.
    pub fn pruefen(
        &self,
        agent_adresse: &Address,
        rahmen: &Anfragerahmen,
    ) -> Result<(), Vollmachtsfehler> {
        if self.bloecke.is_empty() {
            return Err(Vollmachtsfehler::Leer);
        }
        if self.bloecke.len() > MAX_BLOECKE {
            return Err(Vollmachtsfehler::ZuLang {
                bloecke: self.bloecke.len(),
            });
        }
        if Address::aus_schluessel(&self.agent) != *agent_adresse {
            return Err(Vollmachtsfehler::FremderAgent);
        }

        // Die Kette: jeder Block ist mit dem Schlüssel unterschrieben,
        // den sein Vorgänger genannt hat.
        let mut voriger = self.agent;
        for (i, b) in self.bloecke.iter().enumerate() {
            let botschaft = Block::botschaft(&b.vorbehalte, &b.naechster);
            if !voriger.verify(&botschaft, &b.unterschrift) {
                return Err(Vollmachtsfehler::KetteGebrochen { block: i });
            }
            voriger = b.naechster;
        }

        // ⚑ **Der Nachweis schliesst die Kette ab und verhindert das
        // Abschneiden.** Wer den letzten Block streicht, hat einen
        // Nachweis, der nicht mehr zum zuletzt genannten Schlüssel
        // passt.
        let aus_nachweis = BlsSecretKey::key_gen(&self.nachweis)
            .and_then(|k| k.public_key())
            .map_err(|_| Vollmachtsfehler::NachweisPasstNicht)?;
        if aus_nachweis != voriger {
            return Err(Vollmachtsfehler::NachweisPasstNicht);
        }

        // Alle Vorbehalte aller Blöcke, ohne Ausnahme.
        if !self.deckt(rahmen) {
            return Err(Vollmachtsfehler::VorbehaltVerletzt);
        }
        Ok(())
    }

    /// Nur die Vorbehalte, ohne die Signaturkette.
    ///
    /// # ⚑ Wofür das gut ist, und wofür ausdrücklich nicht
    ///
    /// Eine Anfrage wird **zweimal** gegen dieselbe Vollmacht gehalten:
    /// einmal beim Eintreffen, wo die Kosten noch nicht feststehen, und
    /// einmal, wenn der Höchstbetrag bekannt ist. Die Signaturkette ein
    /// zweites Mal zu prüfen kostet bis zu [`MAX_BLOECKE`]
    /// Signaturprüfungen und verdoppelt genau den Aufwand, den die
    /// Ratengrenze des Gateways beschränken soll.
    ///
    /// ⚑ **Das ist keine Prüfung der Vollmacht.** Wer nur das hier ruft,
    /// hat nicht festgestellt, dass die Vollmacht echt ist. Sie ist
    /// **nach** [`Self::pruefen`] zu rufen und nie stattdessen.
    pub fn deckt(&self, rahmen: &Anfragerahmen) -> bool {
        self.bloecke
            .iter()
            .all(|b| b.vorbehalte.iter().all(|v| haelt(v, rahmen)))
    }
}

/// Das Alphabet der Textform.
///
/// **Base64 in seiner URL-sicheren Fassung**, damit eine Vollmacht auch
/// in einer Umgebungsvariablen und in einer Kommandozeile steht, ohne
/// dass jemand quotiert.
const ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

impl Vollmacht {
    /// Die Textform, wie sie in `Authorization: Bearer` steht.
    ///
    /// ⚑ **Ohne Polster.** Ein `=` am Ende müsste in einer Kopfzeile
    /// nicht quotiert werden, in einer Umgebungsvariablen und einer
    /// Kommandozeile aber sehr wohl, und es trägt keine Information.
    pub fn als_bearer(&self) -> String {
        let roh = borsh::to_vec(self).unwrap_or_default();
        let mut aus = String::with_capacity(roh.len() * 4 / 3 + 4);
        let mut puffer: u32 = 0;
        let mut bits: u32 = 0;
        for b in roh {
            puffer = (puffer << 8) | b as u32;
            bits += 8;
            while bits >= 6 {
                bits -= 6;
                aus.push(ALPHABET[((puffer >> bits) & 0x3F) as usize] as char);
            }
        }
        if bits > 0 {
            aus.push(ALPHABET[((puffer << (6 - bits)) & 0x3F) as usize] as char);
        }
        aus
    }

    /// Aus der Textform zurück.
    ///
    /// `None` bei allem, was nicht passt, einschliesslich Anhängseln
    /// und nicht kanonischer Kodierung.
    pub fn aus_bearer(s: &str) -> Option<Self> {
        let mut roh = Vec::with_capacity(s.len() * 3 / 4);
        let mut puffer: u32 = 0;
        let mut bits: u32 = 0;
        for z in s.bytes() {
            let wert = ALPHABET.iter().position(|c| *c == z)? as u32;
            puffer = (puffer << 6) | wert;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                roh.push((puffer >> bits) as u8);
            }
        }
        // ⚑ **Übrige Bits müssen null sein.** Sonst gäbe es zwei
        // Zeichenketten für dieselben Bytes, und das ist Formbarkeit:
        // dieselbe Klasse, die das Fuzzing sonst als Kanonizität prüft.
        if bits > 0 && puffer & ((1 << bits) - 1) != 0 {
            return None;
        }
        // ⚑ **Und Borsh muss die Bytes vollständig lesen.** Ein
        // Anhängsel wäre ein zweiter Weg zu derselben Vollmacht.
        let v: Self = borsh::from_slice(&roh).ok()?;
        (borsh::to_vec(&v).ok()? == roh).then_some(v)
    }

    /// Für welche Sitzung diese Vollmacht gilt, falls sie es sagt.
    ///
    /// ⚑ **Aus dem Vorbehalt und nicht aus einer Kopfzeile daneben.**
    /// Stünde sie daneben, könnte jemand eine Vollmacht für Sitzung A
    /// mit der Angabe „Sitzung B" schicken, und die Prüfung liefe gegen
    /// den falschen Kontrakt.
    pub fn sitzung(&self) -> Option<SitzungId> {
        self.bloecke
            .iter()
            .flat_map(|b| b.vorbehalte.iter())
            .find_map(|v| match v {
                Vorbehalt::NurSitzung(s) => Some(*s),
                _ => None,
            })
    }
}

/// Hält ein Vorbehalt im gegebenen Rahmen?
fn haelt(v: &Vorbehalt, r: &Anfragerahmen) -> bool {
    match v {
        Vorbehalt::GueltigBis(bis) => r.jetzt.0 <= bis.0,
        Vorbehalt::NurSitzung(s) => r.sitzung == *s,
        Vorbehalt::HoechstensCredits(c) => r.credits <= *c,
        Vorbehalt::NurModell(m) => r.modell == *m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> BlsSecretKey {
        BlsSecretKey::key_gen(&[7u8; 32]).expect("Schluessel")
    }

    fn adresse() -> Address {
        Address::aus_schluessel(&agent().public_key().expect("pk"))
    }

    fn sitzung(b: u8) -> SitzungId {
        SitzungId::new([b; 32])
    }

    fn rahmen(jetzt: u64, s: u8, credits: u64) -> Anfragerahmen {
        Anfragerahmen {
            jetzt: EpochId(jetzt),
            sitzung: sitzung(s),
            credits,
            modell: Hash::sha256(b"probe-pipeline"),
        }
    }

    /// Eine ausgestellte Vollmacht gilt in ihrem Rahmen.
    #[test]
    fn eine_ausgestellte_vollmacht_gilt() {
        let v = Vollmacht::ausstellen(
            &agent(),
            vec![Vorbehalt::NurSitzung(sitzung(1)), Vorbehalt::GueltigBis(EpochId(100))],
            [1u8; 32],
        )
        .expect("ausstellen");
        assert_eq!(v.pruefen(&adresse(), &rahmen(50, 1, 10)), Ok(()));
    }

    /// Ein fremder Agent kommt nicht durch: Die Wurzel muss zur
    /// Kontraktadresse passen.
    #[test]
    fn eine_fremde_wurzel_wird_abgewiesen() {
        let v = Vollmacht::ausstellen(&agent(), vec![], [1u8; 32]).expect("ausstellen");
        let fremd = Address::aus_schluessel(
            &BlsSecretKey::key_gen(&[9u8; 32]).unwrap().public_key().unwrap(),
        );
        assert_eq!(
            v.pruefen(&fremd, &rahmen(50, 1, 10)),
            Err(Vollmachtsfehler::FremderAgent)
        );
    }

    /// Jeder Vorbehalt greift, und zwar jeder für sich.
    #[test]
    fn jeder_vorbehalt_greift() {
        let v = Vollmacht::ausstellen(
            &agent(),
            vec![
                Vorbehalt::GueltigBis(EpochId(100)),
                Vorbehalt::NurSitzung(sitzung(1)),
                Vorbehalt::HoechstensCredits(500),
                Vorbehalt::NurModell(Hash::sha256(b"probe-pipeline")),
            ],
            [1u8; 32],
        )
        .expect("ausstellen");
        assert_eq!(v.pruefen(&adresse(), &rahmen(100, 1, 500)), Ok(()));

        // Und jede einzelne Verletzung schliesst.
        assert!(v.pruefen(&adresse(), &rahmen(101, 1, 500)).is_err(), "Ablauf");
        assert!(v.pruefen(&adresse(), &rahmen(100, 2, 500)).is_err(), "Sitzung");
        assert!(v.pruefen(&adresse(), &rahmen(100, 1, 501)).is_err(), "Betrag");
        let anderes = Anfragerahmen {
            modell: Hash::sha256(b"ein anderes modell"),
            ..rahmen(100, 1, 500)
        };
        assert!(v.pruefen(&adresse(), &anderes).is_err(), "Modell");
    }

    /// ⚑ **Abschwächen geht ohne den Aussteller**, und die engere
    /// Vollmacht gilt weiterhin.
    #[test]
    fn abschwaechen_braucht_den_aussteller_nicht() {
        let weit = Vollmacht::ausstellen(
            &agent(),
            vec![Vorbehalt::GueltigBis(EpochId(100))],
            [1u8; 32],
        )
        .expect("ausstellen");
        assert_eq!(weit.pruefen(&adresse(), &rahmen(50, 1, 1_000)), Ok(()));

        // Der Halter engt ein, ohne irgendjemanden zu fragen.
        let eng = weit
            .abschwaechen(vec![Vorbehalt::HoechstensCredits(10)], [2u8; 32])
            .expect("abschwaechen");
        assert_eq!(eng.pruefen(&adresse(), &rahmen(50, 1, 10)), Ok(()));
        assert!(
            eng.pruefen(&adresse(), &rahmen(50, 1, 11)).is_err(),
            "der angehaengte Vorbehalt greift nicht"
        );
        // ⚑ **Und der alte Vorbehalt gilt weiter:** Abschwächen nimmt
        // nichts weg.
        assert!(eng.pruefen(&adresse(), &rahmen(101, 1, 10)).is_err());
    }

    /// ⚑ **Der Angriff, gegen den die Bauart gebaut ist: abschneiden.**
    ///
    /// Wer den letzten Block streicht, hätte wieder die weitere
    /// Vollmacht. Sein Nachweis passt dann nicht mehr zum zuletzt
    /// genannten Schlüssel, und einen passenden kann er nicht bauen:
    /// Dazu bräuchte er die Saat des Vorgängers, und die hat der
    /// Abschwächende weggeworfen.
    #[test]
    fn ein_abgeschnittener_block_faellt_durch() {
        let eng = Vollmacht::ausstellen(&agent(), vec![], [1u8; 32])
            .expect("ausstellen")
            .abschwaechen(vec![Vorbehalt::HoechstensCredits(10)], [2u8; 32])
            .expect("abschwaechen");

        let mut abgeschnitten = eng.clone();
        abgeschnitten.bloecke.pop();
        assert_eq!(
            abgeschnitten.pruefen(&adresse(), &rahmen(50, 1, 1_000)),
            Err(Vollmachtsfehler::NachweisPasstNicht),
            "der abgeschnittene Block liess die weitere Vollmacht wieder aufleben"
        );
    }

    /// ⚑ **Und ein ausgetauschter Vorbehalt auch.**
    ///
    /// Wer den Betrag im letzten Block anhebt, bricht dessen
    /// Unterschrift; wer ihn im ersten anhebt, bricht die ganze Kette.
    #[test]
    fn ein_geaenderter_vorbehalt_bricht_die_kette() {
        let v = Vollmacht::ausstellen(
            &agent(),
            vec![Vorbehalt::HoechstensCredits(10)],
            [1u8; 32],
        )
        .expect("ausstellen")
        .abschwaechen(vec![Vorbehalt::HoechstensCredits(5)], [2u8; 32])
        .expect("abschwaechen");

        let mut letzter = v.clone();
        letzter.bloecke[1].vorbehalte = vec![Vorbehalt::HoechstensCredits(1_000)];
        assert_eq!(
            letzter.pruefen(&adresse(), &rahmen(50, 1, 100)),
            Err(Vollmachtsfehler::KetteGebrochen { block: 1 })
        );

        let mut erster = v.clone();
        erster.bloecke[0].vorbehalte = vec![Vorbehalt::HoechstensCredits(1_000)];
        assert_eq!(
            erster.pruefen(&adresse(), &rahmen(50, 1, 100)),
            Err(Vollmachtsfehler::KetteGebrochen { block: 0 })
        );
    }

    /// ⚑ **Ein untergeschobener Nachfolger bricht die Kette.**
    ///
    /// Ohne den nächsten Schlüssel in der Botschaft könnte ein Halter
    /// den Block behalten und einen eigenen Nachfolger einsetzen; dann
    /// hinge die Kette an nichts.
    #[test]
    fn ein_untergeschobener_nachfolger_bricht_die_kette() {
        let v = Vollmacht::ausstellen(&agent(), vec![], [1u8; 32]).expect("ausstellen");
        let mut gefaelscht = v.clone();
        gefaelscht.bloecke[0].naechster = BlsSecretKey::key_gen(&[42u8; 32])
            .unwrap()
            .public_key()
            .unwrap();
        gefaelscht.nachweis = [42u8; 32];
        assert_eq!(
            gefaelscht.pruefen(&adresse(), &rahmen(50, 1, 10)),
            Err(Vollmachtsfehler::KetteGebrochen { block: 0 })
        );
    }

    /// ⚑ **Die Kettenlänge ist gedeckelt**, sonst bestimmt der
    /// Anfragende, wie viele Paarungen die Tür rechnet (Klasse von
    /// Fund 141).
    #[test]
    fn eine_zu_lange_kette_wird_abgewiesen() {
        let mut v = Vollmacht::ausstellen(&agent(), vec![], [0u8; 32]).expect("ausstellen");
        for i in 1..=MAX_BLOECKE as u8 {
            v = v.abschwaechen(vec![], [i; 32]).expect("abschwaechen");
        }
        assert_eq!(v.bloecke.len(), MAX_BLOECKE + 1);
        assert_eq!(
            v.pruefen(&adresse(), &rahmen(50, 1, 10)),
            Err(Vollmachtsfehler::ZuLang {
                bloecke: MAX_BLOECKE + 1
            })
        );
        // Gegenprobe: genau die Grenze geht noch.
        let mut gerade_noch =
            Vollmacht::ausstellen(&agent(), vec![], [0u8; 32]).expect("ausstellen");
        for i in 1..MAX_BLOECKE as u8 {
            gerade_noch = gerade_noch.abschwaechen(vec![], [i; 32]).expect("abschwaechen");
        }
        assert_eq!(gerade_noch.bloecke.len(), MAX_BLOECKE);
        assert_eq!(gerade_noch.pruefen(&adresse(), &rahmen(50, 1, 10)), Ok(()));
    }

    /// ⚑ **Die Textform ist der API-Schlüssel**, und sie kommt
    /// unverändert zurück.
    #[test]
    fn die_textform_ueberlebt_die_kopfzeile() {
        let v = Vollmacht::ausstellen(
            &agent(),
            vec![Vorbehalt::NurSitzung(sitzung(3)), Vorbehalt::GueltigBis(EpochId(9))],
            [1u8; 32],
        )
        .expect("ausstellen");
        let text = v.als_bearer();
        // Kein Polster, nichts, was quotiert werden müsste.
        assert!(
            text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "die Textform enthaelt Zeichen, die quotiert werden muessten: {text}"
        );
        assert_eq!(Vollmacht::aus_bearer(&text), Some(v));
    }

    /// ⚑ **Nicht kanonische Kodierung wird abgewiesen.**
    ///
    /// Sonst gäbe es zwei Zeichenketten für dieselbe Vollmacht, und
    /// das ist Formbarkeit: dieselbe Klasse, die das Fuzzing sonst als
    /// Kanonizität prüft.
    #[test]
    fn zwei_texte_fuer_dieselbe_vollmacht_gibt_es_nicht() {
        let v = Vollmacht::ausstellen(&agent(), vec![], [1u8; 32]).expect("ausstellen");
        let text = v.als_bearer();
        assert!(Vollmacht::aus_bearer(&text).is_some());

        // Ein Anhaengsel ist ein zweiter Weg zu derselben Vollmacht.
        assert_eq!(
            Vollmacht::aus_bearer(&format!("{text}AAAA")),
            None,
            "ein Anhaengsel wurde stillschweigend ueberlesen"
        );
        // Und ein fremdes Zeichen ist keine Kodierung.
        assert_eq!(Vollmacht::aus_bearer(&format!("{text}=")), None);
        assert_eq!(Vollmacht::aus_bearer(&format!("{text}+")), None);
    }

    /// Die Sitzung kommt aus dem Vorbehalt.
    #[test]
    fn die_sitzung_kommt_aus_dem_vorbehalt() {
        let ohne = Vollmacht::ausstellen(&agent(), vec![], [1u8; 32]).expect("ausstellen");
        assert_eq!(ohne.sitzung(), None);
        let mit = Vollmacht::ausstellen(
            &agent(),
            vec![Vorbehalt::NurSitzung(sitzung(5))],
            [1u8; 32],
        )
        .expect("ausstellen");
        assert_eq!(mit.sitzung(), Some(sitzung(5)));
    }

    /// Die Drahtform kommt unverändert zurück.
    #[test]
    fn eine_vollmacht_uebersteht_die_leitung() {
        let v = Vollmacht::ausstellen(
            &agent(),
            vec![Vorbehalt::NurSitzung(sitzung(3))],
            [1u8; 32],
        )
        .expect("ausstellen")
        .abschwaechen(vec![Vorbehalt::HoechstensCredits(7)], [2u8; 32])
        .expect("abschwaechen");
        let roh = borsh::to_vec(&v).expect("kodieren");
        let zurueck: Vollmacht = borsh::from_slice(&roh).expect("dekodieren");
        assert_eq!(zurueck, v);
        assert_eq!(zurueck.pruefen(&adresse(), &rahmen(50, 3, 7)), Ok(()));
    }
}
