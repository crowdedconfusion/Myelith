//! **Die Netze, die dieser Knoten kennt** (Stufe 4 des Dispatchers,
//! 2026-10-22): je Netz Startwert, Podmodell und vTFE-Profil.
//!
//! # ⚑ Warum zwei
//!
//! Die **Probekette** rechnet das Probemodell (Sparrow 0.6B, nur in den
//! Proben auf der Netzliste) und ist die Kette jeder Probe. Das **Testnetz**
//! rechnet das int4-35B: Damit beginnt die Testphase (Festlegung des
//! Projektinhabers vom 2026-10-22; über int8 oder int4 wird danach
//! entschieden). Beide sind keine echte Kette, und beide sagen es in ihrem
//! Startwert.
//!
//! # ⚑ Das Netz folgt aus dem Zustand
//!
//! Der Kettenzustand trägt das Podmodell seit dem Beginn der Kette; daraus
//! findet [`Netz::des_zustands`] das Netz zurück, und mit ihm Startwert und
//! Profil. So brauchen die Ableitungen der Kette, die nur den Zustand sehen
//! (Zuteilung, Gutschrift, Anwenden), keinen zweiten Parameter, und zwei
//! Quellen für dasselbe Netz kann es nicht geben.
//!
//! # ⚑ Was ein Netz bindet
//!
//! - **Startwert**: Eine Transaktion für das eine Netz scheitert im anderen
//!   an der Prüfung, eine Kettendatei des einen öffnet das andere nicht.
//! - **Podmodell**: Abdruck, Ebenenzahl und Kopf; gegen den Katalog
//!   gehalten durch Proben unten.
//! - **Profil**: die Rechenarbeit je Ebene, nach der die Kette gutschreibt.

use myl_tokenomics::vtfe::ModellProfil;
use myl_types::hash::Hash;
use myl_types::podmodell::Podmodell;

