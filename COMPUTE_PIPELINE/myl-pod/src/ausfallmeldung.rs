//! Die Ausfallmeldung: Frist und Gegenzeichnung (Punkt 3.5).
//!
//! # ⚑ Eine Ausfallmeldung ist eine Waffe
//!
//! Wer melden darf, dass ein anderer ausgefallen sei, kann einen
//! **ehrlichen** Knoten aus seinem Pod werfen und seinen Platz füllen
//! lassen. Bis zu diesem Punkt genügte dafür der Aufruf von
//! [`crate::standby::PodBesetzung::ausfall`]: eine Behauptung ohne
//! Absender, ohne Beleg und ohne Frist.
//!
//! Punkt 3.4 hat das Problem **vergrößert**, nicht verkleinert. Die
//! Netzreserve verlängert den Vorrat, aus dem nachbesetzt wird, und
//! wiederholte Meldungen über dieselbe Position ziehen ihn ganz leer:
//! Nach einer geglückten Übernahme sitzt wieder jemand dort, und „der
//! ist ausgefallen" ist dann eine neue Aussage über eine neue Person.
//! **Dasselbe Leck mit größerem Eimer.**
//!
//! # Was hier eingeführt wird, und was jedes Stück leistet
//!
//! **Erstens: Die Meldung nennt, wer ausgefallen sein soll**, nicht nur
//! die Position ([`Ausfallbehauptung::gemeldeter`]). Damit ist eine
//! wiederholte Meldung erkennbar dieselbe Aussage und keine neue, und
//! eine über den Nachrücker ist erkennbar eine andere. Ohne dieses Feld
//! ist die Entprellung aus Punkt 3.1 wirkungslos, sobald einmal
//! nachbesetzt wurde.
//!
//! **Zweitens: Gegenzeichnung.** Eine Meldung wirkt erst, wenn eine
//! **Mehrheit der übrigen Mitglieder** sie unterschrieben hat
//! ([`mindestzeichner`]). Ein einzelnes bösartiges Mitglied wirft
//! niemanden mehr heraus.
//!
//! **Drittens: eine Frist.** Die Unterschriften müssen innerhalb von
//! [`FRIST_MS`] nach der ersten eintreffen. Ohne sie ließen sich
//! Unterschriften über Stunden sammeln und im günstigen Moment
//! zusammenlegen; ein Ausfall, der vor einer Stunde bezeugt wurde, sagt
//! über jetzt nichts.
//!
//! # ⚑ Und was das alles **nicht** leistet
//!
//! **Gegen eine bösartige Mehrheit des Pods hilft es nicht.** Der Pod
//! ist kein BFT-Komitee; er ist eine Pipeline, und das
//! Verifikationsmodell geht ausdrücklich davon aus, dass ein ganzer Pod
//! falsch rechnen kann. Genau deshalb gibt es die Redundanz mit einem
//! zweiten Pod. Eine Mehrheit, die sich einig ist, wirft jeden heraus,
//! den sie herauswerfen will.
//!
//! **Was bleibt, ist zweierlei, und beides ist etwas wert:** Ein
//! **einzelner** Angreifer kann es nicht mehr, und jede Verdrängung
//! hinterlässt **unterschriebene Aussagen mit Namen**. Wer einen
//! ehrlichen Knoten verdrängt, tut es nachweisbar, und der Nachweis
//! liegt in einer Form vor, die eine Schiedsstelle lesen kann.
//!
//! Dieser Absatz steht hier, damit niemand die Gegenzeichnung für eine
//! Sicherheitsgarantie hält. Sie ist eine **Kosten- und
//! Nachweisverschiebung**, dieselbe Art Aussage wie bei der
//! Adressvielfalt in der Netzschicht.
//!
//! # Die Uhr muss monoton sein
//!
//! [`FRIST_MS`] wird gegen eine vom Aufrufer gelieferte Zeit gemessen.
//! Sie muss **monoton** sein; eine Wanduhr, die zurückspringt, macht aus
//! einer abgelaufenen Frist eine laufende. Dieselbe Bedingung wie beim
//! Rundenwechsel im Konsens.

use std::collections::BTreeSet;

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::bls::{BlsPublicKey, BlsSecretKey, BlsSignature};
use myl_types::ids::{EpochId, MinerId};
use myl_types::uebergang::Rolle;

