//! Der Validator-Satz zu Genesis: wer stimmen darf, mit welchem Gewicht.
//!
//! # Warum eine Datei und nicht das Netz
//!
//! Entschieden am 2026-08-25 (Projektinhaber). Die Alternative wäre
//! gewesen, den Satz zur Laufzeit über Gossip zu entdecken, und das ist
//! der Sybil-Eingang: **Wer sich selbst ankündigen darf, kündigt sich
//! fünfzehnmal an.** Genau dieser Fehler steckte bis v0.3.6 in
//! `myl_consensus::bft` (Fund A3), dort behoben durch eine
//! stimmberechtigte Menge, die von außen kommt. Diese Datei ist das
//! „außen".
//!
//! Die dritte Möglichkeit, den Satz aus dem Ledger-Zustand über eine
//! Genesis-Transaktion zu beziehen, ist nicht verworfen, sondern
//! zurückgestellt: Sie ist der richtige Weg für Ein- und Austritt im
//! Dauerbetrieb, blockiert aber den ersten Netzlauf, bis sie steht.
//!
//! # Der Hash liegt auf dem Inhalt, nicht auf der Datei
//!
//! [`Genesis::hash`] rechnet über die **kanonische Borsh-Kodierung des
//! gelesenen Inhalts**, nach Kennung sortiert, nicht über die Bytes der
//! Datei. Zwei Dateien, die dieselben Validatoren in anderer Reihenfolge
//! oder mit anderen Kommentaren führen, haben denselben Hash.
//!
//! Das ist dieselbe Unterscheidung, die Kap. 6.2 für die Ausführung
//! trifft und der STORAGE-Entwurf für gespeicherte Gegenstände: **Der
//! Inhalt ist verbindlich, die Kodierung nicht.** Läge der Hash auf den
//! Dateibytes, wäre eine umsortierte Zeile ein Konsensbruch, und ein
//! Texteditor, der Zeilenenden ändert, spaltete das Netz.
//!
//! # Die Kennung wird abgeleitet, nicht aufgeschrieben
//!
//! Eine Datei, die Kennung **und** Schlüssel führt, hat zwei Quellen für
//! dieselbe Wahrheit, und irgendwann widersprechen sie sich. Die Kennung
//! ist `sha256(pubkey)`, wie in [`crate::validatorsatz::probe_kennung`],
//! nur hier mit einem Schlüssel, den niemand ableiten kann.
//!
//! # ⚑ Was diese Datei prüft, bevor der Knoten startet
//!
//! 1. **Besitznachweis je Schlüssel** (Fund 27). Ohne ihn kann ein
//!    Angreifer einen Schlüssel eintragen, dessen geheimen Teil er nicht
//!    kennt, aber so gewählt, dass er die Aggregatsignatur der anderen
//!    zu seinen Gunsten verschiebt.
//! 2. **Kein Validator hält ein Drittel oder mehr.** Sonst beschreibt
//!    die Datei ein Netz, das seine eigene Sicherheitsannahme verletzt,
//!    und niemand merkt es beim Lesen. Das ist ein **Startfehler**, kein
//!    Hinweis: Ein solches Netz hat keine BFT-Safety, es hat einen
//!    Diktator mit Vetorecht.
//!
//!    ⚑ **Diese Schranke bestimmt zugleich eine Mindestzahl.** Drei
//!    Werte, die alle unter einem Drittel ihrer Summe liegen, können
//!    diese Summe nicht ergeben. Ein Genesis-Satz braucht also
//!    **mindestens vier** Validatoren, unabhängig von der Verteilung.
//!    Aufgefallen beim Nachrechnen eines fehlgeschlagenen Tests, der
//!    drei Validatoren prüfte und erwartete, dass einer durchgeht.
//!    Festgehalten in
//!    `unter_vier_validatoren_ist_die_schranke_nicht_erfuellbar`.
//! 3. **Keine doppelten Schlüssel**, kein Validator ohne Stake.
//!
//! Nicht geprüft, sondern nur **berichtet**, wird
//! [`Genesis::unterscheidet_koepfe_von_gewicht`]. Ein Produktivnetz mit
//! Hunderten Validatoren erfüllt das nebenbei; ein Testnetz mit gleichen
//! Gewichten nicht, und dort ist es ein Mangel. Ein Ladefehler wäre es
//! aber nicht: Eine gültige Stake-Verteilung darf nicht daran scheitern,
//! dass sie zufällig gleichmäßig ist.

use std::collections::BTreeSet;

use borsh::{BorshDeserialize, BorshSerialize};
use myl_consensus::validator::{VotingMember, VotingSet};
use myl_types::bls::{BlsProofOfPossession, BlsPublicKey, BLS_PK_LEN, BLS_SIG_LEN};
use myl_types::hash::Hash;
use myl_types::ids::MinerId;

