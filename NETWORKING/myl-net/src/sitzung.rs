//! Ende-zu-Ende-verschlüsselte Sitzungen zwischen zwei Endpunkten
//! (Punkte 3.1 bis 3.3, Whitepaper Kap. 9.2).
//!
//! # Warum der Transport nicht genügt
//!
//! libp2p verschlüsselt jede Verbindung mit Noise. Für zwei Knoten, die
//! direkt miteinander sprechen, ist damit alles gesagt, und man könnte
//! diese Datei für überflüssig halten. Sie ist es aus zwei Gründen
//! nicht:
//!
//! **Erstens das Gateway.** Nutzer und erster Shard sprechen nicht
//! direkt, sondern über ein Gateway. Mit Transportverschlüsselung
//! allein sind das zwei Verbindungen, und dazwischen liegt Klartext.
//! Genau dieser Sammelpunkt, an dem der Verkehr vieler Nutzer
//! zusammenläuft, ist in Kap. 9.2 als Angreiferklasse benannt: „Da
//! Gateways nur weiterleiten und nicht rechnen, entfällt damit ein
//! Sammelpunkt". Entfallen tut er nur, wenn das Gateway den Inhalt
//! nicht lesen kann.
//!
//! **Zweitens die Herkunft der Zusage.** Eine Eigenschaft, die aus dem
//! Transport kommt, muss jeder neue Transport neu verdienen. Der Weg
//! zwischen zwei Shard-Minern läuft heute über TCP, QUIC oder ein
//! Relais (Punkt 3.4), morgen über etwas anderes, und eine
//! Konfiguration entscheidet darüber. Eine Zusage, die von der
//! Wegwahl abhängt, ist eine Zusage, die eine Fehlkonfiguration still
//! entfernt. Hier hängt sie an der Nutzlast: Wer den Schlüssel nicht
//! hat, sieht Geheimtext, gleich über welchen Weg er ihn bekommt.
//!
//! # Was diese Schicht nicht leistet
//!
//! Sie schützt **nicht** vor den beteiligten Shard-Minern. Deren
//! Aufgabe ist die Verarbeitung des Inhalts; ein Miner, der die
//! Aktivierungen nicht lesen kann, kann nicht rechnen. Kap. 9.2 sagt
//! das ausdrücklich, und Kap. 9.3 zieht daraus die Risikoklasse C
//! („ungeeignet"). Diese Datei verschiebt die Grenze nicht, sie hält
//! sie ein.
//!
//! # Aufbau
//!
//! - [`Epochenschluessel`]: ein X25519-Paar, das **genau eine Epoche**
//!   lebt und danach vernichtet wird.
//! - [`Epochenankuendigung`]: der öffentliche Punkt, unterschrieben mit
//!   des Konsensschlüssels. Ohne sie trägt nichts von dem hier.
//! - [`Kanal`]: zwei abgeleitete Schlüssel (einer je Richtung), ein
//!   Sendezähler, ein Empfangsstand.
//! - [`Sitzungen`]: alle Kanäle einer Epoche zusammen, damit
//!   [`Sitzungen::rotiere`] sie gemeinsam fallen lassen kann.
//!
//! # ⚑ Die Ankündigung ist die Voraussetzung, nicht das Beiwerk
//!
//! Alles hier rechnet gegen den Punkt, den die Gegenstelle angekündigt
//! hat. Wer einen eigenen unterschieben kann, führt beide Seiten in
//! eine Sitzung mit sich selbst, liest mit und reicht weiter, und
//! **kein einziges Tag geht dabei daneben**. Verschlüsselung ohne
//! beglaubigten Schlüsselaustausch ist keine halbe Sicherheit, sie ist
//! gar keine.
//!
//! [`Epochenankuendigung::pruefe`] ist deshalb der einzige Weg, aus
//! einer Ankündigung einen Punkt zu bekommen: Das Feld ist privat, und
//! es gibt keinen Nebenausgang.
//!
//! **Unterschrieben wird mit dem Konsensschlüssel (BLS), nicht mit der
//! Netzidentität.** Weil `MinerId` und `Address` beide `sha256(pubkey)`
//! sind, ist der Endpunkt der Hash des unterschreibenden Schlüssels,
//! und die Prüfung braucht **nichts als den Endpunkt aus dem
//! Pod-Pfad**. Kein Register, keine Zuordnung von `PeerId` zu
//! `MinerId`, kein Aufrufer, der noch etwas beisteuern muss.
//!
//! # ⚑ Der Epochenschlüssel wird gezogen, nicht abgeleitet
//!
//! Naheliegend wäre, ihn aus einer Langzeitsaat und der Epochennummer
//! zu berechnen: kein Zustand auf der Platte, jederzeit nachrechenbar,
//! kein Verlust bei einem Neustart. Und genau das wäre der Fehler. Wer
//! die Saat bekommt, bekommt **jede vergangene Epoche** mit, und das
//! Vorwärtsgeheimnis, das Punkt 3.3 verlangt, gäbe es nur dem Namen
//! nach.
//!
//! Vorwärtsgeheimnis heißt, dass das Geheimnis nirgends mehr herleitbar
//! ist. Also wird es frisch gezogen ([`Epochenschluessel::ziehe`]) und
//! beim Rotieren vernichtet. Der Preis ist ehrlich zu nennen: Ein
//! Knoten, der neu startet, hat einen neuen Epochenschlüssel und muss
//! ihn ankündigen, bevor er wieder empfangen kann.
//!
//! # ⚑ Der Zähler wohnt beim Schlüssel, und das ist kein Zufall
//!
//! ChaCha20-Poly1305 verträgt keine zweite Nachricht mit demselben
//! Nonce unter demselben Schlüssel. Es geht dabei nicht um ein bisschen
//! Vertraulichkeit: Bei Wiederholung fällt die Authentisierung mit, und
//! ein Angreifer kann fälschen.
//!
//! Der Nonce ist hier der Sendezähler. Ein Zähler, der zurückgesetzt
//! wird, während der Schlüssel steht, ist deshalb eine Katastrophe und
//! kein Schönheitsfehler. Er liegt darum **im selben Wert** wie der
//! Schlüssel: Es gibt keinen Weg, den Zähler zurückzusetzen, ohne einen
//! neuen [`Kanal`] zu bauen, und ein neuer Kanal hat neue Schlüssel.
//! Die Regel ist nicht dokumentiert, sie ist gebaut.

use std::collections::BTreeMap;
use std::fmt;

use borsh::{BorshDeserialize, BorshSerialize};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use myl_types::bls::{BlsPublicKey, BlsSecretKey, BlsSignature, BLS_PK_LEN};
use myl_types::ids::{EpochId, PodId};
use sha2::{Digest, Sha256};
use ml_kem::array::Array;
use ml_kem::kem::{Decapsulate, Encapsulate, FromSeed, KeyExport};
use ml_kem::{DecapsulationKey768, EncapsulationKey768, MlKem768};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

/// Trennzeichenkette der Schlüsselableitung. Konsens-Feld: Zwei Knoten
/// mit verschiedenen Kennungen leiten verschiedene Schlüssel ab und
/// verstehen einander nicht.
pub const SITZUNG_KENNUNG: &str = "myelith-sitzung-1";

/// Länge eines abgeleiteten Sitzungsschlüssels.
pub const SCHLUESSEL_LEN: usize = 32;

/// Länge eines Kapselpunkts: der öffentliche ML-KEM-768-Schlüssel.
pub const KAPSELPUNKT_LEN: usize = 1184;

/// Länge einer Kapsel: das ML-KEM-768-Chiffrat.
pub const KAPSEL_LEN: usize = 1088;

/// Trenner für die Ableitung der KEM-Saat aus der Sitzungssaat.
const KEM_SAAT_INFO: &[u8] = b"myelith-sitzung-kem-saat-v1";

/// Länge des Poly1305-Tags, das `encrypt` an den Geheimtext hängt.
pub const TAG_LEN: usize = 16;

/// Borsh-Länge eines [`Kopf`]: 8 (Epoche) + 32 (Pod) + 32 + 32 + 8.
///
/// Ein Test rechnet das nach. Wer dem Kopf ein Feld hinzufügt und die
/// Konstante vergisst, bekommt einen roten Test statt einer Grenze, die
/// um die Differenz zu großzügig ist.
pub const KOPF_BYTES: usize = 112;

/// Größte Klartextlänge, die durch den Anfragekanal passt.
///
/// Abgeleitet, nicht gesetzt: Eine versiegelte Nachricht reist als
/// Nutzlast über [`crate::anfrage`], und dort gilt
/// [`crate::anfrage::MAX_ANFRAGE_BYTES`]. Vom Budget gehen der Kopf, das
/// Tag und die vier Bytes Borsh-Längenpräfix des Geheimtexts ab.
pub const MAX_KLARTEXT_BYTES: usize =
    crate::anfrage::MAX_ANFRAGE_BYTES - KOPF_BYTES - TAG_LEN - 4;

/// Ein Endpunkt einer Sitzung: 32 Bytes, mehr weiß diese Schicht nicht.
///
/// Für einen Shard-Übergang ist das eine `MinerId`, für eine
/// Nutzer-Sitzung eine Konto-`Address`. Die Sitzungsschicht
/// unterscheidet die beiden **bewusst nicht**. Sie verschlüsselt
/// zwischen zwei Punkten; wer diese Punkte sind, entscheidet die
/// Anwendung. Stünde hier ein `MinerId`, könnte ein Nutzer keine
/// Sitzung führen, ohne einer zu sein, und `myl-net` wüsste plötzlich,
/// was ein Miner ist. Dieselbe Trennung hält [`crate::anfrage`] für
/// seine Nutzlast ein.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct Endpunkt([u8; 32]);

impl Endpunkt {
    /// Aus 32 rohen Bytes.
    pub fn aus_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Die rohen Bytes.
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl<T: AsRef<[u8]>> From<&T> for Endpunkt {
    /// Aus jeder 32-Byte-Kennung von `myl-types` (`MinerId`, `Address`).
    ///
    /// Kürzere Kennungen werden rechts mit Nullen aufgefüllt, längere
    /// abgeschnitten; beides kommt bei den 32-Byte-Typen aus
    /// `myl_types::ids` nicht vor.
    fn from(quelle: &T) -> Self {
        let roh = quelle.as_ref();
        let mut bytes = [0u8; 32];
        let n = roh.len().min(32);
        bytes[..n].copy_from_slice(&roh[..n]);
        Self(bytes)
    }
}

impl fmt::Display for Endpunkt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0[..4] {
            write!(f, "{byte:02x}")?;
        }
        write!(f, "..")
    }
}

/// Der öffentliche X25519-Punkt eines Knotens für eine Epoche.
///
/// Er wird angekündigt, nicht geheim gehalten. Wer ihn kennt, kann
/// nichts damit anfangen, solange er nicht das zugehörige Geheimnis
/// hat.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct Epochenpunkt([u8; 32]);

impl Epochenpunkt {
    /// Aus 32 rohen Bytes, wie sie über das Netz kommen.
    pub fn aus_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Die rohen Bytes zum Ankündigen.
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Woher das Schlüsselmaterial stammt.
///
/// Dieselbe Trennung wie beim Konsensschlüssel in NODE: Ein Probelauf
/// ist erlaubt, aber er sagt es. Es gibt keinen Vorgabewert und keinen
/// Schalter, den jemand vergisst, sondern zwei verschiedene Aufrufe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Herkunft {
    /// Aus dem Zufallsgenerator des Betriebssystems gezogen.
    Gezogen,
    /// Aus einer festen Saat gebaut, für Tests und Probeläufe.
    Probelauf,
}

impl fmt::Display for Herkunft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Herkunft::Gezogen => write!(f, "gezogen"),
            Herkunft::Probelauf => write!(f, "probelauf"),
        }
    }
}

/// Das X25519-Paar eines Knotens für **genau eine** Epoche.
///
/// Das Geheimnis wird beim Fallenlassen überschrieben (`zeroize`).
/// Ehrlich vermerkt, weil es sonst als Zusage gelesen wird: Das deckt
/// die Bytes dieses Werts ab. Kopien, die der Allokator, ein
/// Speicherauszug oder die Auslagerungsdatei angelegt hat, deckt es
/// nicht.
/// Der öffentliche ML-KEM-768-Schlüssel einer Epoche.
///
/// Das Gegenstück zum [`Epochenpunkt`], für den zweiten Zweig des
/// hybriden Austauschs. Er wird zusammen mit dem Epochenpunkt
/// angekündigt und mit derselben Signatur gedeckt.
#[derive(Clone, PartialEq, Eq)]
pub struct Kapselpunkt(Box<[u8; KAPSELPUNKT_LEN]>);

impl Kapselpunkt {
    /// Aus rohen Bytes, ohne Prüfung der inneren Struktur.
    ///
    /// ⚑ **Eine Strukturprüfung gibt es bei ML-KEM praktisch nicht**, und
    /// das ist kein Mangel dieser Umsetzung: Fast jede Bytefolge der
    /// richtigen Länge kodiert einen gültigen Schlüssel. Ein
    /// verfälschter Kapselpunkt fällt deshalb nicht hier auf, sondern
    /// erst am Tag der ersten Nachricht, weil beide Seiten dann
    /// verschiedene Geheimnisse ableiten. Das ist dieselbe Wirkung wie
    /// bei einem verfälschten Chiffrat und die richtige.
    pub fn aus_bytes(roh: [u8; KAPSELPUNKT_LEN]) -> Self {
        Self(Box::new(roh))
    }

    /// Die rohen Bytes.
    pub fn bytes(&self) -> &[u8; KAPSELPUNKT_LEN] {
        &self.0
    }