/// Domain-Separation-Präfix der Ausfallmeldung.
///
/// Eigenes Präfix, additiv wie die übrigen: Eine in dieser Klasse
/// abgegebene Signatur gilt in keiner anderen.
pub const DST_AUSFALLMELDUNG: &[u8] = b"MYELITH_AUSFALLMELDUNG_v1";

/// Wie lange nach der ersten Unterschrift die übrigen eintreffen dürfen.
///
/// Fünf Sekunden. Die Größenordnung folgt aus der Blockzeit von zwei
/// Sekunden: Ein Ausfall, der wirklich einer ist, fällt allen
/// Mitgliedern innerhalb weniger Pipeline-Schritte auf. Wer länger
/// braucht, sammelt und legt zusammen, statt zu bezeugen.
pub const FRIST_MS: u64 = 5_000;

/// Untergrenze der Gegenzeichner, unabhängig von der Pod-Größe.
///
/// ⚑ **Zwei, nie einer.** Bei einer rechnerischen Mehrheit von eins
/// wäre die Gegenzeichnung keine: Ein einzelnes Mitglied entschiede
/// wieder allein. Ein Pod, der so klein ist, dass zwei Zeichner nicht
/// zusammenkommen, kann niemanden verdrängen; das ist die sichere
/// Richtung des Fehlers.
pub const MINDESTENS_ZEICHNER: usize = 2;

/// Wie viele Unterschriften eine Meldung braucht.
///
/// Mehrheit der **übrigen** Mitglieder, also ohne den Gemeldeten, und
/// mindestens [`MINDESTENS_ZEICHNER`]. Bei sechs Mitgliedern bleiben
/// fünf übrig, die Mehrheit ist drei.
///
/// **Warum Mehrheit und nicht Einstimmigkeit:** Einstimmigkeit gäbe
/// jedem einzelnen Mitglied ein Veto gegen jede Nachbesetzung, und ein
/// Pod, der nicht nachbesetzen kann, verliert die Sitzung. Der Angriff
/// wechselte damit nur die Richtung.
pub fn mindestzeichner(mitgliederzahl: usize) -> usize {
    let uebrige = mitgliederzahl.saturating_sub(1);
    (uebrige / 2 + 1).max(MINDESTENS_ZEICHNER)
}

/// Was behauptet wird: eine Position, und wer darauf saß.
#[derive(Debug, Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Ausfallbehauptung {
    /// Die Epoche, in der die Behauptung gilt.
    ///
    /// Sie bindet die Meldung an eine Pod-Zusammensetzung. Ohne sie wäre
    /// eine Unterschrift aus der vorigen Epoche heute noch gültig.
    pub epoche: EpochId,
    /// Der Pod-Index innerhalb der Epochenzuteilung.
    pub pod: u32,
    /// Die Shard-Position im Pod.
    pub position: u32,
    /// ⚑ **Wer ausgefallen sein soll**, nicht nur wo.
    ///
    /// Das Feld ist der Unterschied zwischen einer wiederholten Aussage
    /// und einer neuen. Ohne es zieht dieselbe Meldung nach jeder
    /// Übernahme den nächsten Vorrat.
    pub gemeldeter: MinerId,
}

impl Ausfallbehauptung {
    /// Die zu signierenden Bytes: `DST ‖ Rolle ‖ Borsh(self)`.
    ///
    /// Dieselbe Bauart wie bei der Übergangssignatur: feste Feldbreiten
    /// in fester Reihenfolge, also präfixfrei.
    pub fn to_sign_bytes(&self) -> Vec<u8> {
        let borsh_bytes = borsh::to_vec(self).expect("Ausfallbehauptung ist stets serialisierbar");
        let mut msg = Vec::with_capacity(DST_AUSFALLMELDUNG.len() + 1 + borsh_bytes.len());
        msg.extend_from_slice(DST_AUSFALLMELDUNG);
        msg.push(Rolle::PodMitglied.byte());
        msg.extend_from_slice(&borsh_bytes);
        msg
    }

    /// Unterschreibt die Behauptung in der Rolle `PodMitglied`.
    pub fn signieren(&self, sk: &BlsSecretKey) -> Result<BlsSignature, String> {
        sk.sign(&self.to_sign_bytes()).map_err(|e| e.to_string())
    }
}

/// Eine unterschriebene Meldung eines einzelnen Mitglieds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meldung {
    /// Wer meldet.
    pub melder: MinerId,
    /// Seine Unterschrift über die Behauptung.
    pub signature: BlsSignature,
}