/// Ein Validator, wie die Genesis-Datei ihn führt.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct GenesisValidator {
    /// Öffentlicher BLS-Schlüssel. Der einzige Bezeichner in der Datei.
    pub pubkey: BlsPublicKey,
    /// Besitznachweis über eben diesen Schlüssel (Fund 27).
    pub pop: BlsProofOfPossession,
    /// Stake in MYL-Kleinstbeträgen (1 MYL = 10^6).
    pub stake: u64,
}

impl GenesisValidator {
    /// Die Kennung: `sha256(pubkey)`.
    ///
    /// Abgeleitet und nicht aufgeschrieben, siehe Modulkopf.
    pub fn kennung(&self) -> MinerId {
        let h = Hash::sha256(&self.pubkey.0);
        let mut roh = [0u8; 32];
        roh.copy_from_slice(h.as_bytes());
        MinerId::new(roh)
    }
}

/// Der Genesis-Zustand, soweit ein Knoten ihn zum Mitstimmen braucht.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Genesis {
    /// Netzname. Trennt Probenetze voneinander und vom Hauptnetz.
    ///
    /// Geht in den Hash ein: Zwei Netze mit gleichem Validator-Satz und
    /// verschiedenem Namen sind verschiedene Netze.
    pub netz: String,
    /// Die Validatoren, **nach Kennung sortiert**. Die Sortierung stellt
    /// [`Genesis::aus_text`] her, damit die Reihenfolge in der Datei
    /// keinen Einfluss auf den Hash hat.
    pub validatoren: Vec<GenesisValidator>,
}

/// Was beim Lesen einer Genesis-Datei schiefgehen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenesisFehler {
    /// Eine Zeile ist keine bekannte Anweisung.
    UnbekannteZeile { zeile: usize, inhalt: String },
    /// Eine Anweisung hat die falsche Anzahl Felder.
    FalscheFeldzahl {
        zeile: usize,
        erwartet: usize,
        bekommen: usize,
    },
    /// Ein Hexfeld ist keine gültige Hexzahl der erwarteten Länge.
    KeinHex {
        zeile: usize,
        feld: &'static str,
        erwartete_zeichen: usize,
    },
    /// Der Stake ist keine Zahl oder null.
    UnbrauchbarerStake { zeile: usize },
    /// Kein Netzname angegeben.
    OhneNetznamen,
    /// Keine Validatoren angegeben.
    OhneValidatoren,
    /// Derselbe Schlüssel steht zweimal in der Datei.
    DoppelterSchluessel { kennung: MinerId },
    /// Der Besitznachweis passt nicht zum Schlüssel (Fund 27).
    BesitzNichtNachgewiesen { kennung: MinerId },
    /// Ein Validator hält ein Drittel des Gesamtstakes oder mehr.
    ///
    /// Ein solches Netz hat keine BFT-Safety. Es startet nicht.
    ZuVielGewicht {
        kennung: MinerId,
        stake: u64,
        gesamt: u64,
    },
}

impl std::fmt::Display for GenesisFehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnbekannteZeile { zeile, inhalt } => {
                write!(f, "Zeile {zeile}: unbekannte Anweisung: {inhalt:?}")
            }
            Self::FalscheFeldzahl {
                zeile,
                erwartet,
                bekommen,
            } => write!(
                f,
                "Zeile {zeile}: {erwartet} Felder erwartet, {bekommen} bekommen"
            ),
            Self::KeinHex {
                zeile,
                feld,
                erwartete_zeichen,
            } => write!(
                f,
                "Zeile {zeile}: Feld {feld} ist keine Hexzahl aus {erwartete_zeichen} Zeichen"
            ),
            Self::UnbrauchbarerStake { zeile } => {
                write!(f, "Zeile {zeile}: Stake ist keine Zahl größer als null")
            }
            Self::OhneNetznamen => write!(f, "Die Datei nennt keinen Netznamen"),
            Self::OhneValidatoren => write!(f, "Die Datei nennt keinen Validator"),
            Self::DoppelterSchluessel { kennung } => {
                write!(f, "Schlüssel {kennung:?} steht zweimal in der Datei")
            }
            Self::BesitzNichtNachgewiesen { kennung } => write!(
                f,
                "Besitznachweis von {kennung:?} passt nicht zum Schlüssel (Fund 27)"
            ),
            Self::ZuVielGewicht {
                kennung,
                stake,
                gesamt,
            } => write!(
                f,
                "{kennung:?} hält {stake} von {gesamt} und damit ein Drittel oder mehr. \
                 Ein Netz, in dem ein Einzelner das Quorum blockieren kann, hat keine \
                 BFT-Safety und startet nicht"
            ),
        }
    }
}

impl std::error::Error for GenesisFehler {}