    fn schluessel(&self) -> Result<EncapsulationKey768, SitzungsFehler> {
        let feld = Array::try_from(&self.0[..]).map_err(|_| SitzungsFehler::SchluesselUngueltig)?;
        EncapsulationKey768::new(&feld).map_err(|_| SitzungsFehler::SchluesselUngueltig)
    }
}

impl borsh::BorshSerialize for Kapselpunkt {
    /// Roh, ohne Längenpräfix: Die Länge ist eine Konstante des
    /// Verfahrens und keine Angabe des Absenders.
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        writer.write_all(&self.0[..])
    }
}

impl borsh::BorshDeserialize for Kapselpunkt {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let mut roh = Box::new([0u8; KAPSELPUNKT_LEN]);
        reader.read_exact(&mut roh[..])?;
        Ok(Self(roh))
    }
}

impl fmt::Debug for Kapselpunkt {
    /// Gekürzt: 1184 Bytes im Protokoll helfen niemandem.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Kapselpunkt({:02x}{:02x}..{:02x}{:02x})",
            self.0[0], self.0[1], self.0[KAPSELPUNKT_LEN - 2], self.0[KAPSELPUNKT_LEN - 1]
        )
    }
}

/// Die beiden öffentlichen Punkte einer Gegenstelle für eine Epoche.
///
/// Ergebnis einer geprüften [`Epochenankuendigung`]. Beide Punkte
/// zusammen, weil beide von derselben Signatur gedeckt sind und keiner
/// allein einen Kanal trägt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gegenpunkte {
    /// Der X25519-Punkt.
    pub punkt: Epochenpunkt,
    /// Der ML-KEM-Punkt.
    pub kapselpunkt: Kapselpunkt,
}

/// Das Chiffrat, das eine Senderichtung eröffnet.
///
/// ⚑ **Hier endet die Nicht-Interaktivität des alten Entwurfs, und das
/// ist keine Umsetzungsschwäche, sondern die Natur eines KEM.** Zwei
/// Diffie-Hellman-Punkte ergeben von selbst ein gemeinsames Geheimnis;
/// niemand muss etwas schicken. Ein KEM hat eine Richtung: Der Sender
/// kapselt gegen den Schlüssel des Empfängers, und das dabei entstehende
/// Chiffrat **muss übertragen werden**, sonst kann der Empfänger nichts
/// ableiten.
///
/// Jede Seite kapselt für ihre **eigene Senderichtung**. Die Kapsel ist
/// öffentlich; sie zu lesen nützt niemandem, sie zu verändern führt beim
/// Empfänger auf einen anderen Schlüssel und damit auf einen
/// fehlschlagenden Tag. **Sie braucht deshalb keine eigene Signatur.**
#[derive(Clone, PartialEq, Eq)]
pub struct Kapsel {
    epoche: EpochId,
    pod: PodId,
    von: Endpunkt,
    an: Endpunkt,
    chiffrat: Box<[u8; KAPSEL_LEN]>,
}

impl Kapsel {
    /// Die Epoche, für die sie gilt.
    pub fn epoche(&self) -> EpochId {
        self.epoche
    }

    /// Der Pod.
    pub fn pod(&self) -> PodId {
        self.pod
    }

    /// Der Absender.
    pub fn von(&self) -> Endpunkt {
        self.von
    }

    /// Der Empfänger.
    pub fn an(&self) -> Endpunkt {
        self.an
    }

    /// Das rohe Chiffrat.
    pub fn chiffrat(&self) -> &[u8; KAPSEL_LEN] {
        &self.chiffrat
    }

    /// Auf die Leitung: `epoche ‖ pod ‖ von ‖ an ‖ chiffrat`.
    pub fn zu_bytes(&self) -> Vec<u8> {
        let mut roh = Vec::with_capacity(8 + 32 + 32 + 32 + KAPSEL_LEN);
        roh.extend_from_slice(&self.epoche.0.to_le_bytes());
        roh.extend_from_slice(self.pod.as_bytes());
        roh.extend_from_slice(self.von.bytes());
        roh.extend_from_slice(self.an.bytes());
        roh.extend_from_slice(&self.chiffrat[..]);
        roh
    }

    /// Von der Leitung.
    pub fn aus_bytes(roh: &[u8]) -> Result<Self, SitzungsFehler> {
        const KOPF: usize = 8 + 32 + 32 + 32;
        if roh.len() != KOPF + KAPSEL_LEN {
            return Err(SitzungsFehler::UnleserlicherRahmen);
        }
        let mut acht = [0u8; 8];
        acht.copy_from_slice(&roh[0..8]);
        let mut pod = [0u8; 32];
        pod.copy_from_slice(&roh[8..40]);
        let mut von = [0u8; 32];
        von.copy_from_slice(&roh[40..72]);
        let mut an = [0u8; 32];
        an.copy_from_slice(&roh[72..104]);
        let mut chiffrat = Box::new([0u8; KAPSEL_LEN]);
        chiffrat.copy_from_slice(&roh[KOPF..]);
        Ok(Self {
            epoche: EpochId(u64::from_le_bytes(acht)),
            pod: PodId::new(pod),
            von: Endpunkt::aus_bytes(von),
            an: Endpunkt::aus_bytes(an),
            chiffrat,
        })
    }
}

impl fmt::Debug for Kapsel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Kapsel")
            .field("epoche", &self.epoche)
            .field("pod", &self.pod)
            .field("von", &self.von)
            .field("an", &self.an)
            .finish_non_exhaustive()
    }
}

pub struct Epochenschluessel {
    epoche: EpochId,
    geheim: StaticSecret,
    oeffentlich: Epochenpunkt,
    kem_geheim: DecapsulationKey768,
    kem_oeffentlich: Kapselpunkt,
    herkunft: Herkunft,
}

/// Ein ML-KEM-768-Paar aus einer 64-Byte-Saat.
///
/// Deterministisch: dieselbe Saat ergibt dasselbe Paar. Das ist die
/// Voraussetzung dafür, dass ein Probelauf reproduzierbar bleibt.
fn kem_paar(saat: &[u8; 64]) -> (DecapsulationKey768, Kapselpunkt) {
    let (dk, ek) = MlKem768::from_seed(&Array(*saat));
    let bytes = ek.to_bytes();
    let mut roh = [0u8; KAPSELPUNKT_LEN];
    roh.copy_from_slice(&bytes[..]);
    (dk, Kapselpunkt(Box::new(roh)))
}

impl fmt::Debug for Epochenschluessel {
    /// Ohne das Geheimnis. Ein `Debug`, das einen geheimen Schlüssel in
    /// eine Protokollzeile schreibt, ist ein Leck mit Zeilennummer.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Epochenschluessel")
            .field("epoche", &self.epoche)
            .field("herkunft", &self.herkunft)
            .finish_non_exhaustive()
    }
}

impl Epochenschluessel {
    /// Frisch aus dem Zufallsgenerator des Betriebssystems.
    pub fn ziehe(epoche: EpochId) -> Self {
        let geheim = StaticSecret::random_from_rng(rand_core::OsRng);
        let oeffentlich = Epochenpunkt(PublicKey::from(&geheim).to_bytes());
        let mut kem_saat = Zeroizing::new([0u8; 64]);
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, kem_saat.as_mut());
        let (kem_geheim, kem_oeffentlich) = kem_paar(&kem_saat);
        Self {
            epoche,
            geheim,
            oeffentlich,
            kem_geheim,
            kem_oeffentlich,
            herkunft: Herkunft::Gezogen,
        }
    }

    /// Aus einer festen Saat, für Tests und Probeläufe.
    ///
    /// Trägt [`Herkunft::Probelauf`], damit ein Knoten, der so startet,
    /// es in seinem Protokoll stehen hat.
    pub fn probe(epoche: EpochId, saat: [u8; 32]) -> Self {
        let geheim = StaticSecret::from(saat);
        let oeffentlich = Epochenpunkt(PublicKey::from(&geheim).to_bytes());
        // Die KEM-Saat wird aus derselben Saat abgeleitet, damit ein
        // Probelauf **eine** Zahl braucht und nicht zwei. Getrennt
        // gehalten sind die beiden Zweige trotzdem, denn HKDF mit
        // eigenem `info` gibt aus derselben Quelle unabhängige Ausgaben.
        let hk = Hkdf::<Sha256>::new(None, &saat);
        let mut kem_saat = Zeroizing::new([0u8; 64]);
        hk.expand(KEM_SAAT_INFO, kem_saat.as_mut())
            .expect("64 Bytes liegen weit unter der HKDF-Grenze");
        let (kem_geheim, kem_oeffentlich) = kem_paar(&kem_saat);
        Self {
            epoche,
            geheim,
            oeffentlich,
            kem_geheim,
            kem_oeffentlich,
            herkunft: Herkunft::Probelauf,
        }
    }

    /// Der öffentliche ML-KEM-Punkt dieser Epoche.
    pub fn kapselpunkt(&self) -> Kapselpunkt {
        self.kem_oeffentlich.clone()
    }

    /// Die Epoche, für die dieser Schlüssel gilt.
    pub fn epoche(&self) -> EpochId {
        self.epoche
    }

    /// Der öffentliche Punkt zum Ankündigen.
    pub fn punkt(&self) -> Epochenpunkt {
        self.oeffentlich
    }

    /// Woher das Material stammt.
    pub fn herkunft(&self) -> Herkunft {
        self.herkunft
    }
}

/// Der Klartextkopf einer versiegelten Nachricht.
///
/// # Warum er im Klartext steht
///
/// Ein Gateway muss weiterleiten können, ohne zu lesen. Dafür braucht es
/// Empfänger und Epoche, und beides muss es sehen. Der Inhalt steht
/// nicht darin.
///
/// # ⚑ Und warum er trotzdem nicht änderbar ist
///
/// Ein Klartextkopf, den niemand prüft, ist eine Einladung: Das Gateway
/// schriebe `an` um und leitete die Nachricht an einen Dritten, oder es
/// setzte `epoche` zurück. Der Kopf geht deshalb vollständig als
/// zusätzliche authentisierte Daten in das AEAD ein. Ein geändertes Byte
/// im Kopf, und das Tag stimmt nicht mehr. Das Gateway darf lesen, was
/// es zum Weiterleiten braucht, und nichts davon ändern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Kopf {
    /// Epoche, deren Schlüssel gilt.
    pub epoche: EpochId,
    /// Pod, zu dem die Sitzung gehört.
    pub pod: PodId,
    /// Absender.
    pub von: Endpunkt,
    /// Empfänger.
    pub an: Endpunkt,
    /// Sendezähler des Absenders, zugleich der Nonce.
    pub zaehler: u64,
}

/// Eine versiegelte Nachricht, wie sie über den Draht geht.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Versiegelt {
    /// Klartext, vollständig authentisiert.
    pub kopf: Kopf,
    /// Geheimtext samt angehängtem Tag.
    pub geheimtext: Vec<u8>,
}

impl Versiegelt {
    /// Kanonische Bytes für den Transport über [`crate::anfrage`].
    pub fn zu_bytes(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("Versiegelt ist borsh-serialisierbar")
    }

    /// Zurück aus den Bytes vom Draht.
    pub fn aus_bytes(roh: &[u8]) -> Result<Self, SitzungsFehler> {
        borsh::from_slice(roh).map_err(|_| SitzungsFehler::UnleserlicherRahmen)
    }
}

/// Was beim Versiegeln oder Öffnen schiefgehen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SitzungsFehler {
    /// Absender und Empfänger sind derselbe Endpunkt.
    ///
    /// Dann wären beide Richtungsschlüssel gleich, beide Richtungen
    /// teilten sich einen Nonce-Raum, und die erste Antwort auf die
    /// erste Nachricht wiederholte einen Nonce. Deshalb hier und nicht
    /// später.
    EndpunkteGleich { endpunkt: Endpunkt },
    /// Der Punkt der Gegenstelle hat kleine Ordnung: Das gemeinsame
    /// Geheimnis wäre null, unabhängig vom eigenen Schlüssel.
    PunktOhneBeitrag,
    /// Der Sendezähler ist erschöpft.
    ZaehlerErschoepft,
    /// Der Klartext ist größer als [`MAX_KLARTEXT_BYTES`].
    ZuGross { bytes: usize, grenze: usize },
    /// Der Kopf gehört nicht zu diesem Kanal.
    KopfPasstNicht,
    /// Die Epoche des Kopfes ist nicht die des Schlüssels.
    ///
    /// Nach einer Rotation ist das der Normalfall und kein Angriff: Der
    /// Schlüssel der alten Epoche ist vernichtet.
    EpocheVorbei { kopf: EpochId, kanal: EpochId },
    /// Der Zähler wurde schon gesehen: Wiedereinspielung.
    Wiedereinspielung { zaehler: u64, gesehen_bis: u64 },
    /// Das Tag stimmt nicht. Falscher Schlüssel oder veränderte Bytes.
    TagStimmtNicht,
    /// Die Bytes vom Draht sind kein [`Versiegelt`].
    UnleserlicherRahmen,
    /// Rotation auf eine Epoche, die nicht nach der laufenden liegt.
    RotationRueckwaerts { von: EpochId, nach: EpochId },
    /// Die Signatur der Ankündigung stimmt nicht.
    ///
    /// Genau hier sitzt der Mann in der Mitte: Wer einen fremden
    /// Epochenpunkt unterschieben kann, führt beide Seiten in eine
    /// Sitzung mit sich selbst.
    SignaturStimmtNicht,
    /// Die Ankündigung gilt für eine andere Epoche.
    AnkuendigungFuerAndereEpoche { ankuendigung: EpochId, erwartet: EpochId },
    /// Die Ankündigung gehört zu einem anderen Endpunkt.
    ///
    /// Der Endpunkt ist der Hash des unterschreibenden Schlüssels; wer
    /// hier danebenliegt, hat die Ankündigung eines anderen.
    EndpunktPasstNicht { erwartet: Endpunkt, bekommen: Endpunkt },
    /// Der mitgeführte Konsensschlüssel ist kein gültiger Gruppenpunkt.
    SchluesselUngueltig,
    /// Es soll geöffnet werden, aber die Kapsel der Gegenstelle fehlt
    /// noch. Kein Angriff, sondern eine Reihenfolge: Der
    /// Empfangsschlüssel entsteht erst aus ihrem Chiffrat.
    EmpfangNochNichtBereit,
    /// Eine Kapsel gehört nicht zu diesem Kanal.
    KapselPasstNicht,
    /// Die Identität konnte nicht signieren.
    SignierenGescheitert,
}