/// Ein Netz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Netz {
    /// Der Name auf der Kommandozeile (`--netz`).
    pub name: &'static str,
    /// Der Text, dessen SHA-256 der Startwert ist.
    pub startwert: &'static [u8],
    /// Was die Pods rechnen.
    pub podmodell: Podmodell,
    /// Wie die Kette ihre Arbeit gutschreibt.
    pub profil: ModellProfil,
    /// Der Schlüssel des Artefakts im Katalog, für Meldungen und Proben.
    pub artefakt: &'static str,
    /// ⚑ **Die Vorlage des Netzmodells** (Fund 181, 2026-11-16): In ihr
    /// setzt jede Tür dieses Netzes eine Unterhaltung zusammen. Eine
    /// Protokollgröße und keine Einstellung eines Knotens: Zwei Türen
    /// desselben Netzes setzen denselben Prompt, und ein Klient kann ihn aus
    /// seinen Nachrichten nachrechnen.
    pub vorlage: myl_types::vorlage::Vorlage,
    /// ⚑ **Die erste Tür** (seit dem 2026-10-22): Adressen langlebiger
    /// Knoten (`/ip4/…/tcp/…/p2p/…`), die jeder Knoten dieses Netzes beim
    /// Start zusätzlich zu seinen eigenen Bootstrap-Angaben kennt. Kein
    /// Server: Sie öffnen nur den Weg ins verteilte Adressbuch, und jeder
    /// andere Knoten kann dasselbe.
    pub einstieg: &'static [&'static str],
    /// Ob Knoten dieses Netzes einander im lokalen Netz suchen (mDNS).
    pub lokale_suche: bool,
    /// ⚑ **Anker für den Zustandsabzug** (2026-10-22): Höhe und Blockhash
    /// dauerhaft abgelegter Epochengrenzen, eingetragen mit einem Release.
    /// Ein Neuling ohne eigene Kette beginnt beim jüngsten statt bei Block 0.
    /// Nur Höhen, die jeder Knoten auf Dauer behält (`abzug::ist_langzeit`).
    pub abzuege: &'static [(u64, [u8; 32])],
    /// ⚑ **Der Startsatz aus dem Release** (2026-11-18): der Text einer
    /// Stimmsatzdatei (Validatoren und `start`), wie ihn `--stimmsatz` liest.
    /// Ohne `--stimmsatz` legt der Knoten ihn neben seine Schluesseldatei und
    /// nimmt ihn ([`Netz::stimmsatz_ablegen`]). Vertraut wie Einstieg und
    /// Anker: Er kommt mit einem Release, und ein Release traegt die Signatur
    /// des Freigabeschluessels.
    ///
    /// 📌 **Warum** (Fund vom 2026-11-18, beim Entwurf der
    /// Betreiberanleitung): Jedes Netz ausser der Probekette startet nur mit
    /// einem Stimmsatz, und `myl mining` gab nie einen mit. Ein Miner kam
    /// ueber den Klienten also in kein Testnetz.
    pub stimmsatz: Option<&'static str>,
    /// ⚑ **Die Probeprägung** (Festlegung des Projektinhabers vom
    /// 2026-10-26): So viele Einheiten bekommt an jedem Epochenwechsel das
    /// Auszahlungskonto jedes angemeldeten, nicht gesperrten Miners, damit
    /// ein Testwallet beim Mining sichtbar wächst, bevor der Weg über
    /// bezeugte Arbeit im Betrieb steht.
    ///
    /// ⛔️ **Null ausser auf der Probekette.** Das ist Spielgeld ohne
    /// Arbeitsnachweis; in einem Netz, dessen MYL etwas gelten sollen,
    /// wäre es eine Prägung aus dem Nichts. Die Probe
    /// `nur_die_probekette_praegt_zur_probe` hält das fest.
    pub probepraegung: u64,
    /// ⚑ **Ob die Kette ihren Stimmsatz aus Block 1 lernt** (2026-10-27,
    /// Festlegung des Projektinhabers: Stimmsatz ab Genesis, immer Pflicht).
    /// Jeder Block braucht die Unterschrift eines Mitglieds. Ohne
    /// Stimmsatzdatei wird auf einer Probekette, wer Block 1 unterschrieben
    /// hat, das einzige Mitglied; das ist Vertrauen beim ersten Block und so
    /// schwach wie die Probekette selbst, die keinen Anker hat. ⛔️ **Jedes
    /// andere Netz startet ohne `--stimmsatz` nicht.**
    pub lernt_erzeuger: bool,
    /// ⚑ **Die Gebühren dieses Netzes** (2026-10-27, G1, Zahlen vorläufig).
    pub gebuehren: Gebuehren,
    /// ⚑ **Der Probehahn** (2026-10-27, Festlegung des Projektinhabers: nur
    /// auf Testnetzen): Ein Knoten versorgt sein Konto selbst aus den
    /// Probekonten, deren Schlüssel jeder ableiten kann, damit er Gebühr und
    /// Hinterlegung zahlen kann. ⛔️ **Aus in jedem Netz, dessen MYL etwas
    /// gelten sollen**: Dort gibt es keine Probekonten.
    pub probehahn: bool,
    /// ⚑ **Ein eigener Mindesteinsatz fuer den Stimmsatz** (2026-11-03), nur
    /// auf der Probekette: Der abgeleitete (`S_min = g/p²`, 1 250 MYL) ist
    /// mit der Probepraegung von zehn MYL je Epoche in keinem Lauf zu
    /// erreichen; ohne Stimmsatz aus dem Zustand gaebe es dort auch keine
    /// Saat aus dem Satz, und ein Lauf bewiese nichts darueber. ⛔️ **`None`
    /// in jedem anderen Netz**: Dort gilt der abgeleitete Wert. Die Probe
    /// `nur_die_probekette_senkt_den_mindesteinsatz` haelt das fest.
    pub stimmsatz_mindesteinsatz: Option<u64>,
    /// ⚑ **Nach wie vielen verfehlten Epochen ein Mitglied ruht**
    /// (2026-11-05), nur auf der Probekette kürzer: Mit
    /// [`myl_ledger::stimmsatz::INAKTIV_EPOCHEN`] (24) dauerte die Messung,
    /// ob die Finalität nach einem Ausfall von mehr als einem Drittel von
    /// selbst zurückkehrt, bei 500 ms je Slot über sechs Stunden. ⛔️ `None`
    /// in jedem anderen Netz.
    pub stimmsatz_inaktiv_epochen: Option<u64>,
    /// ⚑ **Die Länge eines Slots in Millisekunden** (2026-11-04,
    /// Erzeugerplan, Festlegung des Projektinhabers: je Netz). Je Slot ist
    /// ein Erzeuger gezogen; ein Block muss den nächsten erreichen, bevor
    /// dessen Slot beginnt, sonst gabelt die Kette. Alle Knoten eines Netzes
    /// rechnen mit derselben Länge, sonst hielten sie Blöcke für künftig.
    pub slot_ms: u64,
    /// ⚑ **Was ein Pod je Sekunde rechnet, in vTFE-Einheiten** (2026-11-26,
    /// Festlegung des Projektinhabers: Kapazität aus gemessener Rate und
    /// Epochenlänge). Daraus und aus der Länge eines Slots folgt die
    /// Kapazität einer Epoche ([`crate::kette::kapazitaet_je_pod`]), gegen
    /// die Preis und Trainingsmenge die Nachfrage messen (Whitepaper 5.4,
    /// 7.1).
    ///
    /// 📌 **Warum je Sekunde und nicht je Epoche:** Bis hierher stand fest
    /// [`POD_LEISTUNG_VORGABE`] mal eine Stunde da, auch in Läufen mit
    /// kürzerem Takt. Im Auftragslauf vom 2026-11-25 (Epoche 24 min) rechnete
    /// ein Pod des 35B je Epoche fast das Fünffache dessen, was die Kette ihm
    /// zutraute, und der Credit-Preis stieg mit einem einzigen Kunden in jeder
    /// Epoche am Deckel.
    pub pod_leistung_vtfe_je_s: u64,
}