/// Liest ein Hexfeld fester Länge.
fn hex_feld<const N: usize>(
    text: &str,
    zeile: usize,
    feld: &'static str,
) -> Result<[u8; N], GenesisFehler> {
    let fehler = || GenesisFehler::KeinHex {
        zeile,
        feld,
        erwartete_zeichen: N * 2,
    };
    if text.len() != N * 2 {
        return Err(fehler());
    }
    let mut roh = [0u8; N];
    for (i, byte) in roh.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(|_| fehler())?;
    }
    Ok(roh)
}

fn als_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

impl Genesis {
    /// Liest eine Genesis-Datei.
    ///
    /// **Format**, eine Anweisung je Zeile, Felder durch Leerraum
    /// getrennt, `#` leitet einen Kommentar ein:
    ///
    /// ```text
    /// netz       <name>
    /// validator  <pubkey-hex>  <pop-hex>  <stake>
    /// ```
    ///
    /// Die Reihenfolge der `validator`-Zeilen ist ohne Bedeutung: Sie
    /// werden nach Kennung sortiert, bevor irgendetwas daraus folgt.
    ///
    /// Prüft anschließend [`Genesis::pruefe`].
    pub fn aus_text(text: &str) -> Result<Self, GenesisFehler> {
        let mut netz: Option<String> = None;
        let mut validatoren = Vec::new();

        for (nr, rohzeile) in text.lines().enumerate() {
            let zeile = nr + 1;
            let ohne_kommentar = rohzeile.split('#').next().unwrap_or("");
            let felder: Vec<&str> = ohne_kommentar.split_whitespace().collect();
            match felder.as_slice() {
                [] => {}
                ["netz", name] => netz = Some((*name).to_string()),
                ["netz", ..] => {
                    return Err(GenesisFehler::FalscheFeldzahl {
                        zeile,
                        erwartet: 2,
                        bekommen: felder.len(),
                    })
                }
                ["validator", pk, pop, stake] => {
                    let pubkey =
                        BlsPublicKey(hex_feld::<BLS_PK_LEN>(pk, zeile, "pubkey")?);
                    let pop =
                        BlsProofOfPossession(hex_feld::<BLS_SIG_LEN>(pop, zeile, "pop")?);
                    let stake: u64 = stake
                        .parse()
                        .map_err(|_| GenesisFehler::UnbrauchbarerStake { zeile })?;
                    if stake == 0 {
                        return Err(GenesisFehler::UnbrauchbarerStake { zeile });
                    }
                    validatoren.push(GenesisValidator { pubkey, pop, stake });
                }
                ["validator", ..] => {
                    return Err(GenesisFehler::FalscheFeldzahl {
                        zeile,
                        erwartet: 4,
                        bekommen: felder.len(),
                    })
                }
                _ => {
                    return Err(GenesisFehler::UnbekannteZeile {
                        zeile,
                        inhalt: ohne_kommentar.trim().to_string(),
                    })
                }
            }
        }

        let netz = netz.ok_or(GenesisFehler::OhneNetznamen)?;
        if validatoren.is_empty() {
            return Err(GenesisFehler::OhneValidatoren);
        }
        // Kanonische Reihenfolge: Der Hash darf nicht davon abhängen,
        // in welcher Zeile jemand seinen Validator eingetragen hat.
        validatoren.sort_by_key(|v| v.kennung());

        let g = Self { netz, validatoren };
        g.pruefe()?;
        Ok(g)
    }

    /// Schreibt die Datei zurück, in kanonischer Reihenfolge.
    pub fn als_text(&self) -> String {
        let mut s = String::new();
        s.push_str("# Myelith-Genesis. Die Kennung wird aus dem Schlüssel abgeleitet.\n");
        s.push_str(&format!("netz {}\n", self.netz));
        for v in &self.validatoren {
            s.push_str(&format!(
                "validator {} {} {}\n",
                als_hex(&v.pubkey.0),
                als_hex(&v.pop.0),
                v.stake
            ));
        }
        s
    }

    /// Der Genesis-Hash: SHA-256 über die kanonische Borsh-Kodierung.
    ///
    /// **Nicht über die Dateibytes**, siehe Modulkopf. Zwei Knoten mit
    /// gleichem Inhalt und verschieden formatierter Datei rechnen
    /// denselben Hash.
    pub fn hash(&self) -> Hash {
        let bytes = borsh::to_vec(self).expect("Genesis ist aus festen Feldern serialisierbar");
        Hash::sha256(&bytes)
    }

    /// Summe aller Stakes.
    pub fn gesamtstake(&self) -> u64 {
        self.validatoren.iter().map(|v| v.stake as u128).sum::<u128>() as u64
    }