/// Warum eine Meldung nicht zählt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meldungsfehler {
    /// Die Meldung gehört zu einer anderen Behauptung.
    FremdeBehauptung,
    /// Der Gemeldete zeichnet seinen eigenen Ausfall gegen.
    ///
    /// ⚑ Das ist kein Formfehler. Wer sich selbst für ausgefallen
    /// erklärt, gibt seine Position freiwillig ab, und das ist ein
    /// anderer Vorgang als eine Verdrängung; er hat hier nichts zu
    /// suchen und **zählt nicht zur Mehrheit**.
    DerGemeldeteSelbst,
    /// Der Melder gehört diesem Pod nicht an.
    KeinMitglied,
    /// Dieses Mitglied hat schon gezeichnet.
    SchonGezeichnet,
    /// Die Unterschrift stimmt nicht.
    Unterschrift,
    /// Die Frist seit der ersten Unterschrift ist verstrichen.
    FristAbgelaufen,
    /// Die Uhr des Aufrufers ist zurückgesprungen.
    ///
    /// ⚑ **Ein Fehler und keine Toleranz.** Wer eine zurückspringende
    /// Uhr durchgehen lässt, macht aus einer abgelaufenen Frist eine
    /// laufende, und genau das wäre der Angriff.
    UhrLaeuftRueckwaerts,
}

impl std::fmt::Display for Meldungsfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t = match self {
            Self::FremdeBehauptung => "Meldung gehoert zu einer anderen Behauptung",
            Self::DerGemeldeteSelbst => "der Gemeldete zeichnet seinen eigenen Ausfall gegen",
            Self::KeinMitglied => "der Melder gehoert diesem Pod nicht an",
            Self::SchonGezeichnet => "dieses Mitglied hat schon gezeichnet",
            Self::Unterschrift => "die Unterschrift stimmt nicht",
            Self::FristAbgelaufen => "die Frist ist verstrichen",
            Self::UhrLaeuftRueckwaerts => "die Uhr ist zurueckgesprungen",
        };
        f.write_str(t)
    }
}

impl std::error::Error for Meldungsfehler {}

/// Wie weit eine Sammlung ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stand {
    /// Es fehlen noch Unterschriften.
    NochNichtGenug {
        /// Wie viele gültige vorliegen.
        haben: usize,
        /// Wie viele nötig sind.
        brauchen: usize,
    },
    /// Genug Unterschriften: Die Nachbesetzung darf laufen.
    Beschlossen,
}

/// Der Nachweis, dass eine Nachbesetzung beschlossen ist.
///
/// # ⚑ Es gibt keinen öffentlichen Weg, einen zu bauen
///
/// Die Felder sind privat und der einzige Konstruktor ist
/// [`Meldungssammlung::beschluss`], der nur liefert, wenn genug gültige
/// Unterschriften innerhalb der Frist vorliegen. **Damit ist „ohne
/// Gegenzeichnung wird niemand verdrängt" eine Eigenschaft des Typs und
/// keine Regel, an die sich ein Aufrufer halten muss.**
///
/// Das ist dieselbe Bauart wie bei der Treasury ohne Schlüssel: Was von
/// der Konstruktion her unmöglich ist, muss niemand prüfen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Beschluss {
    behauptung: Ausfallbehauptung,
    zeichner: Vec<MinerId>,
}

impl Beschluss {
    /// Worüber beschlossen wurde.
    pub fn behauptung(&self) -> &Ausfallbehauptung {
        &self.behauptung
    }

    /// Wer gezeichnet hat, in kanonischer Ordnung.
    ///
    /// **Der Nachweis, der von einer Verdrängung übrig bleibt.** Namen,
    /// keine Zahl: Wer einen ehrlichen Knoten verdrängt, tut es
    /// nachweisbar, und eine Schiedsstelle kann die Liste lesen.
    pub fn zeichner(&self) -> &[MinerId] {
        &self.zeichner
    }
}

/// Sammelt die Gegenzeichnungen zu **einer** Behauptung.
#[derive(Debug, Clone)]
pub struct Meldungssammlung {
    behauptung: Ausfallbehauptung,
    mitglieder: BTreeSet<MinerId>,
    zeichner: BTreeSet<MinerId>,
    brauchen: usize,
    erste_ms: Option<u64>,
    zuletzt_ms: u64,
}