impl fmt::Display for SitzungsFehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SitzungsFehler::EndpunkteGleich { endpunkt } => write!(
                f,
                "Absender und Empfänger sind derselbe Endpunkt {endpunkt}: \
                 beide Richtungen teilten sich einen Schlüssel"
            ),
            SitzungsFehler::PunktOhneBeitrag => write!(
                f,
                "der Punkt der Gegenstelle hat kleine Ordnung: \
                 das gemeinsame Geheimnis wäre null"
            ),
            SitzungsFehler::ZaehlerErschoepft => {
                write!(f, "Sendezähler erschöpft: der Kanal muss neu gebaut werden")
            }
            SitzungsFehler::ZuGross { bytes, grenze } => {
                write!(f, "Klartext {bytes} Bytes, Grenze {grenze}")
            }
            SitzungsFehler::KopfPasstNicht => {
                write!(f, "der Kopf gehört nicht zu diesem Kanal")
            }
            SitzungsFehler::EpocheVorbei { kopf, kanal } => write!(
                f,
                "Nachricht aus Epoche {kopf}, Kanal steht in Epoche {kanal}: \
                 der alte Schlüssel ist vernichtet"
            ),
            SitzungsFehler::Wiedereinspielung {
                zaehler,
                gesehen_bis,
            } => write!(
                f,
                "Zähler {zaehler} schon gesehen (Stand {gesehen_bis}): Wiedereinspielung"
            ),
            SitzungsFehler::TagStimmtNicht => {
                write!(f, "Tag stimmt nicht: falscher Schlüssel oder veränderte Bytes")
            }
            SitzungsFehler::UnleserlicherRahmen => {
                write!(f, "die Bytes sind keine versiegelte Nachricht")
            }
            SitzungsFehler::RotationRueckwaerts { von, nach } => write!(
                f,
                "Rotation von Epoche {von} auf {nach}: eine Rotation geht vorwärts"
            ),
            SitzungsFehler::SignaturStimmtNicht => write!(
                f,
                "die Signatur der Epochenankündigung stimmt nicht: \
                 der Punkt gehört nicht zu diesem Konsensschlüssel"
            ),
            SitzungsFehler::AnkuendigungFuerAndereEpoche {
                ankuendigung,
                erwartet,
            } => write!(
                f,
                "Ankündigung für Epoche {ankuendigung}, erwartet wird {erwartet}"
            ),
            SitzungsFehler::EndpunktPasstNicht { erwartet, bekommen } => write!(
                f,
                "Ankündigung gehört zu Endpunkt {bekommen}, erwartet wird {erwartet}"
            ),
            SitzungsFehler::EmpfangNochNichtBereit => write!(
                f,
                "Empfangsrichtung noch nicht bereit: die Kapsel der Gegenstelle fehlt"
            ),
            SitzungsFehler::KapselPasstNicht => {
                write!(f, "die Kapsel gehört nicht zu diesem Kanal")
            }
            SitzungsFehler::SchluesselUngueltig => write!(
                f,
                "der mitgeführte Konsensschlüssel ist kein gültiger Gruppenpunkt"
            ),
            SitzungsFehler::SignierenGescheitert => {
                write!(f, "der Konsensschlüssel konnte die Ankündigung nicht signieren")
            }
        }
    }
}

impl std::error::Error for SitzungsFehler {}

/// Trennzeichenkette der Ankündigungs-Signatur.
///
/// Eigene Kennung im Nachrichtenpräfix, gebaut wie `DST_POI_BUNDLE` in
/// CONSENSUS: Derselbe BLS-Schlüssel unterschreibt Stimmen und Bündel,
/// und eine Unterschrift unter einer Epochenankündigung darf niemals
/// als Unterschrift unter etwas anderes durchgehen.
pub const DST_EPOCHENPUNKT: &[u8] = b"MYELITH_EPOCHENPUNKT_v1";

/// Der Endpunkt, der zu einem BLS-Schlüssel gehört: `sha256(pubkey)`.
///
/// **Dieselbe Ableitung wie `MinerId` und `Address`.** Genau darauf
/// beruht die Ankündigung: Wer den Schlüssel hat, hat den Endpunkt, und
/// niemand sonst. Ein Gleichstandstest in NODE hält die beiden Seiten
/// zusammen.
pub fn endpunkt_aus_schluessel(pubkey: &BlsPublicKey) -> Endpunkt {
    let mut hasher = Sha256::new();
    hasher.update(pubkey.0);
    Endpunkt(hasher.finalize().into())
}

/// Die unterschriebene Ankündigung eines Epochenpunkts.
///
/// # ⚑ Ohne sie ist die ganze Verschlüsselung wertlos
///
/// [`Kanal::neu`] rechnet gegen den Punkt, den die Gegenstelle
/// angekündigt hat. Kann jemand einen eigenen Punkt unterschieben, führt
/// er beide Seiten in eine Sitzung mit sich selbst, liest alles mit und
/// reicht es weiter. **Kein einziges Tag geht dabei daneben.** Jede
/// Zusage der Sitzungsschicht hängt daran, dass der Punkt zu der
/// Gegenstelle gehört, die der Pod-Pfad nennt.
///
/// # ⚑ Warum mit dem Konsensschlüssel und nicht mit der Netzidentität
///
/// Die erste Fassung vom 2026-08-27 unterschrieb mit der
/// **Netzidentität**, also dem Schlüssel hinter der `PeerId`. Das war
/// prüfbar und trotzdem unbrauchbar: Der Pod-Pfad nennt `MinerId`s, und
/// die Frage an dieser Stelle lautet „gehört dieser Punkt zu MinerId
/// Y?". Eine Unterschrift der `PeerId` X beantwortet eine andere Frage,
/// und **die Zuordnung von X zu Y gibt es im Protokoll nirgends**. Die
/// Prüfung war echt, sie prüfte nur das Falsche.
///
/// Seitdem unterschreibt der **Konsensschlüssel** (BLS). Das ist kein
/// Zusatz, sondern eine Vereinfachung: Weil `MinerId = sha256(pubkey)`
/// gilt, trägt die Ankündigung ihren eigenen Prüfstein mit sich, und
/// [`Epochenankuendigung::pruefe`] braucht **nichts als den erwarteten
/// Endpunkt**. Kein Register, keine Zuordnungstabelle, kein Aufrufer,
/// der noch etwas beisteuern muss und es vergessen kann.
///
/// # Warum es keinen Zugriff auf den Punkt ohne Prüfung gibt
///
/// `punkt` ist privat, und [`Epochenankuendigung::pruefe`] ist der
/// einzige Weg heraus. Ein `pub`-Feld daneben wäre eine Abkürzung, die
/// irgendwann jemand nimmt, und sie sähe an der Aufrufstelle harmlos
/// aus. Dieselbe Überlegung wie beim Sendezähler: Die Regel steht nicht
/// im Kommentar, sie steht im Typ.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Epochenankuendigung {
    epoche: EpochId,
    punkt: Epochenpunkt,
    kapselpunkt: Kapselpunkt,
    pubkey: BlsPublicKey,
    signatur: BlsSignature,
}

/// Die Bytes, über die unterschrieben wird.
///
/// Aufbau: `DST ‖ u64_le(epoche) ‖ pubkey ‖ punkt`. Der eigene
/// öffentliche Schlüssel steht mit darin, damit die Nachricht ohne
/// Nebenwissen eindeutig ist.
fn ankuendigungsbytes(
    epoche: EpochId,
    pubkey: &BlsPublicKey,
    punkt: &Epochenpunkt,
    kapselpunkt: &Kapselpunkt,
) -> Vec<u8> {
    let mut msg =
        Vec::with_capacity(DST_EPOCHENPUNKT.len() + 8 + BLS_PK_LEN + 32 + KAPSELPUNKT_LEN);
    msg.extend_from_slice(DST_EPOCHENPUNKT);
    msg.extend_from_slice(&epoche.0.to_le_bytes());
    msg.extend_from_slice(&pubkey.0);
    msg.extend_from_slice(punkt.bytes());
    msg.extend_from_slice(kapselpunkt.bytes());
    msg
}

impl Epochenankuendigung {
    /// Kündigt den eigenen Epochenpunkt an, unterschrieben mit dem
    /// Konsensschlüssel.
    pub fn neu(
        konsens: &BlsSecretKey,
        schluessel: &Epochenschluessel,
    ) -> Result<Self, SitzungsFehler> {
        let pubkey = konsens
            .public_key()
            .map_err(|_| SitzungsFehler::SignierenGescheitert)?;
        let punkt = schluessel.punkt();
        let kapselpunkt = schluessel.kapselpunkt();
        let bytes = ankuendigungsbytes(schluessel.epoche(), &pubkey, &punkt, &kapselpunkt);
        let signatur = konsens
            .sign(&bytes)
            .map_err(|_| SitzungsFehler::SignierenGescheitert)?;
        Ok(Self {
            epoche: schluessel.epoche(),
            punkt,
            kapselpunkt,
            pubkey,
            signatur,
        })
    }

    /// Die Epoche, für die die Ankündigung gilt. Ungeprüft, und nur zum
    /// Einsortieren gedacht.
    pub fn epoche(&self) -> EpochId {
        self.epoche
    }

    /// Der Endpunkt, den diese Ankündigung **behauptet**.
    ///
    /// Abgeleitet aus dem mitgeführten Schlüssel, nicht daneben
    /// gespeichert: Zwei Felder, die dasselbe sagen sollen, sagen
    /// irgendwann Verschiedenes. Ungeprüft bleibt es trotzdem, denn wer
    /// die Unterschrift nicht angesehen hat, weiß nur, was dasteht.
    pub fn behaupteter_endpunkt(&self) -> Endpunkt {
        endpunkt_aus_schluessel(&self.pubkey)
    }

    /// Prüft die Ankündigung und gibt den Punkt heraus.
    ///
    /// `erwarteter` ist der Endpunkt, mit dem gesprochen werden soll,
    /// also die `MinerId` aus dem Pod-Pfad oder die `Address` des
    /// Nutzers. Mehr braucht diese Prüfung nicht: Weil der Endpunkt der
    /// Hash des unterschreibenden Schlüssels ist, ist die Frage „gehört
    /// dieser Punkt zu dieser Gegenstelle?" vollständig hier
    /// beantwortbar.
    pub fn pruefe(
        &self,
        erwarteter: Endpunkt,
        erwartete_epoche: EpochId,
    ) -> Result<Gegenpunkte, SitzungsFehler> {
        if self.epoche != erwartete_epoche {
            // Vor allem anderen: Eine gültig unterschriebene Ankündigung
            // aus einer alten Epoche ist echt und trotzdem falsch, und
            // ohne diese Prüfung wäre sie ein Weg, die Rotation
            // zurückzudrehen.
            return Err(SitzungsFehler::AnkuendigungFuerAndereEpoche {
                ankuendigung: self.epoche,
                erwartet: erwartete_epoche,
            });
        }
        // Vor dem Hashen: Ein Schlüssel, der kein gültiger Gruppenpunkt
        // ist, hat trotzdem einen Hash, und der könnte passen.
        if self.pubkey.validate().is_err() {
            return Err(SitzungsFehler::SchluesselUngueltig);
        }
        let bekommen = endpunkt_aus_schluessel(&self.pubkey);
        if bekommen != erwarteter {
            return Err(SitzungsFehler::EndpunktPasstNicht {
                erwartet: erwarteter,
                bekommen,
            });
        }
        let bytes = ankuendigungsbytes(self.epoche, &self.pubkey, &self.punkt, &self.kapselpunkt);
        if !self.pubkey.verify(&bytes, &self.signatur) {
            return Err(SitzungsFehler::SignaturStimmtNicht);
        }
        Ok(Gegenpunkte {
            punkt: self.punkt,
            kapselpunkt: self.kapselpunkt.clone(),
        })
    }
}

/// Das Salz der Ableitung: bindet den Schlüssel an Kennung, Epoche und
/// Pod.
///
/// Ohne Epoche im Salz überlebte ein Schlüssel die Rotation, ohne Pod
/// wäre derselbe Schlüssel in zwei Pods gültig, und ein Miner, der in
/// beiden sitzt, trüge Nachrichten von einem in den anderen.
fn salz(epoche: EpochId, pod: &PodId) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SITZUNG_KENNUNG.as_bytes());
    hasher.update(epoche.0.to_le_bytes());
    hasher.update(pod.as_ref());
    hasher.finalize().into()
}