/// ⚑ **Die Vorgabe der Governance, je Sekunde** (`PodKapazitaet` durch
/// `Epochenlaenge`: 3,6 Mrd. vTFE-Einheiten je Stunde, ein Pass je
/// Sekunde). Eine Grössenordnung und kein Messwert; sie gilt, wo nichts
/// gemessen ist. Die Probe `die_vorgabe_der_leistung_folgt_der_governance`
/// hält sie gegen den Spiegelwert der Kette.
pub const POD_LEISTUNG_VORGABE: u64 = myl_tokenomics::VTFE_UNITS_PER_TFE;

/// ⚑ **Die Zahlen der Gebühren** (Festlegung des Projektinhabers vom
/// 2026-10-27: Boden plus wandernder Bytepreis, Hinterlegung für neuen
/// Zustand, Aufschlag je neuem Konto). ⚠️ **Vorläufig**: Die Mechanik ist
/// entschieden, die Zahlen sind vor dem Testnetz festzulegen. Alle in
/// MYL-Kleinstbeträgen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gebuehren {
    /// Der Grundpreis je Byte fällt nie darunter.
    pub boden_je_byte: u64,
    /// Wer an ein neues Konto überweist, zahlt das einmal dazu (an die
    /// Treasury): Ein Konto wird nie gelöscht (die Nonce schützt vor
    /// Wiederholung), also ist es dauerhafter Zustand.
    pub aufschlag_konto: u64,
    /// Hinterlegung für eine Anmeldung als Miner, zurück beim Abmelden.
    pub hinterlegung_anmeldung: u64,
    /// Hinterlegung für eine Sitzung, zurück beim Aufräumen.
    pub hinterlegung_sitzung: u64,
    /// Hinterlegung für ein Speichermanifest, zurück beim Löschen.
    /// ⚠️ **Ruht** (Festlegung des Projektinhabers vom 2026-10-28, Fund 663):
    /// Die Kette kennt noch keine Anweisung für Manifeste; sie entsteht samt
    /// dieser Hinterlegung mit der Speicherrolle der Komponente STORAGE.
    pub hinterlegung_manifest: u64,
}