impl Meldungssammlung {
    /// Eröffnet eine Sammlung für eine Behauptung.
    ///
    /// `mitglieder` ist die Besetzung des Pods **einschließlich** des
    /// Gemeldeten und der Reserve: Wer im Pod sitzt, hat den Ausfall
    /// gesehen und darf bezeugen.
    pub fn neu(behauptung: Ausfallbehauptung, mitglieder: &[MinerId]) -> Self {
        let mitglieder: BTreeSet<MinerId> = mitglieder.iter().copied().collect();
        let brauchen = mindestzeichner(mitglieder.len());
        Self {
            behauptung,
            mitglieder,
            zeichner: BTreeSet::new(),
            brauchen,
            erste_ms: None,
            zuletzt_ms: 0,
        }
    }

    /// Nimmt eine Meldung auf und gibt den neuen Stand zurück.
    ///
    /// `jetzt_ms` kommt aus einer **monotonen** Uhr des Aufrufers.
    pub fn aufnehmen(
        &mut self,
        behauptung: &Ausfallbehauptung,
        meldung: &Meldung,
        pk: &BlsPublicKey,
        jetzt_ms: u64,
    ) -> Result<Stand, Meldungsfehler> {
        if *behauptung != self.behauptung {
            return Err(Meldungsfehler::FremdeBehauptung);
        }
        if meldung.melder == self.behauptung.gemeldeter {
            return Err(Meldungsfehler::DerGemeldeteSelbst);
        }
        if !self.mitglieder.contains(&meldung.melder) {
            return Err(Meldungsfehler::KeinMitglied);
        }
        if jetzt_ms < self.zuletzt_ms {
            return Err(Meldungsfehler::UhrLaeuftRueckwaerts);
        }
        if let Some(erste) = self.erste_ms {
            if jetzt_ms.saturating_sub(erste) > FRIST_MS {
                return Err(Meldungsfehler::FristAbgelaufen);
            }
        }
        if self.zeichner.contains(&meldung.melder) {
            return Err(Meldungsfehler::SchonGezeichnet);
        }
        // ⚑ Die Unterschrift zuletzt, weil sie das Teuerste ist. Alles
        // davor ist ein Vergleich; wer eine fremde Meldung schickt, soll
        // nicht eine Paarungsprüfung kosten.
        if !pk.verify(&self.behauptung.to_sign_bytes(), &meldung.signature) {
            return Err(Meldungsfehler::Unterschrift);
        }
        self.zuletzt_ms = jetzt_ms;
        self.erste_ms.get_or_insert(jetzt_ms);
        self.zeichner.insert(meldung.melder);
        Ok(self.stand())
    }

    /// Der aktuelle Stand, ohne etwas aufzunehmen.
    pub fn stand(&self) -> Stand {
        if self.zeichner.len() >= self.brauchen {
            Stand::Beschlossen
        } else {
            Stand::NochNichtGenug {
                haben: self.zeichner.len(),
                brauchen: self.brauchen,
            }
        }
    }

    /// Ob die Nachbesetzung laufen darf.
    pub fn beschlossen(&self) -> bool {
        matches!(self.stand(), Stand::Beschlossen)
    }

    /// Wer gezeichnet hat, in kanonischer Ordnung.
    ///
    /// **Das ist der Nachweis**, der von einer Verdrängung übrig bleibt:
    /// Namen, nicht eine Zahl.
    pub fn zeichner(&self) -> Vec<MinerId> {
        self.zeichner.iter().copied().collect()
    }

    /// Die Behauptung, um die es geht.
    pub fn behauptung(&self) -> &Ausfallbehauptung {
        &self.behauptung
    }

    /// Wie viele Unterschriften nötig sind.
    pub fn brauchen(&self) -> usize {
        self.brauchen
    }