/// Ein Richtungsschlüssel: `von` nach `an`, nicht umgekehrt.
///
/// Die Reihenfolge im `info` ist die ganze Richtungstrennung. Zwei
/// Aufrufe mit vertauschten Endpunkten geben zwei verschiedene
/// Schlüssel, und damit hat jede Richtung ihren eigenen Nonce-Raum.
fn richtungsschluessel(
    geheim: &StaticSecret,
    gegenstelle: &Epochenpunkt,
    kem_geheimnis: &[u8; SCHLUESSEL_LEN],
    epoche: EpochId,
    pod: &PodId,
    von: &Endpunkt,
    an: &Endpunkt,
) -> Result<Zeroizing<[u8; SCHLUESSEL_LEN]>, SitzungsFehler> {
    let punkt = PublicKey::from(*gegenstelle.bytes());
    let gemeinsam = geheim.diffie_hellman(&punkt);
    if !gemeinsam.was_contributory() {
        // Kleine Ordnung: Das Ergebnis wäre null, gleich welches
        // Geheimnis links steht. Ein Angreifer könnte so einen
        // Schlüssel erzwingen, den er kennt.
        return Err(SitzungsFehler::PunktOhneBeitrag);
    }

    let mut info = Vec::with_capacity(64);
    info.extend_from_slice(von.bytes());
    info.extend_from_slice(an.bytes());

    // ⚑ **Beide Geheimnisse in einen Eingang, und die Reihenfolge ist
    // Teil des Vertrags.** Der hybride Schutz beruht darauf, dass ein
    // Angreifer **beide** Zweige brechen muss: das klassische
    // Diffie-Hellman gegen einen Quantenrechner und ML-KEM gegen die
    // Gitterannahme. HKDF über die Verkettung leistet genau das, denn
    // das Ergebnis bleibt ununterscheidbar von zufällig, solange auch
    // nur einer der beiden Eingänge es ist.
    //
    // Der KEM-Zweig ist **nicht beidseitig beisteuernd**: Wer kapselt,
    // wählt die Zufälligkeit allein. Das ist unbedenklich, weil der
    // Diffie-Hellman-Zweig es ist und beide in dieselbe Ableitung
    // gehen. Genau darum steht hier ein Hybrid und kein Ersatz.
    let mut eingang = Zeroizing::new([0u8; 2 * SCHLUESSEL_LEN]);
    eingang[..SCHLUESSEL_LEN].copy_from_slice(gemeinsam.as_bytes());
    eingang[SCHLUESSEL_LEN..].copy_from_slice(kem_geheimnis);

    let hk = Hkdf::<Sha256>::new(Some(&salz(epoche, pod)), eingang.as_ref());
    let mut schluessel = Zeroizing::new([0u8; SCHLUESSEL_LEN]);
    hk.expand(&info, schluessel.as_mut())
        .expect("32 Bytes liegen weit unter der HKDF-Grenze");
    Ok(schluessel)
}

/// Der Nonce einer Nachricht: der Sendezähler, sonst nichts.
///
/// Zwölf Bytes, davon vier Null und acht Zähler. Der Zähler allein
/// genügt, weil der Schlüssel schon an Epoche, Pod und Richtung gebunden
/// ist: Zwei Nachrichten mit demselben Schlüssel haben verschiedene
/// Zähler, zwei Nachrichten mit demselben Zähler verschiedene Schlüssel.
fn nonce(zaehler: u64) -> Nonce {
    let mut roh = [0u8; 12];
    roh[4..].copy_from_slice(&zaehler.to_le_bytes());
    *Nonce::from_slice(&roh)
}

/// Ein Kanal zwischen zwei Endpunkten, für eine Epoche und einen Pod.
///
/// Hält zwei Schlüssel (einer je Richtung), den eigenen Sendezähler und
/// den höchsten empfangenen Zähler der Gegenstelle. Zähler und
/// Schlüssel liegen zusammen; siehe den Modulkopf.
pub struct Kanal {
    epoche: EpochId,
    pod: PodId,
    ich: Endpunkt,
    gegenstelle: Endpunkt,
    sende_schluessel: Zeroizing<[u8; SCHLUESSEL_LEN]>,
    /// ⚑ **Erst da, wenn ihre Kapsel angekommen ist.** Der alte Entwurf
    /// leitete beide Richtungen aus zwei veröffentlichten Punkten ab und
    /// brauchte keinen Handschlag. Mit dem KEM-Zweig geht das nicht
    /// mehr: Die Gegenstelle kapselt für ihre Senderichtung, und ohne
    /// ihr Chiffrat gibt es hier nichts abzuleiten.
    empfange_schluessel: Option<Zeroizing<[u8; SCHLUESSEL_LEN]>>,
    /// Der eigene X25519-Zweig, aufbewahrt für den Augenblick, in dem
    /// ihre Kapsel eintrifft.
    eigenes_geheim: StaticSecret,
    gegenpunkte: Gegenpunkte,
    /// Was gesendet werden muss, damit die Gegenstelle lesen kann.
    eigene_kapsel: Kapsel,
    sende_zaehler: u64,
    empfangen_bis: Option<u64>,
}

impl fmt::Debug for Kanal {
    /// Ohne Schlüssel.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Kanal")
            .field("epoche", &self.epoche)
            .field("ich", &self.ich)
            .field("gegenstelle", &self.gegenstelle)
            .field("sende_zaehler", &self.sende_zaehler)
            .field("empfangen_bis", &self.empfangen_bis)
            .finish_non_exhaustive()
    }
}

impl Kanal {
    /// Baut den Kanal aus dem eigenen Epochenschlüssel und dem
    /// angekündigten Punkt der Gegenstelle.
    ///
    /// Kein Handschlag, keine Umlaufzeit: Beide Seiten kennen den Punkt
    /// des anderen aus der Pod-Zuteilung und rechnen denselben
    /// Schlüssel aus. Jeder Shard-Übergang kostet Latenz (Kap. 10.1,
    /// Anforderung 3); eine Umlaufzeit vor der ersten Aktivierung wäre
    /// Latenz, die das Protokoll nicht braucht.
    ///
    /// Was dieser Verzicht kostet, steht bei
    /// [`Sitzungen::rotiere`]: Innerhalb einer Epoche gibt es kein
    /// Vorwärtsgeheimnis, es entsteht erst mit der Rotation.
    pub fn neu(
        eigener: &Epochenschluessel,
        gegenpunkte: &Gegenpunkte,
        pod: PodId,
        ich: Endpunkt,
        gegenstelle: Endpunkt,
    ) -> Result<Self, SitzungsFehler> {
        if ich == gegenstelle {
            return Err(SitzungsFehler::EndpunkteGleich { endpunkt: ich });
        }
        let epoche = eigener.epoche;

        // Für die **eigene Senderichtung** wird gegen ihren Kapselpunkt
        // gekapselt. Das Chiffrat gehört anschließend auf die Leitung;
        // ohne es kann die Gegenstelle nichts lesen.
        let (chiffrat, geheimnis) = gegenpunkte.kapselpunkt.schluessel()?.encapsulate();
        let mut kem_sende = Zeroizing::new([0u8; SCHLUESSEL_LEN]);
        kem_sende.copy_from_slice(&geheimnis[..]);
        let mut roh = Box::new([0u8; KAPSEL_LEN]);
        roh.copy_from_slice(&chiffrat[..]);

        let sende_schluessel = richtungsschluessel(
            &eigener.geheim,
            &gegenpunkte.punkt,
            &kem_sende,
            epoche,
            &pod,
            &ich,
            &gegenstelle,
        )?;
        Ok(Self {
            epoche,
            pod,
            ich,
            gegenstelle,
            sende_schluessel,
            empfange_schluessel: None,
            eigenes_geheim: eigener.geheim.clone(),
            gegenpunkte: gegenpunkte.clone(),
            eigene_kapsel: Kapsel {
                epoche,
                pod,
                von: ich,
                an: gegenstelle,
                chiffrat: roh,
            },
            sende_zaehler: 0,
            empfangen_bis: None,
        })
    }

    /// Die eigene Kapsel, die zur Gegenstelle muss.
    ///
    /// Sie ist öffentlich und braucht keine eigene Signatur: Wer sie
    /// verändert, führt die Gegenstelle auf einen anderen Schlüssel, und
    /// der Tag der ersten Nachricht schlägt fehl. Wer sie mitliest,
    /// gewinnt nichts, denn das Geheimnis steckt nicht darin.
    pub fn eigene_kapsel(&self) -> &Kapsel {
        &self.eigene_kapsel
    }

    /// Ist die Empfangsrichtung schon nutzbar?
    pub fn empfangsbereit(&self) -> bool {
        self.empfange_schluessel.is_some()
    }

    /// Nimmt die Kapsel der Gegenstelle an und schließt damit die
    /// Empfangsrichtung.
    ///
    /// Mehrfach aufgerufen mit derselben Kapsel ist das unschädlich; mit
    /// einer **anderen** Kapsel wird abgelehnt, sobald der Schlüssel
    /// steht. Sonst könnte ein Angreifer den Empfangsschlüssel eines
    /// laufenden Kanals austauschen und damit alles bisher Gelesene
    /// entwerten.
    pub fn nimm_kapsel(
        &mut self,
        eigener: &Epochenschluessel,
        kapsel: &Kapsel,
    ) -> Result<(), SitzungsFehler> {
        if kapsel.epoche != self.epoche
            || kapsel.pod != self.pod
            || kapsel.von != self.gegenstelle
            || kapsel.an != self.ich
        {
            return Err(SitzungsFehler::KapselPasstNicht);
        }
        if self.empfange_schluessel.is_some() {
            return Ok(());
        }
        let feld = Array::try_from(&kapsel.chiffrat[..])
            .map_err(|_| SitzungsFehler::UnleserlicherRahmen)?;
        let roh = eigener.kem_geheim.decapsulate(&feld);
        let mut geheimnis = Zeroizing::new([0u8; SCHLUESSEL_LEN]);
        geheimnis.copy_from_slice(&roh[..]);
        let schluessel = richtungsschluessel(
            &self.eigenes_geheim,
            &self.gegenpunkte.punkt,
            &geheimnis,
            self.epoche,
            &self.pod,
            &self.gegenstelle,
            &self.ich,
        )?;
        self.empfange_schluessel = Some(schluessel);
        Ok(())
    }

    /// Die Epoche dieses Kanals.
    pub fn epoche(&self) -> EpochId {
        self.epoche
    }

    /// Der Stand des Sendezählers.
    pub fn sende_zaehler(&self) -> u64 {
        self.sende_zaehler
    }

    /// Der höchste bisher angenommene Zähler der Gegenstelle.
    pub fn empfangen_bis(&self) -> Option<u64> {
        self.empfangen_bis
    }

    /// Versiegelt einen Klartext.
    pub fn versiegle(&mut self, klartext: &[u8]) -> Result<Versiegelt, SitzungsFehler> {
        if klartext.len() > MAX_KLARTEXT_BYTES {
            return Err(SitzungsFehler::ZuGross {
                bytes: klartext.len(),
                grenze: MAX_KLARTEXT_BYTES,
            });
        }
        // Vor dem Verbrauchen prüfen: Ein Überlauf brächte den Zähler
        // auf null zurück, und das ist genau die Wiederholung, die das
        // Verfahren nicht verträgt.
        if self.sende_zaehler == u64::MAX {
            return Err(SitzungsFehler::ZaehlerErschoepft);
        }

        let kopf = Kopf {
            epoche: self.epoche,
            pod: self.pod,
            von: self.ich,
            an: self.gegenstelle,
            zaehler: self.sende_zaehler,
        };
        let aad = borsh::to_vec(&kopf).expect("Kopf ist borsh-serialisierbar");
        let chiffre = ChaCha20Poly1305::new(Key::from_slice(self.sende_schluessel.as_ref()));
        let geheimtext = chiffre
            .encrypt(
                &nonce(kopf.zaehler),
                Payload {
                    msg: klartext,
                    aad: &aad,
                },
            )
            .map_err(|_| SitzungsFehler::TagStimmtNicht)?;

        self.sende_zaehler += 1;
        Ok(Versiegelt { kopf, geheimtext })
    }

    /// Öffnet eine versiegelte Nachricht.
    ///
    /// Der Empfangsstand wandert **nur bei Erfolg** weiter. Eine
    /// gefälschte Nachricht mit hohem Zähler könnte sonst alle echten
    /// Nachricht darunter aussperren, und das wäre ein Angriff, der
    /// nichts kostet.
    pub fn oeffne(&mut self, nachricht: &Versiegelt) -> Result<Vec<u8>, SitzungsFehler> {
        let kopf = &nachricht.kopf;
        if kopf.epoche != self.epoche {
            return Err(SitzungsFehler::EpocheVorbei {
                kopf: kopf.epoche,
                kanal: self.epoche,
            });
        }
        if kopf.pod != self.pod || kopf.von != self.gegenstelle || kopf.an != self.ich {
            return Err(SitzungsFehler::KopfPasstNicht);
        }
        if let Some(bis) = self.empfangen_bis {
            if kopf.zaehler <= bis {
                return Err(SitzungsFehler::Wiedereinspielung {
                    zaehler: kopf.zaehler,
                    gesehen_bis: bis,
                });
            }
        }

        // ⚑ **Die Bereitschaft wird zuletzt geprüft, und die Reihenfolge
        // ist Absicht.** Eine Nachricht aus der falschen Epoche oder dem
        // falschen Pod ist falsch, gleich ob der Empfangsschlüssel schon
        // steht; wer zuerst auf die Bereitschaft prüft, meldet dafür
        // „Kapsel fehlt" und verdeckt den eigentlichen Grund. Alle
        // Prüfungen davor kommen ohne Schlüssel aus.
        let empfange_schluessel = self
            .empfange_schluessel
            .as_ref()
            .ok_or(SitzungsFehler::EmpfangNochNichtBereit)?;

        let aad = borsh::to_vec(kopf).expect("Kopf ist borsh-serialisierbar");
        let chiffre = ChaCha20Poly1305::new(Key::from_slice(empfange_schluessel.as_ref()));
        let klartext = chiffre
            .decrypt(
                &nonce(kopf.zaehler),
                Payload {
                    msg: &nachricht.geheimtext,
                    aad: &aad,
                },
            )
            .map_err(|_| SitzungsFehler::TagStimmtNicht)?;

        self.empfangen_bis = Some(kopf.zaehler);
        Ok(klartext)
    }
}