/// Die vorläufigen Zahlen (2026-10-27, mit dem Projektinhaber): Boden ein
/// Kleinstbetrag je Byte (eine Überweisung von rund 300 Byte kostet
/// 0,0003 MYL), Konto 0,01 MYL, Anmeldung 1 MYL, Sitzung und Manifest je
/// 0,1 MYL.
pub const GEBUEHREN_VORLAEUFIG: Gebuehren = Gebuehren {
    boden_je_byte: 1,
    aufschlag_konto: myl_tokenomics::UNITS_PER_MYL / 100,
    hinterlegung_anmeldung: myl_tokenomics::UNITS_PER_MYL,
    hinterlegung_sitzung: myl_tokenomics::UNITS_PER_MYL / 10,
    hinterlegung_manifest: myl_tokenomics::UNITS_PER_MYL / 10,
};

impl Netz {
    /// Der Startwert als Hash.
    pub fn startwert_hash(&self) -> Hash {
        Hash::sha256(self.startwert)
    }

    /// Der Stand der Pipeline, den die Arbeitsverteilung eines Pods nennt:
    /// der Abdruck des Podmodells.
    pub fn pipeline(&self) -> Hash {
        Hash::from_bytes(self.podmodell.abdruck)
    }

    /// Der jüngste Anker dieses Netzes für den Zustandsabzug.
    pub fn juengster_anker(&self) -> Option<crate::abzug::Anker> {
        self.abzuege
            .iter()
            .max_by_key(|(h, _)| *h)
            .map(|(hoehe, b)| crate::abzug::Anker { hoehe: *hoehe, blockhash: Hash::from_bytes(*b) })
    }

    /// Ob Höhe und Blockhash ein Anker dieses Netzes aus dem Quelltext sind.
    ///
    /// ⚑ **Ein solcher Anker verdient dasselbe Vertrauen wie Genesis**: Er
    /// steht im ausgelieferten Programm. Eine Basis genau dort braucht keine
    /// Hintergrundprüfung, und seit die Knoten ihre Kette verdichten
    /// (2026-10-23), könnte ihr auch niemand mehr die Blöcke ab Genesis
    /// liefern.
    pub fn ist_anker(&self, hoehe: u64, blockhash: &Hash) -> bool {
        self.abzuege.iter().any(|(h, b)| *h == hoehe && b == blockhash.as_bytes())
    }

    /// **Legt den Startsatz aus dem Release ab**, als Datei
    /// `<netz>.stimmsatz` im Ordner, und gibt ihren Pfad. `Ok(None)`, wenn
    /// das Release fuer dieses Netz keinen mitbringt.
    ///
    /// Gelesen wird er vorher: Ein unlesbarer Satz oder einer fuer ein
    /// anderes Netz ist ein Fehler des Release, und der Knoten startet dann
    /// nicht, statt mit einem Satz zu laufen, den niemand meinte.
    pub fn stimmsatz_ablegen(&self, ordner: &std::path::Path) -> Result<Option<std::path::PathBuf>, String> {
        let Some(text) = self.stimmsatz else {
            return Ok(None);
        };
        let datei = crate::stimmsatzdatei::Stimmsatzdatei::aus_text(text)
            .map_err(|e| format!("der Startsatz des Netzes {} in diesem Release ist unlesbar: {e}", self.name))?;
        if datei.netz != self.name {
            return Err(format!(
                "der Startsatz des Netzes {} in diesem Release gehoert zum Netz {}",
                self.name, datei.netz
            ));
        }
        let pfad = ordner.join(format!("{}.stimmsatz", self.name));
        std::fs::write(&pfad, text).map_err(|e| format!("{}: {e}", pfad.display()))?;
        Ok(Some(pfad))
    }