    /// Der Beschluss, sobald genug gezeichnet haben.
    ///
    /// `None`, solange die Mehrheit fehlt. **Der einzige Weg zu einem
    /// [`Beschluss`]**, siehe dort.
    pub fn beschluss(&self) -> Option<Beschluss> {
        if !self.beschlossen() {
            return None;
        }
        Some(Beschluss {
            behauptung: self.behauptung,
            zeichner: self.zeichner(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein Mitglied: Schlüssel und die daraus abgeleitete Kennung.
    struct Mitglied {
        sk: BlsSecretKey,
        pk: BlsPublicKey,
        id: MinerId,
    }

    fn mitglied(b: u8) -> Mitglied {
        let sk = BlsSecretKey::key_gen(&[b.wrapping_add(1); 32]).expect("Schluessel");
        let pk = sk.public_key().expect("pk");
        let id = MinerId::aus_schluessel(&pk);
        Mitglied { sk, pk, id }
    }

    fn pod(n: u8) -> Vec<Mitglied> {
        (0..n).map(mitglied).collect()
    }

    fn behauptung(gemeldeter: MinerId) -> Ausfallbehauptung {
        Ausfallbehauptung {
            epoche: EpochId(7),
            pod: 3,
            position: 1,
            gemeldeter,
        }
    }

    fn meldung(m: &Mitglied, b: &Ausfallbehauptung) -> Meldung {
        Meldung {
            melder: m.id,
            signature: b.signieren(&m.sk).expect("Unterschrift"),
        }
    }

    /// Mehrheit der Übrigen, mit einer Untergrenze von zwei.
    #[test]
    fn die_schwelle_folgt_der_podgroesse() {
        assert_eq!(mindestzeichner(6), 3, "sechs Mitglieder, fuenf uebrig");
        assert_eq!(mindestzeichner(5), 3);
        assert_eq!(mindestzeichner(4), 2);
        assert_eq!(mindestzeichner(3), 2);
        // ⚑ Nie einer, auch wenn die Rechnung eins ergäbe.
        assert_eq!(mindestzeichner(2), MINDESTENS_ZEICHNER);
        assert_eq!(mindestzeichner(1), MINDESTENS_ZEICHNER);
        assert_eq!(mindestzeichner(0), MINDESTENS_ZEICHNER);
    }

    /// ⚑ **Der Kern: Einer allein wirft niemanden heraus.**
    #[test]
    fn ein_einzelner_melder_reicht_nicht() {
        let p = pod(6);
        let b = behauptung(p[1].id);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);

        let stand = s
            .aufnehmen(&b, &meldung(&p[0], &b), &p[0].pk, 1_000)
            .expect("Aufnahme");
        assert_eq!(stand, Stand::NochNichtGenug { haben: 1, brauchen: 3 });
        assert!(!s.beschlossen());
    }

    /// Mit der Mehrheit ist es beschlossen, und die Zeichner stehen mit
    /// Namen da.
    #[test]
    fn die_mehrheit_beschliesst_und_hinterlaesst_namen() {
        let p = pod(6);
        let b = behauptung(p[1].id);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);

        for m in [&p[0], &p[2], &p[3]] {
            s.aufnehmen(&b, &meldung(m, &b), &m.pk, 1_000).expect("Aufnahme");
        }
        assert!(s.beschlossen());
        assert_eq!(s.zeichner().len(), 3);
        for m in [&p[0], &p[2], &p[3]] {
            assert!(s.zeichner().contains(&m.id), "der Zeichner fehlt im Nachweis");
        }
    }

    /// ⚑ Der Gemeldete zeichnet seinen eigenen Ausfall nicht gegen.
    #[test]
    fn der_gemeldete_zeichnet_nicht_mit() {
        let p = pod(6);
        let b = behauptung(p[1].id);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);
        assert_eq!(
            s.aufnehmen(&b, &meldung(&p[1], &b), &p[1].pk, 1_000),
            Err(Meldungsfehler::DerGemeldeteSelbst)
        );
    }

    /// Wer nicht im Pod sitzt, bezeugt nichts.
    #[test]
    fn ein_fremder_bezeugt_nichts() {
        let p = pod(6);
        let fremder = mitglied(90);
        let b = behauptung(p[1].id);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);
        assert_eq!(
            s.aufnehmen(&b, &meldung(&fremder, &b), &fremder.pk, 1_000),
            Err(Meldungsfehler::KeinMitglied)
        );
    }