/// Alle Kanäle eines Knotens für die laufende Epoche.
///
/// # Warum das ein eigener Wert ist
///
/// Der Epochenschlüssel allein zu vernichten genügt **nicht**. Aus ihm
/// abgeleitete Richtungsschlüssel liegen in jedem [`Kanal`], und ein
/// Kanal, der eine Rotation überlebt, öffnet weiterhin Nachrichten der
/// alten Epoche. Das Vorwärtsgeheimnis wäre dahin, und zwar unsichtbar:
/// Der Epochenschlüssel wäre ordentlich weg, und alles sähe richtig aus.
///
/// Deshalb liegen Schlüssel und Kanäle in einem Wert, und
/// [`Sitzungen::rotiere`] lässt beide zusammen fallen.
pub struct Sitzungen {
    epoche: EpochId,
    eigener: Epochenschluessel,
    ich: Endpunkt,
    kanaele: BTreeMap<(PodId, Endpunkt), Kanal>,
}

impl fmt::Debug for Sitzungen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sitzungen")
            .field("epoche", &self.epoche)
            .field("ich", &self.ich)
            .field("kanaele", &self.kanaele.len())
            .finish_non_exhaustive()
    }
}

impl Sitzungen {
    /// Neu für eine Epoche.
    pub fn neu(ich: Endpunkt, eigener: Epochenschluessel) -> Self {
        Self {
            epoche: eigener.epoche,
            eigener,
            ich,
            kanaele: BTreeMap::new(),
        }
    }

    /// Die laufende Epoche.
    pub fn epoche(&self) -> EpochId {
        self.epoche
    }

    /// Der eigene öffentliche Punkt zum Ankündigen.
    pub fn punkt(&self) -> Epochenpunkt {
        self.eigener.punkt()
    }

    /// Woher das eigene Material stammt.
    pub fn herkunft(&self) -> Herkunft {
        self.eigener.herkunft()
    }

    /// Anzahl offener Kanäle.
    pub fn anzahl(&self) -> usize {
        self.kanaele.len()
    }

    /// Öffnet einen Kanal oder gibt den bestehenden zurück.
    pub fn kanal(
        &mut self,
        pod: PodId,
        gegenstelle: Endpunkt,
        gegenpunkte: &Gegenpunkte,
    ) -> Result<&mut Kanal, SitzungsFehler> {
        let schluessel = (pod, gegenstelle);
        if !self.kanaele.contains_key(&schluessel) {
            let kanal = Kanal::neu(&self.eigener, gegenpunkte, pod, self.ich, gegenstelle)?;
            self.kanaele.insert(schluessel, kanal);
        }
        Ok(self
            .kanaele
            .get_mut(&schluessel)
            .expect("gerade eingefügt oder schon vorhanden"))
    }

    /// Nimmt eine eingegangene Kapsel an und schließt damit die
    /// Empfangsrichtung des betroffenen Kanals.
    ///
    /// **Der Kanal muss schon bestehen.** Wer eine Kapsel bekommt, ohne
    /// selbst einen Kanal geöffnet zu haben, hat noch keinen
    /// Kapselpunkt der Gegenstelle geprüft und dürfte ihr also gar nicht
    /// glauben. Die Reihenfolge ist deshalb Absicht: erst die geprüfte
    /// Ankündigung, dann der Kanal, dann die Kapsel.
    pub fn nimm_kapsel(&mut self, kapsel: &Kapsel) -> Result<(), SitzungsFehler> {
        if kapsel.epoche != self.epoche {
            return Err(SitzungsFehler::KapselPasstNicht);
        }
        let schluessel = (kapsel.pod, kapsel.von);
        let kanal = self
            .kanaele
            .get_mut(&schluessel)
            .ok_or(SitzungsFehler::KapselPasstNicht)?;
        kanal.nimm_kapsel(&self.eigener, kapsel)
    }