    /// Das Netz zu einem Namen.
    pub fn aus_name(name: &str) -> Option<&'static Netz> {
        NETZE.iter().find(|n| n.name == name)
    }

    /// **Das Netz eines Kettenzustands**, aus seinem Podmodell.
    ///
    /// ⚑ **Ohne Podmodell die Probekette**: Ein Zustand ohne Podmodell
    /// entsteht nur in Proben, die den Ledger unmittelbar bauen, und dort
    /// gilt der Startwert der Probekette wie vor dem 2026-10-22.
    pub fn des_zustands(zustand: &myl_ledger::state::LedgerState) -> &'static Netz {
        zustand
            .podmodell
            .and_then(|pm| NETZE.iter().find(|n| n.podmodell == pm))
            .unwrap_or(&PROBEKETTE)
    }
}

/// Die Probekette: das Probemodell, nur in den Proben freigegeben.
///
/// Kopf in Platz 20 (`lm_head.bin` 311 MB gegen eine mittlere Ebene von
/// 15,7 MB, aufgerundet) und in Zeit 5 (gemessen am 2026-10-21, P1).
pub const PROBEKETTE: Netz = Netz {
    name: "probe",
    startwert: crate::kette::PROBE_STARTWERT,
    podmodell: Podmodell {
        abdruck: [
            0x92, 0x40, 0xfa, 0x59, 0xdd, 0xb4, 0x60, 0xeb,
            0x1b, 0x2e, 0xfc, 0x77, 0x87, 0x9c, 0xc7, 0x5e,
            0x68, 0x88, 0x2a, 0x5c, 0x43, 0x0b, 0x1c, 0xdc,
            0x02, 0x22, 0xcd, 0x02, 0xd4, 0xba, 0x51, 0xbd,
        ],
        ebenen: 28,
        kopf_platz: 20,
        kopf_zeit: 5,
    },
    profil: myl_tokenomics::vtfe::PROBE_MODELL,
    artefakt: "sparrow-0.6b-int8",
    // Qwen3-0.6B, Familie `qwen3`.
    vorlage: myl_types::vorlage::Vorlage::ChatMl,
    // Proben bauen ihr Netz selbst; eine lokale Suche liesse Proben auf
    // derselben Maschine einander finden.
    einstieg: &[],
    lokale_suche: false,
    abzuege: &[],
    // Die Probekette lernt ihren Erzeuger aus Block 1.
    stimmsatz: None,
    // Zehn MYL je Epoche: genug, dass ein Wallet in Minuten sichtbar
    // wächst, und rund, damit eine Abweichung auffällt.
    probepraegung: 10 * myl_tokenomics::UNITS_PER_MYL,
    lernt_erzeuger: true,
    gebuehren: GEBUEHREN_VORLAEUFIG,
    probehahn: true,
    // Ein halbes MYL: der Einsatz, den die Miner im Netzlauf hinterlegen.
    stimmsatz_mindesteinsatz: Some(myl_tokenomics::UNITS_PER_MYL / 2),
    // Zwei Epochen: Ein Ausfall kehrt in einem Lauf von zwei Stunden zurück.
    stimmsatz_inaktiv_epochen: Some(2),
    // Eine halbe Sekunde: kurz genug für viele Epochen in einem Lauf, lang
    // genug für die Laufzeiten im Netzlauf.
    slot_ms: 500,
    // Nicht gemessen: Die Probekette rechnet mit Probepods.
    pod_leistung_vtfe_je_s: POD_LEISTUNG_VORGABE,
};

/// Der Startwert des Testnetzes. Wie bei der Probekette sagt der Text,
/// was die Kette ist und was nicht.
/// Seit der Historienwurzel (2026-10-23) mit neuer Kennung, aus demselben
/// Grund wie bei [`crate::kette::PROBE_STARTWERT`].
pub const TESTNETZ_STARTWERT: &[u8] = b"MYELITH-TESTNETZ-RAVEN-35B-A3B-INT4-KEINE-ECHTE-KETTE-H1";