    /// Zweimal dieselbe Unterschrift zählt einmal.
    #[test]
    fn zweimal_derselbe_zeichner_zaehlt_einmal() {
        let p = pod(6);
        let b = behauptung(p[1].id);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);
        s.aufnehmen(&b, &meldung(&p[0], &b), &p[0].pk, 1_000).expect("erste");
        assert_eq!(
            s.aufnehmen(&b, &meldung(&p[0], &b), &p[0].pk, 1_100),
            Err(Meldungsfehler::SchonGezeichnet)
        );
        assert_eq!(s.stand(), Stand::NochNichtGenug { haben: 1, brauchen: 3 });
    }

    /// Eine gefälschte Unterschrift zählt nicht.
    #[test]
    fn eine_fremde_unterschrift_zaehlt_nicht() {
        let p = pod(6);
        let b = behauptung(p[1].id);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);
        // p[0] meldet, aber die Unterschrift stammt von p[2].
        let falsch = Meldung {
            melder: p[0].id,
            signature: b.signieren(&p[2].sk).expect("sig"),
        };
        assert_eq!(
            s.aufnehmen(&b, &falsch, &p[0].pk, 1_000),
            Err(Meldungsfehler::Unterschrift)
        );
    }

    /// ⚑ **Die Meldung über den Nachrücker ist eine andere Aussage.**
    /// Genau das war die Lücke: Ohne den Namen des Gemeldeten zog
    /// dieselbe Meldung nach jeder Übernahme den nächsten Vorrat.
    #[test]
    fn eine_meldung_ueber_den_nachruecker_ist_eine_andere() {
        let p = pod(6);
        let b_alt = behauptung(p[1].id);
        let b_neu = behauptung(p[4].id);
        assert_ne!(b_alt, b_neu);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b_alt, &ids);
        assert_eq!(
            s.aufnehmen(&b_neu, &meldung(&p[0], &b_neu), &p[0].pk, 1_000),
            Err(Meldungsfehler::FremdeBehauptung),
            "die zweite Aussage lief in die erste Sammlung"
        );
    }

    /// Eine Unterschrift über eine andere Epoche gilt hier nicht.
    #[test]
    fn eine_unterschrift_aus_einer_anderen_epoche_gilt_nicht() {
        let p = pod(6);
        let b = behauptung(p[1].id);
        let mut vorige = b;
        vorige.epoche = EpochId(6);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);
        let alt = Meldung {
            melder: p[0].id,
            signature: vorige.signieren(&p[0].sk).expect("sig"),
        };
        assert_eq!(
            s.aufnehmen(&b, &alt, &p[0].pk, 1_000),
            Err(Meldungsfehler::Unterschrift)
        );
    }

    /// ⚑ Nach der Frist zählt nichts mehr.
    #[test]
    fn nach_der_frist_zaehlt_nichts_mehr() {
        let p = pod(6);
        let b = behauptung(p[1].id);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);
        s.aufnehmen(&b, &meldung(&p[0], &b), &p[0].pk, 1_000).expect("erste");
        s.aufnehmen(&b, &meldung(&p[2], &b), &p[2].pk, 1_000 + FRIST_MS)
            .expect("noch in der Frist");
        assert_eq!(
            s.aufnehmen(&b, &meldung(&p[3], &b), &p[3].pk, 1_001 + FRIST_MS),
            Err(Meldungsfehler::FristAbgelaufen)
        );
        assert!(!s.beschlossen(), "die Frist hat die Mehrheit nicht verhindert");
    }

    /// ⚑ Eine zurückspringende Uhr ist ein Fehler, keine Toleranz.
    #[test]
    fn eine_rueckwaerts_laufende_uhr_wird_abgewiesen() {
        let p = pod(6);
        let b = behauptung(p[1].id);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);
        s.aufnehmen(&b, &meldung(&p[0], &b), &p[0].pk, 10_000).expect("erste");
        assert_eq!(
            s.aufnehmen(&b, &meldung(&p[2], &b), &p[2].pk, 9_999),
            Err(Meldungsfehler::UhrLaeuftRueckwaerts)
        );
    }

    /// In einem Pod, der zu klein für zwei Zeichner ist, wird niemand
    /// verdrängt. Fehlschlag in die sichere Richtung.
    #[test]
    fn ein_zu_kleiner_pod_verdraengt_niemanden() {
        let p = pod(2);
        let b = behauptung(p[1].id);
        let ids: Vec<MinerId> = p.iter().map(|m| m.id).collect();
        let mut s = Meldungssammlung::neu(b, &ids);
        s.aufnehmen(&b, &meldung(&p[0], &b), &p[0].pk, 1_000).expect("erste");
        // Nur ein möglicher Zeichner, gebraucht werden zwei.
        assert!(!s.beschlossen());
        assert_eq!(s.brauchen(), MINDESTENS_ZEICHNER);
    }
}