    /// Die stimmberechtigte Menge für die erste Runde.
    ///
    /// **Gewicht gleich Stake, und das ist keine Vereinfachung.**
    /// `myl_consensus::voting_weight::calculate_voting_weight` rechnet
    /// `stake + stake · abgeklungene_Arbeit / arbeitsbezug`, gedeckelt.
    /// Zu Genesis ist die Arbeitshistorie leer, der Bonus also null, und
    /// der Deckel greift auf `stake · hoechstfaktor` und damit nicht.
    /// Es bleibt exakt der Stake.
    pub fn stimmberechtigte(&self) -> VotingSet {
        let mitglieder = self
            .validatoren
            .iter()
            .map(|v| {
                (
                    v.kennung(),
                    VotingMember {
                        pubkey: v.pubkey,
                        weight: v.stake,
                    },
                )
            })
            .collect();
        VotingSet::from_members(mitglieder)
    }

    /// Die Kennungen in kanonischer Reihenfolge.
    ///
    /// Diese Reihenfolge ist die Producer-Liste für
    /// `myl_consensus::select_leader`: Sie hängt an den Schlüsseln, also
    /// rechnet jeder Knoten dieselbe.
    pub fn kennungen(&self) -> Vec<MinerId> {
        self.validatoren.iter().map(|v| v.kennung()).collect()
    }

    /// Prüft, was vor dem Start feststehen muss.
    ///
    /// Siehe Modulkopf, Punkte 1 bis 3.
    pub fn pruefe(&self) -> Result<(), GenesisFehler> {
        let mut gesehen = BTreeSet::new();
        for v in &self.validatoren {
            let kennung = v.kennung();
            if !gesehen.insert(kennung) {
                return Err(GenesisFehler::DoppelterSchluessel { kennung });
            }
            // Fund 27: Ohne Besitznachweis ließe sich ein Schlüssel
            // eintragen, dessen geheimen Teil niemand kennt, aber so
            // gewählt, dass er die Aggregatsignatur verschiebt.
            if !v.pubkey.verify_possession(&v.pop) {
                return Err(GenesisFehler::BesitzNichtNachgewiesen { kennung });
            }
        }

        let gesamt = self.gesamtstake();
        for v in &self.validatoren {
            // „hält ein Drittel oder mehr" heißt 3 · stake >= gesamt.
            if (v.stake as u128) * 3 >= gesamt as u128 {
                return Err(GenesisFehler::ZuVielGewicht {
                    kennung: v.kennung(),
                    stake: v.stake,
                    gesamt,
                });
            }
        }
        Ok(())
    }

    /// **Invariante 2: Unterscheidet diese Verteilung Köpfe von Gewicht?**
    ///
    /// Gibt zwei gleich große Teilmengen zurück, von denen die eine das
    /// Quorum erreicht und die andere nicht, oder `None`, wenn es keine
    /// solchen gibt.
    ///
    /// # Warum das gebraucht wird
    ///
    /// Bei **gleichen** Gewichten sind Kopfzählung und Gewichtszählung
    /// numerisch dasselbe. Genau die Fehlerklasse, die `myl_consensus`
    /// als Fund A3 schon einmal hatte (der Zustandsautomat zählte
    /// Nachrichten statt Gewicht), liefe dann grün durch. Eine
    /// Verteilung, für die diese Funktion ein Paar liefert, kann den
    /// Unterschied überhaupt zeigen.
    ///
    /// # ⚑ Warum das kein Ladefehler ist
    ///
    /// Ein Produktivnetz mit Hunderten Validatoren erfüllt es nebenbei.
    /// Eine gültige Stake-Verteilung darf nicht daran scheitern, dass
    /// sie zufällig gleichmäßig ist. Der Knoten **berichtet** das
    /// Ergebnis beim Start, und der Test über die mitgelieferte
    /// Probenetz-Datei verlangt es.
    ///
    /// # Grenze
    ///
    /// Sucht erschöpfend und deshalb nur bis
    /// [`Self::MAX_TEILMENGENSUCHE`] Validatoren. Darüber gibt sie
    /// `None` zurück, **ohne** dass das eine Aussage wäre: Bei so vielen
    /// Validatoren ist die Eigenschaft praktisch immer erfüllt, und
    /// 2^n Teilmengen sind nicht mehr aufzählbar.
    pub fn unterscheidet_koepfe_von_gewicht(&self) -> Option<(Vec<MinerId>, Vec<MinerId>)> {
        let n = self.validatoren.len();
        if n == 0 || n > Self::MAX_TEILMENGENSUCHE {
            return None;
        }
        let schwelle = self.stimmberechtigte().quorum_threshold();
        // Je Kopfzahl das erste erreichende und das erste verfehlende
        // Muster merken; sobald beide für dieselbe Kopfzahl da sind,
        // ist das Paar gefunden.
        let mut erreicht: Vec<Option<u32>> = vec![None; n + 1];
        let mut verfehlt: Vec<Option<u32>> = vec![None; n + 1];
        for muster in 0u32..(1u32 << n) {
            let koepfe = muster.count_ones() as usize;
            let gewicht: u128 = (0..n)
                .filter(|i| muster & (1 << i) != 0)
                .map(|i| self.validatoren[i].stake as u128)
                .sum();
            let ziel = if gewicht >= schwelle as u128 {
                &mut erreicht[koepfe]
            } else {
                &mut verfehlt[koepfe]
            };
            if ziel.is_none() {
                *ziel = Some(muster);
            }
            if let (Some(a), Some(b)) = (erreicht[koepfe], verfehlt[koepfe]) {
                return Some((self.zu_kennungen(a), self.zu_kennungen(b)));
            }
        }
        None
    }