/// ⚑ **Das Testnetz mit dem int4-35B** (Festlegung des Projektinhabers vom
/// 2026-10-22).
///
/// Kopf in Platz 4 (beide Köpfe, 1,5 GB gegen eine mittlere Ebene von
/// 0,45 GB, aufgerundet) und in Zeit 1, gemessen am 2026-10-21 (P1).
pub const TESTNETZ_INT4: Netz = Netz {
    name: "test-int4",
    startwert: TESTNETZ_STARTWERT,
    podmodell: Podmodell {
        abdruck: [
            0x92, 0x35, 0xc8, 0x16, 0x2b, 0x39, 0x1d, 0x6f,
            0x97, 0x6d, 0xe2, 0x4c, 0x53, 0xe1, 0x45, 0xa6,
            0x66, 0x0b, 0xe8, 0x2d, 0x46, 0xb0, 0xdf, 0xf4,
            0x3d, 0x13, 0xac, 0x0b, 0x02, 0x5f, 0x01, 0x1a,
        ],
        ebenen: 40,
        kopf_platz: 4,
        kopf_zeit: 1,
    },
    profil: myl_tokenomics::vtfe::RAVEN_35B_A3B,
    artefakt: "raven-35b-a3b-int4",
    // Familie `qwen3_5-moe-hybrid`: der Denkblock steht in der Aufforderung.
    vorlage: myl_types::vorlage::Vorlage::ChatMlDenkblock,
    // ⚠️ Noch kein langlebiger Knoten: Bis es einen gibt, ist die Liste
    //   leer, und ausserhalb des lokalen Netzes braucht der Einstieg eine
    //   Bootstrap-Angabe.
    einstieg: &[],
    lokale_suche: true,
    // Noch keiner: Das Testnetz hat noch keine dauerhafte Epochengrenze.
    abzuege: &[],
    // ⚠️ Noch keiner: Wer im Startsatz steht, entscheidet der
    //   Projektinhaber. Bis dahin braucht jeder Knoten `--stimmsatz`.
    stimmsatz: None,
    probepraegung: 0,
    lernt_erzeuger: false,
    gebuehren: GEBUEHREN_VORLAEUFIG,
    // Ein Testnetz: Seine Probekonten sind Spielgeld wie auf der Probekette.
    probehahn: true,
    stimmsatz_mindesteinsatz: None,
    stimmsatz_inaktiv_epochen: None,
    // ⚑ Zwei Sekunden, die Blockzeit der Governance-Vorgabe (`Blockzeit`,
    //   Whitepaper 3.5.2: 1 bis 2 s). 1 800 Bloecke sind damit die Stunde, auf
    //   der Sperrfrist, Streitfrist und Vertrauensfrist stehen; die Probe
    //   `die_blockzeit_des_testnetzes_folgt_der_governance` haelt das fest.
    //   📌 Hier stand bis 0.117.0 eine Sekunde: Sieben Tage Sperrfrist waren
    //   dann 3,5 Tage und die Vertrauensfrist 2,3 Tage.
    slot_ms: 2_000,
    // ⚑ Gemessen im Auftragslauf vom 2026-11-25 auf einem Apple M5 Pro
    //   (Docker, vier Shards je Pod): 108 044 Pässe in 9 065 s über zwei
    //   Aufträge, also 11,9 je Sekunde. ⚠️ Eine Untergrenze: ein Klient,
    //   Runde für Runde, ohne Stapel aus mehreren Sitzungen. Auf anderer
    //   Hardware neu messen; der Auftragslauf meldet die Rate im Bericht.
    pod_leistung_vtfe_je_s: 11_900_000,
};