    /// Rotiert auf die nächste Epoche (Punkt 3.3).
    ///
    /// # ⚑ Ohne Schonfrist, und das ist eine Entscheidung
    ///
    /// Nachrichten der alten Epoche, die nach der Rotation eintreffen,
    /// sind verloren. Eine Schonfrist von einer Epoche würde sie retten
    /// und **das Vorwärtsgeheimnis um genau diese Epoche verschieben**.
    /// Das Akzeptanzkriterium von Phase 3 sagt „ein Mitschnitt aus
    /// Epoche e ist nach Schlüsselrotation in Epoche e+1 nicht mehr
    /// entschlüsselbar", und eine Schonfrist hielte das nicht ein.
    ///
    /// ⚑ **Die erste Begründung dafür war falsch und steht hier, damit
    /// niemand sie wiederfindet.** Sie lautete: Der Verlust sei
    /// verkraftbar, weil die Pod-Zusammensetzung am selben Grenzpunkt
    /// wechsele, eine Sitzung über die Epochengrenze also ohnehin ihre
    /// Gegenstelle verliere. **Das stimmt nicht.**
    /// `myl_pod::PodBesetzung::epochenwechsel` gibt einen
    /// `RebuildAuftrag` **nur für Positionen aus, deren Miner sich
    /// wirklich ändert**; alle anderen laufen mit ihrem Zwischenstand
    /// weiter. Ein Pod, der zur Hälfte gleich besetzt bleibt, hat über
    /// die Grenze hinweg echte, weiterlaufende Verbindungen.
    ///
    /// **Die tragfähige Begründung ist eine andere:** Der Verlust
    /// betrifft die Nachrichten eines Augenblicks je Stunde, und er ist
    /// **sichtbar**. Wer nach der Rotation eine alte Nachricht bekommt,
    /// erhält [`SitzungsFehler::EpocheVorbei`] und nicht etwa
    /// stillschweigend nichts. Damit gehört die Wiederholung dorthin,
    /// wo die Sequenz geführt wird, nämlich in COMPUTE_PIPELINE, und
    /// nicht in einen aufgeweichten Schlüsselplan.
    ///
    /// **Diese Wiederholung gibt es heute nicht**, und das ist als
    /// offener Punkt vermerkt, nicht als gelöst behandelt.
    pub fn rotiere(
        &mut self,
        neuer: Epochenschluessel,
    ) -> Result<(), SitzungsFehler> {
        if neuer.epoche <= self.epoche {
            return Err(SitzungsFehler::RotationRueckwaerts {
                von: self.epoche,
                nach: neuer.epoche,
            });
        }
        // Beides zusammen: der Epochenschlüssel und jeder daraus
        // abgeleitete Richtungsschlüssel.
        self.kanaele.clear();
        self.epoche = neuer.epoche;
        self.eigener = neuer;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pod(n: u8) -> PodId {
        PodId::new([n; 32])
    }

    fn endpunkt(n: u8) -> Endpunkt {
        Endpunkt::aus_bytes([n; 32])
    }

    /// Zwei Knoten mit festen Saaten, damit ein Fehlschlag reproduzierbar
    /// ist. Beide Kanäle stehen im selben Pod und derselben Epoche.
    /// Die eigenen beiden Punkte, wie sie eine geprüfte Ankündigung
    /// liefern würde. Nur für Tests: Im Betrieb kommt das Paar
    /// ausschließlich aus [`Epochenankuendigung::pruefe`], weil erst die
    /// Signatur es an einen Endpunkt bindet.
    fn punkte(s: &Epochenschluessel) -> Gegenpunkte {
        Gegenpunkte {
            punkt: s.punkt(),
            kapselpunkt: s.kapselpunkt(),
        }
    }

    fn paar(epoche: u64) -> (Kanal, Kanal) {
        let a_schluessel = Epochenschluessel::probe(EpochId(epoche), [1u8; 32]);
        let b_schluessel = Epochenschluessel::probe(EpochId(epoche), [2u8; 32]);
        let mut a = Kanal::neu(
            &a_schluessel,
            &punkte(&b_schluessel),
            pod(7),
            endpunkt(0xaa),
            endpunkt(0xbb),
        )
        .expect("Kanal A");
        let mut b = Kanal::neu(
            &b_schluessel,
            &punkte(&a_schluessel),
            pod(7),
            endpunkt(0xbb),
            endpunkt(0xaa),
        )
        .expect("Kanal B");
        // Der Handschlag: jede Seite gibt ihre Kapsel an die andere.
        let a_kapsel = a.eigene_kapsel().clone();
        let b_kapsel = b.eigene_kapsel().clone();
        b.nimm_kapsel(&b_schluessel, &a_kapsel).expect("Kapsel A→B");
        a.nimm_kapsel(&a_schluessel, &b_kapsel).expect("Kapsel B→A");
        (a, b)
    }

    #[test]
    fn die_kennung_ist_versioniert() {
        // Ohne Version leitet eine Formatänderung stumm andere
        // Schlüssel ab, und zwei Knoten reden aneinander vorbei, ohne
        // dass jemand einen Fehler sieht.
        assert!(SITZUNG_KENNUNG.ends_with("-1"));
    }

    #[test]
    fn der_kopf_ist_so_lang_wie_die_konstante_sagt() {
        // Wer dem Kopf ein Feld gibt und KOPF_BYTES vergisst, bekommt
        // hier einen roten Test statt einer Grenze, die um die Differenz
        // zu großzügig ist.
        let kopf = Kopf {
            epoche: EpochId(1),
            pod: pod(3),
            von: endpunkt(1),
            an: endpunkt(2),
            zaehler: 42,
        };
        assert_eq!(borsh::to_vec(&kopf).unwrap().len(), KOPF_BYTES);
    }

    #[test]
    fn die_groesste_nachricht_passt_durch_den_anfragekanal() {
        // Nicht der Rechenweg wird geprüft, sondern das Ergebnis: eine
        // wirklich maximale Nachricht, wirklich serialisiert.
        let (mut a, _) = paar(1);
        let klartext = vec![0u8; MAX_KLARTEXT_BYTES];
        let versiegelt = a.versiegle(&klartext).expect("versiegeln");
        assert!(
            versiegelt.zu_bytes().len() <= crate::anfrage::MAX_ANFRAGE_BYTES,
            "die größte Nachricht passt nicht durch den Kanal, der sie tragen soll"
        );
    }

    #[test]
    fn ein_zu_grosser_klartext_wird_abgelehnt() {
        let (mut a, _) = paar(1);
        let ergebnis = a.versiegle(&vec![0u8; MAX_KLARTEXT_BYTES + 1]);
        assert!(matches!(ergebnis, Err(SitzungsFehler::ZuGross { .. })));
    }

    #[test]
    fn was_versiegelt_wurde_geht_bei_der_gegenstelle_wieder_auf() {
        let (mut a, mut b) = paar(1);
        let klartext = b"eine Aktivierung".to_vec();
        let versiegelt = a.versiegle(&klartext).expect("versiegeln");
        assert_ne!(versiegelt.geheimtext, klartext, "der Klartext steht im Geheimtext");
        assert_eq!(b.oeffne(&versiegelt).expect("öffnen"), klartext);
    }

    #[test]
    fn der_rahmen_ueberlebt_den_weg_ueber_den_draht() {
        let (mut a, mut b) = paar(1);
        let versiegelt = a.versiegle(b"unterwegs").expect("versiegeln");
        let bytes = versiegelt.zu_bytes();
        let zurueck = Versiegelt::aus_bytes(&bytes).expect("lesen");
        assert_eq!(b.oeffne(&zurueck).expect("öffnen"), b"unterwegs");
    }

    #[test]
    fn kaputte_bytes_geben_einen_fehler_und_keinen_absturz() {
        assert!(matches!(
            Versiegelt::aus_bytes(&[1u8, 2, 3]),
            Err(SitzungsFehler::UnleserlicherRahmen)
        ));
    }

    #[test]
    fn eine_eigene_nachricht_wird_nicht_als_fremde_angenommen() {
        // Dritter Fall von Fund 71: Auch dieser Test hieß nach der
        // Ableitung und prüft den Kopfvergleich, denn der greift vor
        // der Entschlüsselung. Dass die beiden Richtungen wirklich
        // verschiedene Schlüssel bekommen, prüft
        // `das_salz_trennt_epoche_pod_und_richtung`; dass die
        // Zuordnung im Kanal stimmt, prüfen die Hin- und Rückläufe.
        let (mut a, _) = paar(1);
        let versiegelt = a.versiegle(b"nur fuer B").expect("versiegeln");
        assert!(matches!(
            a.oeffne(&versiegelt),
            Err(SitzungsFehler::KopfPasstNicht)
        ));
    }

    #[test]
    fn derselbe_endpunkt_auf_beiden_seiten_wird_abgelehnt() {
        let schluessel = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
        let ergebnis = Kanal::neu(
            &schluessel,
            &punkte(&schluessel),
            pod(1),
            endpunkt(5),
            endpunkt(5),
        );
        assert!(matches!(
            ergebnis,
            Err(SitzungsFehler::EndpunkteGleich { .. })
        ));
    }

    #[test]
    fn ein_punkt_ohne_beitrag_wird_abgelehnt() {
        // Ein Punkt kleiner Ordnung erzwingt ein gemeinsames Geheimnis
        // aus lauter Nullen, und das kennt der Angreifer.
        let schluessel = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
        let ergebnis = Kanal::neu(
            &schluessel,
            &Gegenpunkte {
                punkt: Epochenpunkt::aus_bytes([0u8; 32]),
                kapselpunkt: schluessel.kapselpunkt(),
            },
            pod(1),
            endpunkt(1),
            endpunkt(2),
        );
        assert!(matches!(ergebnis, Err(SitzungsFehler::PunktOhneBeitrag)));
    }

    /// ⚑ Gegenproben zum hybriden Austausch (Post-Quantum).
    mod hybrid {
        use super::*;

        /// Die Grundaussage: Nach dem Kapseltausch lesen beide Seiten.
        #[test]
        fn der_handschlag_macht_beide_richtungen_lesbar() {
            let (mut a, mut b) = paar(1);
            let hin = a.versiegle(b"von A nach B").expect("versiegeln");
            assert_eq!(b.oeffne(&hin).expect("öffnen"), b"von A nach B");
            let zurueck = b.versiegle(b"von B nach A").expect("versiegeln");
            assert_eq!(a.oeffne(&zurueck).expect("öffnen"), b"von B nach A");
        }

        /// ⚑ **Ohne Kapsel ist nichts zu lesen, und das ist der
        /// sichtbarste Unterschied zum alten Entwurf.** Vorher genügten
        /// zwei veröffentlichte Punkte für beide Richtungen.
        #[test]
        fn ohne_kapsel_ist_nichts_zu_lesen() {
            let a_s = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
            let b_s = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
            let mut a = Kanal::neu(&a_s, &punkte(&b_s), pod(7), endpunkt(0xaa), endpunkt(0xbb))
                .expect("Kanal A");
            let mut b = Kanal::neu(&b_s, &punkte(&a_s), pod(7), endpunkt(0xbb), endpunkt(0xaa))
                .expect("Kanal B");
            assert!(!b.empfangsbereit());
            let hin = a.versiegle(b"zu frueh").expect("versiegeln");
            assert!(matches!(
                b.oeffne(&hin),
                Err(SitzungsFehler::EmpfangNochNichtBereit)
            ));
            // Und mit der Kapsel geht dieselbe Nachricht auf.
            let a_kapsel = a.eigene_kapsel().clone();
            b.nimm_kapsel(&b_s, &a_kapsel).expect("Kapsel");
            assert!(b.empfangsbereit());
            assert_eq!(b.oeffne(&hin).expect("öffnen"), b"zu frueh");
        }

        /// Eine Kapsel aus einem anderen Kanal wird abgelehnt, statt
        /// still einen falschen Schlüssel zu setzen.
        #[test]
        fn eine_fremde_kapsel_wird_abgelehnt() {
            let a_s = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
            let b_s = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
            let mut b = Kanal::neu(&b_s, &punkte(&a_s), pod(7), endpunkt(0xbb), endpunkt(0xaa))
                .expect("Kanal B");
            let a = Kanal::neu(&a_s, &punkte(&b_s), pod(7), endpunkt(0xaa), endpunkt(0xbb))
                .expect("Kanal A");
            let echt = a.eigene_kapsel().clone();

            for (feld, kaputt) in [
                ("Epoche", Kapsel { epoche: EpochId(2), ..echt.clone() }),
                ("Pod", Kapsel { pod: pod(8), ..echt.clone() }),
                ("Absender", Kapsel { von: endpunkt(0xcc), ..echt.clone() }),
                ("Empfänger", Kapsel { an: endpunkt(0xcc), ..echt.clone() }),
            ] {
                assert!(
                    matches!(
                        b.nimm_kapsel(&b_s, &kaputt),
                        Err(SitzungsFehler::KapselPasstNicht)
                    ),
                    "falsches Feld {} wurde angenommen",
                    feld
                );
            }
            // Gegenprobe: die echte geht durch.
            b.nimm_kapsel(&b_s, &echt).expect("die echte Kapsel");
        }

        /// ⚑ Ein verfälschtes Chiffrat bricht nicht laut, sondern führt
        /// auf einen anderen Schlüssel. Der Fehlschlag kommt am Tag, und
        /// das ist die richtige Stelle: ML-KEM lehnt ungültige Chiffrate
        /// nicht sichtbar ab, sondern liefert einen pseudozufälligen
        /// Wert (implizite Zurückweisung).
        #[test]
        fn eine_verfaelschte_kapsel_fuehrt_auf_einen_anderen_schluessel() {
            let a_s = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
            let b_s = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
            let mut a = Kanal::neu(&a_s, &punkte(&b_s), pod(7), endpunkt(0xaa), endpunkt(0xbb))
                .expect("Kanal A");
            let mut b = Kanal::neu(&b_s, &punkte(&a_s), pod(7), endpunkt(0xbb), endpunkt(0xaa))
                .expect("Kanal B");
            let mut kapsel = a.eigene_kapsel().clone();
            kapsel.chiffrat[0] ^= 0xff;
            b.nimm_kapsel(&b_s, &kapsel)
                .expect("wird angenommen, denn die Fälschung ist nicht erkennbar");
            let hin = a.versiegle(b"Inhalt").expect("versiegeln");
            assert!(matches!(b.oeffne(&hin), Err(SitzungsFehler::TagStimmtNicht)));
        }

        /// ⚑ **Der Kapselpunkt hängt an der Signatur.** Ohne diese
        /// Bindung könnte ein Angreifer den Post-Quantum-Zweig durch
        /// einen eigenen Schlüssel ersetzen und ihn damit abschalten,
        /// ohne die Signatur zu brechen. Der Test tauscht genau das aus.
        #[test]
        fn ein_getauschter_kapselpunkt_bricht_die_signatur() {
            let sk = konsens(1);
            let mein_endpunkt = endpunkt_aus_schluessel(&sk.public_key().unwrap());
            let echt = Epochenschluessel::probe(EpochId(9), [1u8; 32]);
            let fremd = Epochenschluessel::probe(EpochId(9), [7u8; 32]);
            let ankuendigung = Epochenankuendigung::neu(&sk, &echt).expect("ankündigen");
            assert!(ankuendigung.pruefe(mein_endpunkt, EpochId(9)).is_ok());

            let getauscht = Epochenankuendigung {
                kapselpunkt: fremd.kapselpunkt(),
                ..ankuendigung.clone()
            };
            assert!(matches!(
                getauscht.pruefe(mein_endpunkt, EpochId(9)),
                Err(SitzungsFehler::SignaturStimmtNicht)
            ));
        }

        /// ⚑ Der KEM-Zweig wirkt wirklich auf den Schlüssel: gleiche
        /// Diffie-Hellman-Seite, anderes KEM-Geheimnis, anderer
        /// Schlüssel. Ohne diesen Test könnte der zweite Eingang
        /// versehentlich verworfen werden, und alles bliebe grün.
        #[test]
        fn der_kem_zweig_geht_in_den_schluessel_ein() {
            let a = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
            let punkt = Epochenschluessel::probe(EpochId(1), [2u8; 32]).punkt();
            let mit = |kem: &[u8; SCHLUESSEL_LEN]| {
                richtungsschluessel(
                    &a.geheim,
                    &punkt,
                    kem,
                    EpochId(1),
                    &pod(7),
                    &endpunkt(0xaa),
                    &endpunkt(0xbb),
                )
                .expect("ableiten")
                .to_vec()
            };
            assert_ne!(mit(&[0u8; SCHLUESSEL_LEN]), mit(&[1u8; SCHLUESSEL_LEN]));
            assert_eq!(mit(&[5u8; SCHLUESSEL_LEN]), mit(&[5u8; SCHLUESSEL_LEN]));
        }

        /// Die Saat bestimmt auch den KEM-Schlüssel, sonst wäre ein
        /// Probelauf nicht reproduzierbar.
        #[test]
        fn dieselbe_saat_ergibt_denselben_kapselpunkt() {
            let a = Epochenschluessel::probe(EpochId(1), [42u8; 32]);
            let b = Epochenschluessel::probe(EpochId(1), [42u8; 32]);
            assert_eq!(a.kapselpunkt(), b.kapselpunkt());
            let c = Epochenschluessel::probe(EpochId(1), [43u8; 32]);
            assert_ne!(a.kapselpunkt(), c.kapselpunkt());
        }

        /// Die Kapsel überlebt die Leitung.
        #[test]
        fn die_kapsel_ueberlebt_die_serialisierung() {
            let a_s = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
            let b_s = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
            let a = Kanal::neu(&a_s, &punkte(&b_s), pod(7), endpunkt(0xaa), endpunkt(0xbb))
                .expect("Kanal A");
            let roh = a.eigene_kapsel().zu_bytes();
            assert_eq!(roh.len(), 8 + 32 + 32 + 32 + KAPSEL_LEN);
            let zurueck = Kapsel::aus_bytes(&roh).expect("lesen");
            assert_eq!(&zurueck, a.eigene_kapsel());
            // Ein Byte zu wenig wird abgelehnt, statt still zu raten.
            assert!(matches!(
                Kapsel::aus_bytes(&roh[..roh.len() - 1]),
                Err(SitzungsFehler::UnleserlicherRahmen)
            ));
        }
    }

    #[test]
    fn ein_dritter_kann_nicht_mitlesen() {
        // Das ist das Akzeptanzkriterium von Phase 3: Ein Beobachter
        // bekommt aus dem Geheimtext weder Prompt noch Aktivierung.
        let (mut a, _) = paar(1);
        let versiegelt = a.versiegle(b"Prompt und Aktivierung").expect("versiegeln");
        let a_kapsel = a.eigene_kapsel().clone();

        let c_schluessel = Epochenschluessel::probe(EpochId(1), [9u8; 32]);
        let a_schluessel = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
        let mut c = Kanal::neu(
            &c_schluessel,
            &punkte(&a_schluessel),
            pod(7),
            endpunkt(0xbb),
            endpunkt(0xaa),
        )
        .expect("Kanal C");

        // ⚑ **C bekommt sogar die Kapsel**, die A für B erzeugt hat, und
        // das ist der Punkt dieses Tests. Ohne sie scheiterte C schon an
        // `EmpfangNochNichtBereit`, und der Test bewiese nur, dass ein
        // Lauscher nichts hat. Mit ihr beweist er, dass ihm auch das
        // Mitgehörte nichts nützt: A hat gegen **Bs** Kapselpunkt
        // gekapselt, C entkapselt mit **seinem** Schlüssel und bekommt
        // ein anderes Geheimnis. ML-KEM lehnt dabei nicht sichtbar ab,
        // sondern liefert einen pseudozufälligen Wert; der Fehlschlag
        // kommt erst am Tag, und genau so soll es sein.
        c.nimm_kapsel(&c_schluessel, &a_kapsel)
            .expect("die Kapsel ist an C adressiert und wird angenommen");
        assert!(c.empfangsbereit(), "C hält sich für empfangsbereit");
        assert!(matches!(
            c.oeffne(&versiegelt),
            Err(SitzungsFehler::TagStimmtNicht)
        ));
    }

    #[test]
    fn ein_veraenderter_kopf_faellt_auf() {
        // Der Fall des böswilligen Gateways: Es leitet weiter, schreibt
        // aber den Empfänger um.
        let (mut a, mut b) = paar(1);
        let mut versiegelt = a.versiegle(b"fuer B").expect("versiegeln");
        versiegelt.kopf.an = endpunkt(0xcc);
        assert!(matches!(
            b.oeffne(&versiegelt),
            Err(SitzungsFehler::KopfPasstNicht)
        ));
    }

    #[test]
    fn ein_umgeschriebener_zaehler_faellt_auf() {
        // Der Zähler steht im Kopf und ist zugleich der Nonce. Wer ihn
        // ändert, ändert beides, und das Tag stimmt nicht mehr.
        let (mut a, mut b) = paar(1);
        let mut versiegelt = a.versiegle(b"fuer B").expect("versiegeln");
        versiegelt.kopf.zaehler = 99;
        assert!(matches!(
            b.oeffne(&versiegelt),
            Err(SitzungsFehler::TagStimmtNicht)
        ));
    }

    #[test]
    fn ein_verkipptes_byte_im_geheimtext_faellt_auf() {
        let (mut a, mut b) = paar(1);
        let mut versiegelt = a.versiegle(b"unversehrt").expect("versiegeln");
        versiegelt.geheimtext[0] ^= 1;
        assert!(matches!(
            b.oeffne(&versiegelt),
            Err(SitzungsFehler::TagStimmtNicht)
        ));
    }

    #[test]
    fn eine_wiedereinspielung_wird_abgelehnt() {
        let (mut a, mut b) = paar(1);
        let versiegelt = a.versiegle(b"einmal").expect("versiegeln");
        assert!(b.oeffne(&versiegelt).is_ok());
        assert!(matches!(
            b.oeffne(&versiegelt),
            Err(SitzungsFehler::Wiedereinspielung { .. })
        ));
    }

    #[test]
    fn eine_luecke_im_zaehler_sperrt_nicht_aus() {
        // Ein verlorenes Paket darf den Kanal nicht schließen: verlangt
        // wird größer, nicht genau der nächste.
        let (mut a, mut b) = paar(1);
        let _verloren = a.versiegle(b"geht unterwegs verloren").expect("versiegeln");
        let angekommen = a.versiegle(b"kommt an").expect("versiegeln");
        assert_eq!(b.oeffne(&angekommen).expect("öffnen"), b"kommt an");
    }

    #[test]
    fn eine_faelschung_mit_hohem_zaehler_verschiebt_den_stand_nicht() {
        // Sonst genügte eine erfundene Nachricht mit Zähler u64::MAX,
        // um jede echte Nachricht danach auszusperren.
        let (mut a, mut b) = paar(1);
        let echt = a.versiegle(b"echt").expect("versiegeln");
        let mut faelschung = echt.clone();
        faelschung.kopf.zaehler = u64::MAX;
        assert!(b.oeffne(&faelschung).is_err());
        assert_eq!(b.empfangen_bis(), None, "die Fälschung hat den Stand verschoben");
        assert_eq!(b.oeffne(&echt).expect("öffnen"), b"echt");
    }

    #[test]
    fn der_sendezaehler_waechst_je_nachricht() {
        let (mut a, _) = paar(1);
        assert_eq!(a.sende_zaehler(), 0);
        a.versiegle(b"eins").expect("versiegeln");
        a.versiegle(b"zwei").expect("versiegeln");
        assert_eq!(a.sende_zaehler(), 2);
    }

    #[test]
    fn ein_erschoepfter_zaehler_versiegelt_nicht_mehr() {
        // Lieber ein Fehler als ein Überlauf auf null: Der wäre die
        // Nonce-Wiederholung, die das Verfahren nicht verträgt.
        let (mut a, _) = paar(1);
        a.sende_zaehler = u64::MAX;
        assert!(matches!(
            a.versiegle(b"eine zu viel"),
            Err(SitzungsFehler::ZaehlerErschoepft)
        ));
    }

    #[test]
    fn eine_nachricht_aus_einem_anderen_pod_wird_abgewiesen() {
        // Der Kopfvergleich, nicht die Ableitung: Die prüft
        // `das_salz_trennt_epoche_pod_und_richtung`.
        let a_schluessel = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
        let b_schluessel = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
        let mut a = Kanal::neu(
            &a_schluessel,
            &punkte(&b_schluessel),
            pod(7),
            endpunkt(0xaa),
            endpunkt(0xbb),
        )
        .expect("Kanal A");
        let mut b_anderer_pod = Kanal::neu(
            &b_schluessel,
            &punkte(&a_schluessel),
            pod(8),
            endpunkt(0xbb),
            endpunkt(0xaa),
        )
        .expect("Kanal B");
        let versiegelt = a.versiegle(b"nur in Pod 7").expect("versiegeln");
        assert!(matches!(
            b_anderer_pod.oeffne(&versiegelt),
            Err(SitzungsFehler::KopfPasstNicht)
        ));
    }

    #[test]
    fn eine_nachricht_aus_einer_anderen_epoche_wird_abgewiesen() {
        // Auch hier der Kopfvergleich. Dass der Schlüssel selbst ein
        // anderer ist, prüft `das_salz_trennt_epoche_pod_und_richtung`.
        let a_schluessel = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
        let b_alt = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
        let b_neu = Epochenschluessel::probe(EpochId(2), [2u8; 32]);
        let mut a = Kanal::neu(
            &a_schluessel,
            &punkte(&b_alt),
            pod(7),
            endpunkt(0xaa),
            endpunkt(0xbb),
        )
        .expect("Kanal A");
        let a_neu = Epochenschluessel::probe(EpochId(2), [1u8; 32]);
        let mut b = Kanal::neu(
            &b_neu,
            &punkte(&a_neu),
            pod(7),
            endpunkt(0xbb),
            endpunkt(0xaa),
        )
        .expect("Kanal B");
        let versiegelt = a.versiegle(b"aus Epoche 1").expect("versiegeln");
        assert!(matches!(
            b.oeffne(&versiegelt),
            Err(SitzungsFehler::EpocheVorbei { .. })
        ));
    }

    /// Ein Konsensschlüssel aus fester Saat, damit ein Fehlschlag
    /// reproduzierbar ist.
    fn konsens(n: u8) -> BlsSecretKey {
        BlsSecretKey::key_gen(&[n; 32]).expect("Schlüsselerzeugung")
    }

    #[test]
    fn der_endpunkt_ist_der_hash_des_konsensschluessels() {
        // Das ist der Angelpunkt der ganzen Ankündigung: Wer den
        // Schlüssel hat, hat den Endpunkt, und niemand sonst. Ein
        // Gleichstandstest in NODE hält diese Ableitung mit der von
        // `MinerId` zusammen.
        let pk = konsens(1).public_key().expect("Schlüssel");
        let erwartet = {
            let mut h = Sha256::new();
            h.update(pk.0);
            let roh: [u8; 32] = h.finalize().into();
            Endpunkt::aus_bytes(roh)
        };
        assert_eq!(endpunkt_aus_schluessel(&pk), erwartet);
        assert_ne!(
            endpunkt_aus_schluessel(&konsens(2).public_key().unwrap()),
            erwartet
        );
    }

    #[test]
    fn eine_ankuendigung_gibt_den_punkt_erst_nach_der_pruefung_heraus() {
        let sk = konsens(1);
        let mein_endpunkt = endpunkt_aus_schluessel(&sk.public_key().unwrap());
        let schluessel = Epochenschluessel::probe(EpochId(9), [1u8; 32]);
        let ankuendigung = Epochenankuendigung::neu(&sk, &schluessel).expect("ankündigen");
        assert_eq!(ankuendigung.epoche(), EpochId(9));
        assert_eq!(ankuendigung.behaupteter_endpunkt(), mein_endpunkt);
        assert_eq!(
            ankuendigung.pruefe(mein_endpunkt, EpochId(9)).expect("prüfen"),
            Gegenpunkte {
                punkt: schluessel.punkt(),
                kapselpunkt: schluessel.kapselpunkt(),
            }
        );
    }

    #[test]
    fn die_pruefung_braucht_nichts_ausser_dem_erwarteten_endpunkt() {
        // ⚑ Der Grund für den Wechsel vom Netz- auf den
        // Konsensschlüssel. Vorher musste der Aufrufer den öffentlichen
        // Netzschlüssel der Gegenstelle beisteuern, und die Zuordnung
        // von PeerId zu MinerId gibt es im Protokoll nirgends. Jetzt
        // genügt, was ohnehin im Pod-Pfad steht.
        let sk = konsens(1);
        let aus_dem_pod_pfad = endpunkt_aus_schluessel(&sk.public_key().unwrap());
        let ankuendigung =
            Epochenankuendigung::neu(&sk, &Epochenschluessel::probe(EpochId(9), [1u8; 32]))
                .expect("ankündigen");
        assert!(ankuendigung.pruefe(aus_dem_pod_pfad, EpochId(9)).is_ok());
    }

    #[test]
    fn eine_ankuendigung_fuer_eine_andere_gegenstelle_wird_abgewiesen() {
        // Der eigentliche Angriff: Ein echter Teilnehmer kündigt einen
        // echten, richtig unterschriebenen Punkt an, aber es ist nicht
        // der, mit dem gesprochen werden soll.
        let angreifer = konsens(66);
        let ankuendigung = Epochenankuendigung::neu(
            &angreifer,
            &Epochenschluessel::probe(EpochId(9), [66u8; 32]),
        )
        .expect("ankündigen");
        let opfer = endpunkt_aus_schluessel(&konsens(1).public_key().unwrap());
        assert!(matches!(
            ankuendigung.pruefe(opfer, EpochId(9)),
            Err(SitzungsFehler::EndpunktPasstNicht { .. })
        ));
    }

    #[test]
    fn ein_untergeschobener_punkt_faellt_auf() {
        // Der Mann in der Mitte: Er ersetzt den angekündigten Punkt
        // durch seinen eigenen und führte beide Seiten sonst in eine
        // Sitzung mit ihm.
        let sk = konsens(1);
        let mein_endpunkt = endpunkt_aus_schluessel(&sk.public_key().unwrap());
        let mut ankuendigung =
            Epochenankuendigung::neu(&sk, &Epochenschluessel::probe(EpochId(9), [1u8; 32]))
                .expect("ankündigen");
        ankuendigung.punkt = Epochenschluessel::probe(EpochId(9), [66u8; 32]).punkt();
        assert!(matches!(
            ankuendigung.pruefe(mein_endpunkt, EpochId(9)),
            Err(SitzungsFehler::SignaturStimmtNicht)
        ));
    }

    #[test]
    fn ein_ausgetauschter_schluessel_faellt_auf() {
        // Wer den Schlüssel tauscht, tauscht den Endpunkt mit, denn der
        // eine ist der Hash des anderen. Genau das macht die Zuordnung
        // fälschungssicher, ohne dass jemand ein Register führen muss.
        let sk = konsens(1);
        let mein_endpunkt = endpunkt_aus_schluessel(&sk.public_key().unwrap());
        let mut ankuendigung =
            Epochenankuendigung::neu(&sk, &Epochenschluessel::probe(EpochId(9), [1u8; 32]))
                .expect("ankündigen");
        ankuendigung.pubkey = konsens(66).public_key().unwrap();
        assert!(matches!(
            ankuendigung.pruefe(mein_endpunkt, EpochId(9)),
            Err(SitzungsFehler::EndpunktPasstNicht { .. })
        ));
    }

    #[test]
    fn ein_ungueltiger_konsensschluessel_wird_abgewiesen() {
        // Vor dem Hashen geprüft: Ein Schlüssel, der kein gültiger
        // Gruppenpunkt ist, hat trotzdem einen Hash, und der könnte
        // passen.
        let kaputt = BlsPublicKey([0u8; BLS_PK_LEN]);
        let s = Epochenschluessel::probe(EpochId(9), [1u8; 32]);
        let ankuendigung = Epochenankuendigung {
            epoche: EpochId(9),
            punkt: s.punkt(),
            kapselpunkt: s.kapselpunkt(),
            pubkey: kaputt,
            signatur: BlsSignature([0u8; 96]),
        };
        assert!(matches!(
            ankuendigung.pruefe(endpunkt_aus_schluessel(&kaputt), EpochId(9)),
            Err(SitzungsFehler::SchluesselUngueltig)
        ));
    }

    #[test]
    fn eine_ankuendigung_aus_der_alten_epoche_wird_abgewiesen() {
        // Sie ist echt unterschrieben und trotzdem falsch: Ohne diese
        // Prüfung wäre sie der Weg, die Rotation zurückzudrehen.
        let sk = konsens(1);
        let mein_endpunkt = endpunkt_aus_schluessel(&sk.public_key().unwrap());
        let ankuendigung =
            Epochenankuendigung::neu(&sk, &Epochenschluessel::probe(EpochId(9), [1u8; 32]))
                .expect("ankündigen");
        assert!(matches!(
            ankuendigung.pruefe(mein_endpunkt, EpochId(10)),
            Err(SitzungsFehler::AnkuendigungFuerAndereEpoche { .. })
        ));
    }

    #[test]
    fn eine_umdatierte_ankuendigung_faellt_auf() {
        // Der Feldvergleich allein genügt nicht: Wäre die Epoche nicht
        // mitunterschrieben, könnte ein Angreifer eine echte
        // Ankündigung aus Epoche 9 auf 10 umdatieren, und der Vergleich
        // wäre zufrieden. Der Punkt aus der alten Epoche gälte dann in
        // der neuen, und das Vorwärtsgeheimnis wäre dahin.
        let sk = konsens(1);
        let mein_endpunkt = endpunkt_aus_schluessel(&sk.public_key().unwrap());
        let mut ankuendigung =
            Epochenankuendigung::neu(&sk, &Epochenschluessel::probe(EpochId(9), [1u8; 32]))
                .expect("ankündigen");
        ankuendigung.epoche = EpochId(10);
        assert!(matches!(
            ankuendigung.pruefe(mein_endpunkt, EpochId(10)),
            Err(SitzungsFehler::SignaturStimmtNicht)
        ));
    }

    #[test]
    fn die_signatur_gilt_nur_mit_der_eigenen_kennung() {
        // Derselbe BLS-Schlüssel unterschreibt Stimmen, Bündel und
        // Ankündigungen. Ohne Trennzeichenkette könnte eine
        // Unterschrift aus einem anderen Zusammenhang hier durchgehen.
        let sk = konsens(1);
        let pubkey = sk.public_key().unwrap();
        let mein_endpunkt = endpunkt_aus_schluessel(&pubkey);
        let schluessel = Epochenschluessel::probe(EpochId(9), [1u8; 32]);
        let punkt = schluessel.punkt();

        let mut ohne_kennung = Vec::new();
        ohne_kennung.extend_from_slice(&EpochId(9).0.to_le_bytes());
        ohne_kennung.extend_from_slice(&pubkey.0);
        ohne_kennung.extend_from_slice(punkt.bytes());

        let gefaelscht = Epochenankuendigung {
            epoche: EpochId(9),
            punkt,
            kapselpunkt: schluessel.kapselpunkt(),
            pubkey,
            signatur: sk.sign(&ohne_kennung).expect("signieren"),
        };
        assert!(matches!(
            gefaelscht.pruefe(mein_endpunkt, EpochId(9)),
            Err(SitzungsFehler::SignaturStimmtNicht)
        ));
        // Aufbau festgenagelt, wie `DST_POI_BUNDLE` es in CONSENSUS
        // vormacht: Wer ein Feld einschiebt, bekommt hier einen roten
        // Test statt einer stillen Formatänderung.
        let kapselpunkt = schluessel.kapselpunkt();
        let msg = ankuendigungsbytes(EpochId(9), &pubkey, &punkt, &kapselpunkt);
        assert!(msg.starts_with(DST_EPOCHENPUNKT));
        assert_eq!(
            msg.len(),
            DST_EPOCHENPUNKT.len() + 8 + BLS_PK_LEN + 32 + KAPSELPUNKT_LEN
        );
        // ⚑ Der Kapselpunkt steht **hinten und mit drin**: Ohne ihn wäre
        // der zweite Zweig des Austauschs ungedeckt, und ein Angreifer
        // könnte ihn austauschen, ohne die Signatur zu brechen.
        assert_eq!(&msg[msg.len() - KAPSELPUNKT_LEN..], &kapselpunkt.bytes()[..]);
        assert_eq!(
            &msg[msg.len() - KAPSELPUNKT_LEN - 32..msg.len() - KAPSELPUNKT_LEN],
            punkt.bytes()
        );
    }

    #[test]
    fn die_ankuendigung_ueberlebt_borsh() {
        let sk = konsens(1);
        let mein_endpunkt = endpunkt_aus_schluessel(&sk.public_key().unwrap());
        let schluessel = Epochenschluessel::probe(EpochId(9), [1u8; 32]);
        let ankuendigung = Epochenankuendigung::neu(&sk, &schluessel).expect("ankündigen");
        let roh = borsh::to_vec(&ankuendigung).expect("serialisieren");
        let zurueck: Epochenankuendigung = borsh::from_slice(&roh).expect("lesen");
        assert_eq!(zurueck, ankuendigung);
        assert_eq!(
            zurueck.pruefe(mein_endpunkt, EpochId(9)).expect("prüfen"),
            Gegenpunkte {
                punkt: schluessel.punkt(),
                kapselpunkt: schluessel.kapselpunkt(),
            }
        );
    }

    #[test]
    fn das_salz_trennt_epoche_pod_und_richtung() {
        // ⚑ Fund 71. Zwei Tests hießen vorher „gibt einen anderen
        // Schlüssel" und prüften in Wahrheit den Kopfvergleich: Sie
        // wären auch dann grün geblieben, wenn Epoche und Pod gar nicht
        // ins Salz eingegangen wären. Grün, aber nicht aus dem Grund,
        // der im Namen stand.
        //
        // Der Unterschied ist nicht akademisch. Fehlt die Epoche im
        // Salz, bleibt derselbe Schlüssel über die Rotation hinaus
        // gültig, und ein Mitschnitt aus Epoche e ist in e+1 weiter zu
        // öffnen, sobald jemand den Kopf umschreibt. Genau das schließt
        // das Akzeptanzkriterium von Phase 3 aus.
        //
        // Hier wird deshalb die Ableitung selbst verglichen.
        let a = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
        let punkt = Epochenschluessel::probe(EpochId(1), [2u8; 32]).punkt();
        // Ein fester KEM-Zweig, damit dieser Test die **Trennung nach
        // Epoche, Pod und Richtung** prüft und nicht die Zufälligkeit
        // der Kapselung.
        let kem = [9u8; SCHLUESSEL_LEN];
        let ableiten = |epoche, pod_nr, von, an| {
            richtungsschluessel(
                &a.geheim,
                &punkt,
                &kem,
                EpochId(epoche),
                &pod(pod_nr),
                &endpunkt(von),
                &endpunkt(an),
            )
            .expect("ableiten")
        };

        let grund = ableiten(1, 7, 1, 2);
        assert_ne!(*grund, *ableiten(2, 7, 1, 2), "die Epoche steht nicht im Salz");
        assert_ne!(*grund, *ableiten(1, 8, 1, 2), "der Pod steht nicht im Salz");
        assert_ne!(*grund, *ableiten(1, 7, 2, 1), "die Richtung steht nicht im info");
    }

    #[test]
    fn der_kopf_haengt_kryptografisch_am_geheimtext() {
        // Der Empfänger vergleicht den Kopf ohnehin Feld für Feld, und
        // ein umgeschriebener Kopf fällt schon dort auf. Diese Prüfung
        // ist die zweite Schicht, und sie ist nicht überflüssig: Sie
        // hält auch dann, wenn ein späterer Aufrufer den Kanal anhand
        // des Kopfes *heraussucht*, statt ihn zu vergleichen. Ohne AAD
        // wäre die Bindung eine Verabredung zwischen zwei Codestellen,
        // mit AAD ist sie gerechnet.
        let (mut a, _) = paar(1);
        let versiegelt = a.versiegle(b"Inhalt").expect("versiegeln");
        let chiffre = ChaCha20Poly1305::new(Key::from_slice(a.sende_schluessel.as_ref()));
        let nonce_der_nachricht = nonce(versiegelt.kopf.zaehler);

        let mut umgeschrieben = versiegelt.kopf;
        umgeschrieben.pod = pod(9);
        let falsche_aad = borsh::to_vec(&umgeschrieben).expect("Kopf");
        assert!(
            chiffre
                .decrypt(
                    &nonce_der_nachricht,
                    Payload {
                        msg: &versiegelt.geheimtext,
                        aad: &falsche_aad,
                    },
                )
                .is_err(),
            "der Kopf steht nicht in den authentisierten Daten"
        );

        let richtige_aad = borsh::to_vec(&versiegelt.kopf).expect("Kopf");
        assert!(chiffre
            .decrypt(
                &nonce_der_nachricht,
                Payload {
                    msg: &versiegelt.geheimtext,
                    aad: &richtige_aad,
                },
            )
            .is_ok());
    }

    #[test]
    fn beide_seiten_leiten_denselben_schluessel_ab() {
        // Ohne diese Gleichheit gäbe es keinen Kanal, sondern zwei
        // Knoten, die aneinander vorbei verschlüsseln.
        let a = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
        let b = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
        // ⚑ **Seit dem hybriden Austausch genügt der eigene
        // Diffie-Hellman-Zweig dafür nicht mehr.** Der KEM-Zweig kommt
        // aus der Kapsel der Gegenstelle; er ist hier fest gesetzt,
        // damit der Test genau die Aussage prüft, die er im Namen führt:
        // dass der **Diffie-Hellman-Teil** von beiden Seiten dasselbe
        // ergibt. Dass der KEM-Teil zusammenpasst, prüft
        // `der_handschlag_macht_beide_richtungen_lesbar`.
        let kem = [5u8; SCHLUESSEL_LEN];
        let bei_a = richtungsschluessel(
            &a.geheim,
            &b.punkt(),
            &kem,
            EpochId(1),
            &pod(7),
            &endpunkt(0xaa),
            &endpunkt(0xbb),
        )
        .expect("ableiten");
        let bei_b = richtungsschluessel(
            &b.geheim,
            &a.punkt(),
            &kem,
            EpochId(1),
            &pod(7),
            &endpunkt(0xaa),
            &endpunkt(0xbb),
        )
        .expect("ableiten");
        assert_eq!(*bei_a, *bei_b);
    }

    #[test]
    fn nach_der_rotation_geht_die_alte_nachricht_nicht_mehr_auf() {
        // Das zweite Akzeptanzkriterium von Phase 3: ein Mitschnitt aus
        // Epoche e ist in e+1 nicht mehr zu öffnen.
        let a_schluessel = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
        let b_schluessel = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
        let a_punkt = punkte(&a_schluessel);
        let mut a = Kanal::neu(
            &a_schluessel,
            &punkte(&b_schluessel),
            pod(7),
            endpunkt(0xaa),
            endpunkt(0xbb),
        )
        .expect("Kanal A");
        let mitschnitt = a.versiegle(b"Inhalt aus Epoche 1").expect("versiegeln");
        let a_kapsel = a.eigene_kapsel().clone();

        let mut b = Sitzungen::neu(endpunkt(0xbb), b_schluessel);
        b.kanal(pod(7), endpunkt(0xaa), &a_punkt).expect("Kanal");
        b.nimm_kapsel(&a_kapsel).expect("Kapsel");
        assert!(b
            .kanal(pod(7), endpunkt(0xaa), &a_punkt)
            .expect("Kanal")
            .oeffne(&mitschnitt)
            .is_ok());

        b.rotiere(Epochenschluessel::probe(EpochId(2), [3u8; 32]))
            .expect("rotieren");

        // Nach der Rotation gibt es keinen Weg mehr zum alten Schlüssel:
        // Der Kanal wird neu gebaut, mit dem neuen Epochenschlüssel.
        let ergebnis = b
            .kanal(pod(7), endpunkt(0xaa), &a_punkt)
            .expect("Kanal")
            .oeffne(&mitschnitt);
        assert!(matches!(ergebnis, Err(SitzungsFehler::EpocheVorbei { .. })));
    }

    #[test]
    fn die_rotation_raeumt_jeden_kanal_weg() {
        // Den Epochenschlüssel allein zu ersetzen genügte nicht: Die
        // abgeleiteten Richtungsschlüssel liegen in den Kanälen.
        let b_schluessel = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
        let fremd = punkte(&Epochenschluessel::probe(EpochId(1), [1u8; 32]));
        let mut b = Sitzungen::neu(endpunkt(0xbb), b_schluessel);
        b.kanal(pod(7), endpunkt(0xaa), &fremd).expect("Kanal");
        b.kanal(pod(8), endpunkt(0xcc), &fremd).expect("Kanal");
        assert_eq!(b.anzahl(), 2);

        b.rotiere(Epochenschluessel::probe(EpochId(2), [3u8; 32]))
            .expect("rotieren");
        assert_eq!(b.anzahl(), 0, "ein Kanal hat die Rotation überlebt");
        assert_eq!(b.epoche(), EpochId(2));
    }

    #[test]
    fn dieselbe_gegenstelle_bekommt_denselben_kanal() {
        // Sonst begänne der Sendezähler bei jedem Aufruf wieder bei
        // null, und der zweite Aufruf wiederholte den ersten Nonce.
        let b_schluessel = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
        let fremd = punkte(&Epochenschluessel::probe(EpochId(1), [1u8; 32]));
        let mut b = Sitzungen::neu(endpunkt(0xbb), b_schluessel);
        b.kanal(pod(7), endpunkt(0xaa), &fremd)
            .expect("Kanal")
            .versiegle(b"eins")
            .expect("versiegeln");
        let stand = b
            .kanal(pod(7), endpunkt(0xaa), &fremd)
            .expect("Kanal")
            .sende_zaehler();
        assert_eq!(stand, 1, "der Kanal wurde neu gebaut statt weitergeführt");
        assert_eq!(b.anzahl(), 1);
    }

    #[test]
    fn die_rotation_geht_nicht_rueckwaerts() {
        let mut b = Sitzungen::neu(
            endpunkt(0xbb),
            Epochenschluessel::probe(EpochId(5), [2u8; 32]),
        );
        for epoche in [4u64, 5] {
            assert!(matches!(
                b.rotiere(Epochenschluessel::probe(EpochId(epoche), [3u8; 32])),
                Err(SitzungsFehler::RotationRueckwaerts { .. })
            ));
        }
        assert_eq!(b.epoche(), EpochId(5));
    }

    #[test]
    fn die_herkunft_des_materials_steht_am_schluessel() {
        // Kein Vorgabewert, kein Schalter: zwei verschiedene Aufrufe.
        assert_eq!(
            Epochenschluessel::probe(EpochId(1), [1u8; 32]).herkunft(),
            Herkunft::Probelauf
        );
        assert_eq!(
            Epochenschluessel::ziehe(EpochId(1)).herkunft(),
            Herkunft::Gezogen
        );
    }

    #[test]
    fn zwei_gezogene_schluessel_sind_verschieden() {
        let a = Epochenschluessel::ziehe(EpochId(1));
        let b = Epochenschluessel::ziehe(EpochId(1));
        assert_ne!(a.punkt(), b.punkt(), "der Zufallsgenerator liefert Festwerte");
    }

    #[test]
    fn das_debug_zeigt_kein_schluesselmaterial() {
        // Ein Debug, das ein Geheimnis in eine Protokollzeile schreibt,
        // ist ein Leck mit Zeilennummer.
        let schluessel = Epochenschluessel::probe(EpochId(1), [7u8; 32]);
        let bytes = schluessel.geheim.to_bytes();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let text = format!("{schluessel:?}");
        assert!(!text.contains(&hex), "das Geheimnis steht im Debug");
        assert!(text.contains("probelauf") || text.contains("Probelauf"));

        let (a, _) = paar(1);
        let kanal_text = format!("{a:?}");
        let sende_hex: String = a
            .sende_schluessel
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert!(
            !kanal_text.contains(&sende_hex),
            "der Sitzungsschlüssel steht im Debug"
        );
    }

    #[test]
    fn jeder_fehler_sagt_was_geschehen_ist() {
        // Eine Fehlermeldung ohne Inhalt kostet beim Suchen mehr als sie
        // beim Schreiben spart.
        let faelle = [
            SitzungsFehler::EndpunkteGleich {
                endpunkt: endpunkt(1),
            },
            SitzungsFehler::PunktOhneBeitrag,
            SitzungsFehler::ZaehlerErschoepft,
            SitzungsFehler::ZuGross {
                bytes: 5,
                grenze: 4,
            },
            SitzungsFehler::KopfPasstNicht,
            SitzungsFehler::EpocheVorbei {
                kopf: EpochId(1),
                kanal: EpochId(2),
            },
            SitzungsFehler::Wiedereinspielung {
                zaehler: 3,
                gesehen_bis: 7,
            },
            SitzungsFehler::TagStimmtNicht,
            SitzungsFehler::UnleserlicherRahmen,
            SitzungsFehler::RotationRueckwaerts {
                von: EpochId(2),
                nach: EpochId(1),
            },
            SitzungsFehler::SignaturStimmtNicht,
            SitzungsFehler::AnkuendigungFuerAndereEpoche {
                ankuendigung: EpochId(9),
                erwartet: EpochId(10),
            },
            SitzungsFehler::EndpunktPasstNicht {
                erwartet: endpunkt(1),
                bekommen: endpunkt(2),
            },
            SitzungsFehler::SchluesselUngueltig,
            SitzungsFehler::SignierenGescheitert,
        ];
        for fall in faelle {
            let text = fall.to_string();
            assert!(text.len() > 20, "zu knapp: {text}");
            assert!(!text.ends_with(' '));
        }
    }
}