    /// Obergrenze der erschöpfenden Teilmengensuche.
    ///
    /// 2^20 sind rund eine Million Muster, also Millisekunden. Darüber
    /// wächst es exponentiell, und der Nutzen sinkt: Die Eigenschaft ist
    /// bei vielen Validatoren praktisch immer erfüllt.
    pub const MAX_TEILMENGENSUCHE: usize = 20;

    fn zu_kennungen(&self, muster: u32) -> Vec<MinerId> {
        (0..self.validatoren.len())
            .filter(|i| muster & (1 << i) != 0)
            .map(|i| self.validatoren[i].kennung())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_types::bls::BlsSecretKey;

    /// Ein Schlüsselpaar samt Besitznachweis, aus einer festen Saat.
    fn schluessel(byte: u8) -> (BlsPublicKey, BlsProofOfPossession) {
        let sk = BlsSecretKey::key_gen(&[byte.wrapping_add(1); 32]).expect("key_gen");
        (
            sk.public_key().expect("pubkey"),
            sk.prove_possession().expect("pop"),
        )
    }

    /// Die Verteilung des Probenetzes: fünf Validatoren, 900 MYL.
    ///
    /// Sie ist **konstruiert, nicht gegriffen** — siehe den Test
    /// [`die_verteilung_legt_drei_grenzfaelle_aus`].
    fn probenetz_text() -> String {
        let stakes = [
            250_000_000u64,
            230_000_000,
            200_000_000,
            120_000_000,
            100_000_000,
        ];
        let mut s = String::from("# Probenetz\nnetz myelith-probenetz-1\n");
        for (i, stake) in stakes.iter().enumerate() {
            let (pk, pop) = schluessel(i as u8);
            s.push_str(&format!(
                "validator {} {} {}\n",
                als_hex(&pk.0),
                als_hex(&pop.0),
                stake
            ));
        }
        s
    }

    fn probenetz() -> Genesis {
        Genesis::aus_text(&probenetz_text()).expect("Probenetz muss lesbar sein")
    }

    #[test]
    fn das_probenetz_laesst_sich_lesen() {
        let g = probenetz();
        assert_eq!(g.netz, "myelith-probenetz-1");
        assert_eq!(g.validatoren.len(), 5);
        assert_eq!(g.gesamtstake(), 900_000_000);
    }

    #[test]
    fn die_kennung_haengt_am_schluessel() {
        // Zwei Quellen für dieselbe Wahrheit widersprechen sich
        // irgendwann. Deshalb steht die Kennung nicht in der Datei.
        let g = probenetz();
        for v in &g.validatoren {
            assert_eq!(
                v.kennung().as_bytes(),
                Hash::sha256(&v.pubkey.0).as_bytes()
            );
        }
    }

    #[test]
    fn die_reihenfolge_in_der_datei_aendert_den_hash_nicht() {
        // Der Hash liegt auf dem Inhalt, nicht auf der Datei.
        let vorwaerts = probenetz();
        let text = probenetz_text();
        let mut zeilen: Vec<&str> = text.lines().collect();
        let kopf: Vec<&str> = zeilen.drain(..2).collect();
        zeilen.reverse();
        let rueckwaerts_text = format!("{}\n{}\n", kopf.join("\n"), zeilen.join("\n"));
        let rueckwaerts = Genesis::aus_text(&rueckwaerts_text).expect("lesbar");
        assert_eq!(vorwaerts.hash(), rueckwaerts.hash());
        assert_eq!(vorwaerts, rueckwaerts);
    }

    #[test]
    fn kommentare_und_leerraum_aendern_den_hash_nicht() {
        let ohne = probenetz();
        let mit = Genesis::aus_text(&format!(
            "# oben\n\n   \n{}\n# unten\n",
            probenetz_text().replace('\n', "   # Anmerkung\n")
        ))
        .expect("lesbar");
        assert_eq!(ohne.hash(), mit.hash());
    }

    #[test]
    fn ein_anderer_netzname_ist_ein_anderes_netz() {
        let a = probenetz();
        let b = Genesis::aus_text(&probenetz_text().replace("probenetz-1", "probenetz-2"))
            .expect("lesbar");
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn ein_anderer_stake_ist_ein_anderer_hash() {
        let a = probenetz();
        let b = Genesis::aus_text(&probenetz_text().replace("100000000", "110000000"))
            .expect("lesbar");
        assert_ne!(a.hash(), b.hash());
    }

    // ── Invariante 1: kein Drittel ──────────────────────────────────

    #[test]
    fn invariante_1_kein_validator_haelt_ein_drittel() {
        let g = probenetz();
        let gesamt = g.gesamtstake();
        for v in &g.validatoren {
            assert!(
                (v.stake as u128) * 3 < gesamt as u128,
                "{} von {} ist ein Drittel oder mehr",
                v.stake,
                gesamt
            );
        }
    }

    #[test]
    fn ein_netz_mit_einem_drittelhalter_startet_nicht() {
        // Die Gegenprobe zur Invariante. Ein solches Netz hat keine
        // BFT-Safety: Der Halter blockiert jedes Quorum allein.
        let (pk_a, pop_a) = schluessel(10);
        let (pk_b, pop_b) = schluessel(11);
        let (pk_c, pop_c) = schluessel(12);
        let text = format!(
            "netz kaputt\n\
             validator {} {} 300\n\
             validator {} {} 300\n\
             validator {} {} 300\n",
            als_hex(&pk_a.0),
            als_hex(&pop_a.0),
            als_hex(&pk_b.0),
            als_hex(&pop_b.0),
            als_hex(&pk_c.0),
            als_hex(&pop_c.0),
        );
        match Genesis::aus_text(&text) {
            Err(GenesisFehler::ZuVielGewicht { stake, gesamt, .. }) => {
                assert_eq!((stake, gesamt), (300, 900));
            }
            andere => panic!("erwartet ZuVielGewicht, bekommen {andere:?}"),
        }
    }

    /// Baut eine Genesis aus Stakes, mit frischen Schlüsseln je Eintrag.
    fn genesis_mit(netz: &str, saat: u8, stakes: &[u64]) -> Result<Genesis, GenesisFehler> {
        let mut text = format!("netz {netz}\n");
        for (i, stake) in stakes.iter().enumerate() {
            let (pk, pop) = schluessel(saat.wrapping_add(i as u8));
            text.push_str(&format!(
                "validator {} {} {}\n",
                als_hex(&pk.0),
                als_hex(&pop.0),
                stake
            ));
        }
        Genesis::aus_text(&text)
    }

    #[test]
    fn die_grenze_liegt_auf_der_richtigen_seite() {
        // 299 · 3 = 897 < 900: weniger als ein Drittel, also zulässig.
        assert!(genesis_mit("knapp-drunter", 10, &[299, 299, 151, 151]).is_ok());
        // 300 · 3 = 900 >= 900: genau ein Drittel, also nicht mehr.
        // Genau ein Drittel reicht schon, um jedes Quorum zu blockieren.
        assert!(matches!(
            genesis_mit("knapp-drueber", 20, &[300, 299, 150, 151]),
            Err(GenesisFehler::ZuVielGewicht { stake: 300, .. })
        ));
    }

    /// ⚑ **Unter vier Validatoren ist die Schranke gar nicht erfüllbar.**
    ///
    /// Drei Werte, die alle **unter** einem Drittel der Summe liegen,
    /// können nicht die Summe ergeben. Die Schranke ist damit
    /// gleichbedeutend mit „mindestens vier Validatoren", und das
    /// unabhängig von der Verteilung.
    ///
    /// Aufgefallen beim Nachrechnen eines fehlgeschlagenen Tests: Der
    /// prüfte drei Validatoren und erwartete, dass einer davon durchgeht.
    #[test]
    fn unter_vier_validatoren_ist_die_schranke_nicht_erfuellbar() {
        for stakes in [
            vec![300u64, 300, 300],
            vec![500, 300, 100],
            vec![298, 301, 301],
            vec![1, 1, 1],
            vec![450, 450],
            vec![900],
        ] {
            assert!(
                matches!(
                    genesis_mit("zu-wenige", 30, &stakes),
                    Err(GenesisFehler::ZuVielGewicht { .. })
                ),
                "{stakes:?} kam durch, obwohl drei Werte unter je einem \
                 Drittel ihre eigene Summe nicht erreichen können"
            );
        }
        // Und die Gegenrichtung: Ab vier geht es.
        assert!(genesis_mit("gerade-genug", 40, &[250, 250, 250, 250]).is_ok());
    }

    // ── Invariante 2: Köpfe gegen Gewicht ───────────────────────────

    #[test]
    fn invariante_2_die_verteilung_unterscheidet_koepfe_von_gewicht() {
        let g = probenetz();
        let (erreicht, verfehlt) = g
            .unterscheidet_koepfe_von_gewicht()
            .expect("die Verteilung ist genau dafür gebaut");
        assert_eq!(
            erreicht.len(),
            verfehlt.len(),
            "die beiden Teilmengen müssen gleich viele Köpfe haben, \
             sonst zeigen sie den Unterschied nicht"
        );
    }

    #[test]
    fn bei_gleichen_gewichten_gibt_es_kein_solches_paar() {
        // Die Gegenprobe. Genau deshalb wurden gleiche Gewichte für den
        // ersten Netzlauf verworfen: Kopfzählung und Gewichtszählung
        // wären numerisch dasselbe, und Fund A3 liefe grün durch.
        let mut text = String::from("netz gleich\n");
        for i in 0..5u8 {
            let (pk, pop) = schluessel(20 + i);
            text.push_str(&format!(
                "validator {} {} 100\n",
                als_hex(&pk.0),
                als_hex(&pop.0)
            ));
        }
        let g = Genesis::aus_text(&text).expect("lesbar");
        assert!(
            g.unterscheidet_koepfe_von_gewicht().is_none(),
            "bei gleichen Gewichten darf es kein unterscheidendes Paar geben"
        );
    }

    /// **Die drei Grenzfälle, für die diese Verteilung gebaut wurde.**
    ///
    /// Alle drei Teilmengen haben **drei von fünf Köpfen** und drei
    /// verschiedene Urteile. Ohne diesen Test ist die Verteilung eine
    /// Zahlenreihe, die irgendwann jemand glättet.
    #[test]
    fn die_verteilung_legt_drei_grenzfaelle_aus() {
        let g = probenetz();
        let schwelle = g.stimmberechtigte().quorum_threshold();
        assert_eq!(schwelle, 600_000_001, "⌊2·900/3⌋ + 1");

        let stake: Vec<u64> = g.validatoren.iter().map(|v| v.stake).collect();
        let mut sortiert = stake.clone();
        sortiert.sort_unstable_by(|a, b| b.cmp(a));
        assert_eq!(
            sortiert,
            vec![250_000_000, 230_000_000, 200_000_000, 120_000_000, 100_000_000]
        );

        // 1. Drei Köpfe, unter der Schwelle: eine Mehrheit der Köpfe ist
        //    kein Quorum. Fängt Kopfzählung statt Gewichtszählung.
        assert!(200_000_000u64 + 120_000_000 + 100_000_000 < schwelle);
        // 2. Drei Köpfe, **exakt** zwei Drittel: reicht nicht. Fängt ein
        //    verrutschtes `+ 1` in quorum_threshold, und daran hängt
        //    BFT-Safety: Bei exakt 2/3 überschneiden sich zwei Quoren
        //    nicht mehr zwingend in einem ehrlichen Gewicht.
        assert_eq!(250_000_000u64 + 230_000_000 + 120_000_000, 600_000_000);
        assert!(600_000_000 < schwelle);
        // 3. Drei Köpfe, darüber: die Gegenprobe. Ohne sie wäre „immer
        //    ablehnen" grün.
        assert!(250_000_000u64 + 230_000_000 + 200_000_000 >= schwelle);
    }

    /// ⚑ **Warum vier Knoten nicht gereicht hätten.**
    ///
    /// Damit drei von vier das Quorum verfehlen, müsste der
    /// ausgeschlossene Vierte mehr als ein Drittel halten, und dann
    /// verletzt das Netz seine eigene Sicherheitsannahme. Umgekehrt
    /// erreichen zwei Knoten mit je höchstens einem Drittel zusammen
    /// höchstens zwei Drittel, und das Quorum verlangt **mehr**.
    /// Bei vier Knoten gilt also zwangsläufig „Quorum genau dann, wenn
    /// drei Köpfe", und der Unterschied ist nicht zeigbar.
    #[test]
    fn bei_vier_knoten_ist_der_unterschied_nicht_konstruierbar() {
        for stakes in [
            [250u64, 250, 250, 250],
            [290, 280, 220, 210],
            [299, 251, 250, 200],
            [280, 270, 260, 190],
        ] {
            let mut text = String::from("netz vier\n");
            for (i, stake) in stakes.iter().enumerate() {
                let (pk, pop) = schluessel(30 + i as u8);
                text.push_str(&format!(
                    "validator {} {} {}\n",
                    als_hex(&pk.0),
                    als_hex(&pop.0),
                    stake
                ));
            }
            let g = Genesis::aus_text(&text)
                .unwrap_or_else(|e| panic!("{stakes:?} sollte gültig sein: {e}"));
            assert!(
                g.unterscheidet_koepfe_von_gewicht().is_none(),
                "{stakes:?} zeigte den Unterschied doch. Dann stimmt die \
                 Herleitung im Doc-Kommentar nicht"
            );
        }
    }

    // ── Fund 27: Besitznachweis ─────────────────────────────────────

    #[test]
    fn fund_27_ein_schluessel_ohne_besitznachweis_kommt_nicht_hinein() {
        // Ohne diese Prüfung ließe sich ein Schlüssel eintragen, dessen
        // geheimen Teil niemand kennt, aber so gewählt, dass er die
        // Aggregatsignatur der anderen verschiebt.
        let (_, pop_fremd) = schluessel(41);
        let text = probenetz_text();
        let erste = text
            .lines()
            .find(|l| l.starts_with("validator"))
            .expect("eine validator-Zeile");
        let felder: Vec<&str> = erste.split_whitespace().collect();
        let verfaelscht = format!(
            "validator {} {} {}",
            felder[1],
            als_hex(&pop_fremd.0),
            felder[3]
        );
        assert!(matches!(
            Genesis::aus_text(&text.replace(erste, &verfaelscht)),
            Err(GenesisFehler::BesitzNichtNachgewiesen { .. })
        ));
    }

    #[test]
    fn derselbe_schluessel_zweimal_faellt_auf() {
        let (pk, pop) = schluessel(50);
        let zeile = format!("validator {} {} 100\n", als_hex(&pk.0), als_hex(&pop.0));
        let (pk2, pop2) = schluessel(51);
        let text = format!(
            "netz doppelt\n{zeile}{zeile}validator {} {} 400\n",
            als_hex(&pk2.0),
            als_hex(&pop2.0)
        );
        assert!(matches!(
            Genesis::aus_text(&text),
            Err(GenesisFehler::DoppelterSchluessel { .. })
        ));
    }

    // ── Format ──────────────────────────────────────────────────────

    #[test]
    fn die_datei_laesst_sich_zurueckschreiben_und_wieder_lesen() {
        let g = probenetz();
        let wieder = Genesis::aus_text(&g.als_text()).expect("Rückschrift muss lesbar sein");
        assert_eq!(g, wieder);
        assert_eq!(g.hash(), wieder.hash());
    }

    #[test]
    fn eine_datei_ohne_netznamen_wird_abgewiesen() {
        let (pk, pop) = schluessel(60);
        let text = format!("validator {} {} 100\n", als_hex(&pk.0), als_hex(&pop.0));
        assert_eq!(Genesis::aus_text(&text), Err(GenesisFehler::OhneNetznamen));
    }

    #[test]
    fn eine_datei_ohne_validatoren_wird_abgewiesen() {
        assert_eq!(
            Genesis::aus_text("netz leer\n"),
            Err(GenesisFehler::OhneValidatoren)
        );
    }

    #[test]
    fn ein_tippfehler_im_schluessel_faellt_auf_und_nennt_die_zeile() {
        let text = "netz x\nvalidator abcd deadbeef 100\n";
        match Genesis::aus_text(text) {
            Err(GenesisFehler::KeinHex { zeile, feld, .. }) => {
                assert_eq!((zeile, feld), (2, "pubkey"));
            }
            andere => panic!("erwartet KeinHex, bekommen {andere:?}"),
        }
    }

    #[test]
    fn eine_unbekannte_anweisung_wird_nicht_ueberlesen() {
        // Eine stillschweigend ignorierte Zeile ist eine Zeile, die
        // jemand für wirksam hält.
        let text = format!("netz x\nvalidatoren 5\n{}", probenetz_text());
        assert!(matches!(
            Genesis::aus_text(&text),
            Err(GenesisFehler::UnbekannteZeile { zeile: 2, .. })
        ));
    }

    #[test]
    fn stake_null_wird_abgewiesen() {
        // Gewicht null hieße: darf mitreden, zählt nie. Das ist kein
        // Validator, das ist ein Missverständnis.
        let (pk, pop) = schluessel(70);
        let text = format!(
            "netz x\nvalidator {} {} 0\n",
            als_hex(&pk.0),
            als_hex(&pop.0)
        );
        assert!(matches!(
            Genesis::aus_text(&text),
            Err(GenesisFehler::UnbrauchbarerStake { zeile: 2 })
        ));
    }

    #[test]
    fn das_stimmgewicht_zu_genesis_ist_der_stake() {
        // Keine Vereinfachung: Die Arbeitshistorie ist leer, der Bonus
        // also null, und der Deckel greift nicht.
        let g = probenetz();
        let menge = g.stimmberechtigte();
        assert_eq!(menge.len(), 5);
        assert_eq!(menge.total_weight(), 900_000_000);
        for v in &g.validatoren {
            assert_eq!(menge.weight(&v.kennung()), v.stake);
        }
    }
}