/// Alle Netze, die dieser Knoten kennt.
pub const NETZE: [Netz; 2] = [PROBEKETTE, TESTNETZ_INT4];

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Startsatz aus vier Probeschluesseln fuer das Netz `name`.
    fn startsatz(name: &str) -> &'static str {
        let mut text = format!("netz {name}\nstart 1794700800000\n");
        for k in ["a", "b", "c", "d"] {
            text.push_str(&crate::schluessel::Konsensschluessel::probe(k).expect("Schluessel").genesiszeile(100).expect("Zeile"));
            text.push('\n');
        }
        Box::leak(text.into_boxed_str())
    }

    fn ordner(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("myl-netz-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("Ordner");
        d
    }

    /// ⚑ **Jeder Startsatz, den ein Release mitbringt, ist lesbar und
    /// gehoert zu seinem Netz.** Heute bringt keines einen mit; die Probe
    /// greift mit dem ersten.
    #[test]
    fn jeder_startsatz_im_release_ist_lesbar_und_gehoert_zu_seinem_netz() {
        for n in NETZE {
            if let Some(text) = n.stimmsatz {
                let datei = crate::stimmsatzdatei::Stimmsatzdatei::aus_text(text)
                    .unwrap_or_else(|e| panic!("{}: {e}", n.name));
                assert_eq!(datei.netz, n.name);
                assert!(datei.start_ms.is_some(), "{}: ohne `start` kaeme ein Einsteiger nur mit Anker hinein (A43)", n.name);
            }
        }
    }

    /// ⚑ **Der Startsatz aus dem Release landet als Datei beim Knoten**,
    /// genau so, wie `--stimmsatz` ihn liest. Gegenproben: ohne Satz keine
    /// Datei, und ein Satz fuer ein anderes Netz wird abgewiesen.
    #[test]
    fn der_startsatz_aus_dem_release_wird_abgelegt() {
        let d = ordner("ablage");
        let netz = Netz { stimmsatz: Some(startsatz("test-int4")), ..TESTNETZ_INT4 };
        let pfad = netz.stimmsatz_ablegen(&d).expect("abgelegt").expect("ein Pfad");
        assert_eq!(pfad, d.join("test-int4.stimmsatz"));
        let text = std::fs::read_to_string(&pfad).expect("lesen");
        let datei = crate::stimmsatzdatei::Stimmsatzdatei::aus_text(&text).expect("lesbar");
        assert_eq!(datei.stimmberechtigte().len(), 4);
        assert_eq!(datei.start_ms, Some(1_794_700_800_000));

        let ohne = ordner("ohne");
        assert_eq!(TESTNETZ_INT4.stimmsatz_ablegen(&ohne), Ok(None));
        assert_eq!(std::fs::read_dir(&ohne).expect("Ordner").count(), 0, "ohne Satz keine Datei");

        let fremd = Netz { stimmsatz: Some(startsatz("probe")), ..TESTNETZ_INT4 };
        let fehler = fremd.stimmsatz_ablegen(&ordner("fremd")).expect_err("ein Satz fuer ein anderes Netz");
        assert!(fehler.contains("gehoert zum Netz probe"), "{fehler}");
        for x in ["ablage", "ohne", "fremd"] {
            let _ = std::fs::remove_dir_all(ordner(x));
        }
    }

    /// Der Eintrag eines Artefakts im Katalog, als Text bis zu seinem Ende.
    fn eintrag(name: &str) -> &'static str {
        let katalog = include_str!("../../../MODELS/llm/KATALOG.json");
        let ab = katalog.find(&format!("\"{name}\": {{")).unwrap_or_else(|| panic!("{name} fehlt im Katalog"));
        let rest = &katalog[ab..];
        &rest[..rest.find("\n  }").expect("Eintrag endet nicht")]
    }

    /// ⚑ **Jedes Netz gegen den Katalog**: Abdruck auf der Netzliste (beim
    /// Testnetz) oder im Prüfnetz (bei der Probekette), Ebenenzahl und
    /// Breite des Profils wie dort. Ein neu gebautes Artefakt bekommt
    /// einen neuen Abdruck; dann muss das Netz mit.
    /// ⚑ Ein Anker steht nur auf einer Höhe, die jeder Knoten auf Dauer
    /// behält; auf einer gewöhnlichen könnte ihn zwei Epochen später
    /// niemand mehr liefern.
    /// ⛔️ Spielgeld gibt es nur auf der Probekette; jedes andere Netz
    /// prägt allein für bezeugte Arbeit.
    #[test]
    fn nur_die_probekette_praegt_zur_probe() {
        for n in NETZE {
            if n.name == PROBEKETTE.name {
                assert!(n.probepraegung > 0, "die Probekette prägt nichts zur Probe");
            } else {
                assert_eq!(n.probepraegung, 0, "{} prägt Spielgeld", n.name);
            }
        }
    }

    /// ⛔️ Einen gesenkten Mindesteinsatz fuer den Stimmsatz und eine kurze
    /// Frist bis zum Ruhen gibt es nur auf der Probekette; jedes andere Netz
    /// rechnet den Einsatz aus `S_min` und ruht nach 24 Epochen.
    #[test]
    fn nur_die_probekette_senkt_den_mindesteinsatz() {
        for n in NETZE {
            if n.name == PROBEKETTE.name {
                assert!(n.stimmsatz_mindesteinsatz.is_some());
                assert!(n.stimmsatz_inaktiv_epochen.is_some());
            } else {
                assert_eq!(n.stimmsatz_mindesteinsatz, None, "{} senkt den Mindesteinsatz", n.name);
                assert_eq!(n.stimmsatz_inaktiv_epochen, None, "{} verkürzt die Frist bis zum Ruhen", n.name);
            }
        }
    }

    #[test]
    fn jeder_anker_steht_auf_einer_dauerhaften_hoehe() {
        for n in NETZE {
            for (h, _) in n.abzuege {
                assert!(crate::abzug::ist_langzeit(*h), "{}: Anker bei {h} wird nicht auf Dauer behalten", n.name);
            }
        }
    }

    #[test]
    fn jedes_netz_stimmt_mit_dem_katalog() {
        for (netz, block) in [(PROBEKETTE, "pruefnetz"), (TESTNETZ_INT4, "netz")] {
            let e = eintrag(netz.artefakt);
            let hex: String = netz.podmodell.abdruck.iter().map(|b| format!("{b:02x}")).collect();
            let block_ab = e.find(&format!("\"{block}\": {{")).unwrap_or_else(|| panic!("{}: {block} fehlt", netz.name));
            assert!(e[block_ab..].contains(&format!("\"abdruck\": \"{hex}\"")), "{}: Abdruck weicht ab", netz.name);
            assert!(e.contains(&format!("\"layer\": {},", netz.podmodell.ebenen)), "{}: Ebenenzahl weicht ab", netz.name);
            assert_eq!(netz.profil.num_layers, netz.podmodell.ebenen as u64, "{}: Profil und Podmodell", netz.name);
            assert!(e.contains(&format!("\"hidden_size\": {},", netz.profil.hidden_size)), "{}: Breite weicht ab", netz.name);
        }
    }

    /// Zwei Netze, zwei Startwerte, zwei Podmodelle; jedes findet sich aus
    /// seinem Zustand wieder.
    #[test]
    fn die_netze_sind_verschieden_und_finden_sich_wieder() {
        assert_ne!(PROBEKETTE.startwert_hash(), TESTNETZ_INT4.startwert_hash());
        assert_ne!(PROBEKETTE.podmodell, TESTNETZ_INT4.podmodell);
        for n in NETZE {
            let mut z = myl_ledger::state::LedgerState::genesis(1);
            z.podmodell = Some(n.podmodell);
            assert_eq!(Netz::des_zustands(&z), &n);
            assert_eq!(Netz::aus_name(n.name), Some(&n));
        }
        assert_eq!(Netz::des_zustands(&myl_ledger::state::LedgerState::genesis(1)), &PROBEKETTE);
        assert!(Netz::aus_name("echt").is_none());
    }
}
