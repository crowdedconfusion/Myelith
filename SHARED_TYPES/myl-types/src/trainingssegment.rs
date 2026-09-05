//! Das **Trainingssegment**: die zweite Arbeitsklasse (TRAINING 2.1,
//! Whitepaper Kap. 6.1 und 6.6 sinngemäss).
//!
//! # ⚑ Warum es keinen neuen Verifikationsmechanismus braucht
//!
//! Ein Inferenzsegment ist verifizierbar, weil es eine **reine Funktion**
//! seiner Eingabe ist: Zwei Miner mit denselben Angaben müssen dieselbe
//! Ausgabe liefern, und der Vergleich ist ein Hashvergleich. Genau
//! dasselbe gilt hier, mit anderer Eingabe und anderer Ausgabe:
//!
//! | | Eingabe | Ausgabe |
//! |---|---|---|
//! | Inferenz | Prompt-Stück, KV-Wurzel, θ_v | Aktivierungsspur, Ergebnis |
//! | **Training** | θ_v, Charge, Startschritt, Schrittzahl, Lernrate | **Δm je Gewicht** |
//!
//! Die Streitfallauflösung überträgt sich unverändert: Weichen zwei Pods
//! ab, wird bisektioniert, bis der erste abweichende Schritt feststeht,
//! und dieser eine Schritt wird nachgerechnet.
//!
//! ⚑ **Und dass er nachrechenbar ist, ist keine Behauptung mehr.**
//! `integer_llm_runtime::trainingsschleife` rechnet ihn, für eine dichte
//! Ebene und für ein Expertengemisch, und liefert einen Abdruck. Bis zum
//! 2026-09-04 gab es nichts, das ein Δm erzeugt; ein Typ dafür wäre eine
//! Kiste ohne Aufrufer gewesen, und das ist die Fehlerklasse, die dieses
//! Projekt viermal getroffen hat.
//!
//! # ⚑ Was dieser Typ NICHT tut, und warum das Absicht ist
//!
//! **Er geht nicht in die Kette.** Es gibt keine Anweisung, die ein
//! Trainingsbündel einreicht, und das ist kein Versehen: Ein Bündel
//! entsteht, wenn ein Pod Trainingsarbeit **über das Netz** geleistet
//! hat, und diesen Weg gibt es noch nicht (TRAINING 2.2 und weiter, und
//! in COMPUTE_PIPELINE steht keine Zeile Trainingscode). Eine Anweisung
//! ohne Aufrufer wäre genau der Fehler, den dieser Modulkopf zwei Absätze
//! weiter oben beschreibt.
//!
//! **Und der Streitfall hat keine Zähne.** `create_slash_decision` und
//! `apply_verdict` haben ausserhalb ihrer Tests keinen Aufrufer; der Weg
//! von einem Streitfall in einen Block fehlt für die Inferenz genauso.
//! Dieser Typ erbt die Mechanik, nicht ihre Durchsetzung.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::bls::BlsSignature;
use crate::hash::Hash;
use crate::ids::{MerkleRoot, MinerId, SegmentId};

/// Was ein Trainingssegment beansprucht.
///
/// **Feldnamen und Feldreihenfolge sind Konsens-Vertrag**, aus demselben
/// Grund wie bei [`crate::Segment`]: Borsh serialisiert in
/// Deklarationsreihenfolge, und über dieser Struktur hängen Commitments.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Trainingssegment {
    /// Die Kennung des Segments.
    pub id: SegmentId,
    /// Die Modellfassung, von der aus gerechnet wurde: θ_v samt
    /// Ausführungsspezifikation.
    ///
    /// ⚑ **Der Startzustand und nicht nur die Formatversion.** Ein Δm
    /// ist eine Differenz, und eine Differenz ohne ihren Ausgangspunkt
    /// ist keine Angabe. Zwei Miner, die von verschiedenen Gewichten
    /// starten, liefern verschiedene Δm und hätten beide recht.
    pub modell_version: MerkleRoot,
    /// Commitment über die Charge: welche Daten in welcher Reihenfolge.
    ///
    /// ⚑ **Undurchsichtig und mit Absicht.** Wer welche Charge bekommt,
    /// ist eine eigene Frage (TRAINING 2.2 und die Datenprovenienz);
    /// hier zählt nur, dass sie **festgelegt** ist. Dasselbe Verhältnis
    /// wie `input_commitment` beim Inferenzsegment.
    pub charge: Hash,
    /// Der erste Schritt dieses Segments.
    ///
    /// ⚑ Er geht in den Würfel des stochastischen Rundens ein
    /// (`optimierer::Schrittkennung`), ist also **Teil der Rechnung**
    /// und keine Buchhaltung. Zwei Segmente mit demselben Startschritt
    /// auf derselben Ebene rundeten identisch.
    pub startschritt: u64,
    /// Wie viele Schritte.
    pub schrittzahl: u32,
    /// Zähler der Lernrate.
    pub lr_zaehler: i64,
    /// Nenner der Lernrate.
    ///
    /// # ⚑ Warum die Lernrate im Segment steht und nicht im Protokoll
    ///
    /// Sie muss **committet** sein, sonst ist das Segment keine reine
    /// Funktion: Ein Miner, der mit einer anderen Lernrate rechnet,
    /// liefert ein anderes Δm, und der Nachrechner müsste raten, welche
    /// gemeint war. Hier steht sie, also ist sie gebunden.
    ///
    /// ⚑ **Eine harte Schranke steht bewusst NICHT dabei**, und der
    /// Grund ist eine Messung vom 2026-09-05. Ein zu grosser Schritt auf
    /// einem Router war bis dahin **nicht behebbar**: Der Ganzzahl-
    /// Softmax sättigte, und ein gesättigter Router hat überall den
    /// Gradienten null. Das wäre ein Argument für eine Protokollschranke
    /// gewesen. Der Boden in `moe::route_top_k` (θ_v 0.18.0) hat den
    /// Zustand **entfernt**; ein zu grosser Schritt verschwendet seither
    /// Arbeit, statt einen Router zu töten, und Verschwendung braucht
    /// keine Konsensregel.
    ///
    /// Was bleibt, ist die Prüfung auf Sinn: siehe [`Trainingssegment::pruefen`].
    pub lr_nenner: i64,
    /// Commitment über **Δm**, das Ergebnis.
    ///
    /// # ⚑ Über die Differenz und nicht über den Endzustand
    ///
    /// Der naheliegende Griff wäre ein Abdruck über die
    /// fortgeschriebenen Gewichte, und genau den liefert
    /// `optimierer::trainingsabdruck`. **Er hätte hier die falsche
    /// Form.** Die Aggregation lautet
    ///
    /// ```text
    /// m_{v+1} = klemmen(m_v + Σ Δm_i)
    /// ```
    ///
    /// Viele Miner rechnen **gegen denselben Ausgangszustand**, und ihre
    /// Ergebnisse addieren sich. Ein Commitment über Endzustände addiert
    /// sich nicht: Zwei Miner, die verschiedene Chargen rechnen, hätten
    /// zwei verschiedene Endzustände, und keiner davon ist der, den die
    /// Kette am Ende trägt.
    ///
    /// **Der Abdruck über den Endzustand bleibt richtig für das, wofür
    /// er gebaut wurde:** zwei Maschinen, die **dieselbe** Arbeit
    /// rechnen und ihr Ergebnis vergleichen. Das ist die Frage des
    /// Miettags, nicht die der Aggregation.
    pub delta_commitment: Hash,
    /// Die Miner, die gerechnet haben, in Pipeline-Reihenfolge.
    pub pod_pfad: Vec<MinerId>,
    /// Eine BLS-Signatur je Übergang.
    pub signaturen: Vec<BlsSignature>,
}

/// Was an einem Trainingssegment nicht stimmt.
///
/// ⚑ **`Copy`, weil `myl_ledger::TransitionError` es traegt** und selbst
/// `Copy` ist (2026-09-05). Alle Varianten sind Zahlen; ein Fehler, der
/// eine Allokation braeuchte, gehoerte ohnehin nicht in einen
/// Uebergangsfehler, der in jedem Block millionenfach entstehen kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segmentfehler {
    /// Ein Segment ohne Schritte hat nichts gerechnet.
    OhneSchritte,
    /// Ein Nenner von null ist keine Lernrate.
    LernrateOhneNenner,
    /// Ein Zähler von null bewegt kein Gewicht.
    LernrateOhneZaehler,
    /// Eine negative Lernrate stiege den Verlust hinauf.
    ///
    /// ⚑ **Kein Geschmacksurteil.** Der Schritt ist
    /// `m − g·lr`; mit negativem `lr` ist er ein Aufstieg, und ein Pod,
    /// der das beansprucht, beansprucht Arbeit, die das Modell
    /// verschlechtert.
    LernrateNegativ,
    /// Ein Pod ohne Mitglieder hat nicht gerechnet.
    OhnePod,
    /// Je Übergang eine Signatur, sonst ist unklar, wer was bezeugt.
    SignaturenPassenNicht { pfad: usize, signaturen: usize },
}

impl std::fmt::Display for Segmentfehler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OhneSchritte => f.write_str("schrittzahl ist null"),
            Self::LernrateOhneNenner => f.write_str("lr_nenner ist null"),
            Self::LernrateOhneZaehler => f.write_str("lr_zaehler ist null"),
            Self::LernrateNegativ => {
                f.write_str("die Lernrate ist negativ: das waere ein Aufstieg, kein Schritt")
            }
            Self::OhnePod => f.write_str("pod_pfad ist leer"),
            Self::SignaturenPassenNicht { pfad, signaturen } => write!(
                f,
                "{pfad} Miner im Pfad, aber {signaturen} Signaturen"
            ),
        }
    }
}

impl std::error::Error for Segmentfehler {}

impl Trainingssegment {
    /// Prüft, was sich ohne Nachrechnen prüfen lässt.
    ///
    /// ⚑ **Das ist kein Verifikationsersatz.** Ob das Δm stimmt, sagt
    /// nur das Nachrechnen. Hier fällt weg, was schon an der Form
    /// erkennbar unmöglich ist, damit der teure Weg nicht für Unsinn
    /// bezahlt wird.
    pub fn pruefen(&self) -> Result<(), Segmentfehler> {
        if self.schrittzahl == 0 {
            return Err(Segmentfehler::OhneSchritte);
        }
        if self.lr_nenner == 0 {
            return Err(Segmentfehler::LernrateOhneNenner);
        }
        if self.lr_zaehler == 0 {
            return Err(Segmentfehler::LernrateOhneZaehler);
        }
        if (self.lr_zaehler < 0) != (self.lr_nenner < 0) {
            return Err(Segmentfehler::LernrateNegativ);
        }
        if self.pod_pfad.is_empty() {
            return Err(Segmentfehler::OhnePod);
        }
        if self.signaturen.len() != self.pod_pfad.len() {
            return Err(Segmentfehler::SignaturenPassenNicht {
                pfad: self.pod_pfad.len(),
                signaturen: self.signaturen.len(),
            });
        }
        Ok(())
    }

    /// Die Botschaft, die ein Pod unterschreibt.
    ///
    /// # ⚑ Was darin steht, und warum jedes Feld
    ///
    /// **Alles, was das Ergebnis bestimmt, und die Signaturen selbst
    /// nicht.** Wer ein Feld herausliesse, das die Rechnung beeinflusst,
    /// liesse den Koordinator es nach dem Einsammeln der Unterschriften
    /// ändern: dieselbe Lücke wie Fund 115, wo `num_segments` nicht in
    /// der Botschaft stand und der Koordinator damit die
    /// Stichprobenwahrscheinlichkeit verdünnen konnte.
    ///
    /// ⚑ **Der `pod_pfad` steht mit drin**, obwohl er das Ergebnis nicht
    /// ändert: Er sagt, **wer** haftet. Ohne ihn liesse sich ein
    /// bezeugtes Segment einem anderen Pod unterschieben.
    pub fn botschaft(&self) -> [u8; 32] {
        let mut roh: Vec<u8> = Vec::new();
        roh.extend_from_slice(b"myl-trainingssegment-v1");
        roh.extend_from_slice(self.id.as_bytes());
        roh.extend_from_slice(self.modell_version.as_bytes());
        roh.extend_from_slice(self.charge.as_bytes());
        roh.extend_from_slice(&self.startschritt.to_le_bytes());
        roh.extend_from_slice(&self.schrittzahl.to_le_bytes());
        roh.extend_from_slice(&self.lr_zaehler.to_le_bytes());
        roh.extend_from_slice(&self.lr_nenner.to_le_bytes());
        roh.extend_from_slice(self.delta_commitment.as_bytes());
        // ⚑ **Die Länge vor der Liste.** Ohne sie liessen sich zwei
        // verschiedene Pfade zu derselben Bytefolge zusammensetzen;
        // dieselbe Überlegung wie beim Trainingsabdruck.
        roh.extend_from_slice(&(self.pod_pfad.len() as u64).to_le_bytes());
        for m in &self.pod_pfad {
            roh.extend_from_slice(m.as_bytes());
        }
        *Hash::sha256(&roh).as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment() -> Trainingssegment {
        Trainingssegment {
            id: SegmentId::new([7u8; 32]),
            modell_version: MerkleRoot::new([1u8; 32]),
            charge: Hash::from_bytes([2u8; 32]),
            startschritt: 40,
            schrittzahl: 8,
            lr_zaehler: 1,
            lr_nenner: 4096,
            delta_commitment: Hash::from_bytes([3u8; 32]),
            pod_pfad: vec![MinerId::new([9u8; 32]), MinerId::new([10u8; 32])],
            signaturen: vec![BlsSignature([0u8; 96]), BlsSignature([1u8; 96])],
        }
    }

    #[test]
    fn ein_gesundes_segment_besteht() {
        segment().pruefen().expect("gesund");
    }

    /// ⚑ **Ein Segment ohne Schritte hat nichts gerechnet.**
    #[test]
    fn ohne_schritte_faellt_es_durch() {
        let mut s = segment();
        s.schrittzahl = 0;
        assert_eq!(s.pruefen(), Err(Segmentfehler::OhneSchritte));
    }

    /// ⚑ **Eine negative Lernrate wäre ein Aufstieg**, und ein Pod, der
    /// das beansprucht, beansprucht Arbeit, die das Modell
    /// verschlechtert.
    #[test]
    fn eine_negative_lernrate_faellt_durch() {
        let mut s = segment();
        s.lr_zaehler = -1;
        assert_eq!(s.pruefen(), Err(Segmentfehler::LernrateNegativ));
        // Und andersherum genauso.
        let mut s = segment();
        s.lr_nenner = -4096;
        assert_eq!(s.pruefen(), Err(Segmentfehler::LernrateNegativ));
        // ⛑ Zwei negative Vorzeichen sind eine positive Lernrate, und
        // die ist erlaubt: Sonst prüfte der Test das Vorzeichen eines
        // Feldes statt das der Rate.
        let mut s = segment();
        s.lr_zaehler = -1;
        s.lr_nenner = -4096;
        s.pruefen().expect("minus durch minus ist plus");
    }

    #[test]
    fn eine_lernrate_ohne_nenner_faellt_durch() {
        let mut s = segment();
        s.lr_nenner = 0;
        assert_eq!(s.pruefen(), Err(Segmentfehler::LernrateOhneNenner));
    }

    /// ⚑ **Je Übergang eine Signatur**, sonst ist unklar, wer was
    /// bezeugt.
    #[test]
    fn ungleich_viele_signaturen_fallen_durch() {
        let mut s = segment();
        s.signaturen.pop();
        assert_eq!(
            s.pruefen(),
            Err(Segmentfehler::SignaturenPassenNicht { pfad: 2, signaturen: 1 })
        );
    }

    /// ⚑ **Jedes Feld, das die Rechnung bestimmt, geht in die
    /// Botschaft.**
    ///
    /// Die Gegenprobe zu Fund 115: Dort stand `num_segments` **nicht**
    /// in der Signierbotschaft, und der Koordinator konnte es nach dem
    /// Einsammeln der Unterschriften ändern. Dieser Test ändert jedes
    /// Feld einzeln und verlangt eine andere Botschaft.
    #[test]
    fn jedes_feld_bewegt_die_botschaft() {
        let s = segment();
        let b = s.botschaft();

        let mut aenderungen: Vec<(&str, Trainingssegment)> = Vec::new();
        let mut x = s.clone();
        x.id = SegmentId::new([8u8; 32]);
        aenderungen.push(("id", x));
        let mut x = s.clone();
        x.modell_version = MerkleRoot::new([4u8; 32]);
        aenderungen.push(("modell_version", x));
        let mut x = s.clone();
        x.charge = Hash::from_bytes([5u8; 32]);
        aenderungen.push(("charge", x));
        let mut x = s.clone();
        x.startschritt = 41;
        aenderungen.push(("startschritt", x));
        let mut x = s.clone();
        x.schrittzahl = 9;
        aenderungen.push(("schrittzahl", x));
        let mut x = s.clone();
        x.lr_zaehler = 2;
        aenderungen.push(("lr_zaehler", x));
        let mut x = s.clone();
        x.lr_nenner = 2048;
        aenderungen.push(("lr_nenner", x));
        let mut x = s.clone();
        x.delta_commitment = Hash::from_bytes([6u8; 32]);
        aenderungen.push(("delta_commitment", x));
        let mut x = s.clone();
        x.pod_pfad = vec![MinerId::new([9u8; 32])];
        aenderungen.push(("pod_pfad", x));

        for (name, geaendert) in aenderungen {
            assert_ne!(
                geaendert.botschaft(),
                b,
                "{name} steht nicht in der Botschaft: der Koordinator koennte es \
                 nach dem Einsammeln der Unterschriften aendern"
            );
        }
    }

    /// ⚑ **Die Signaturen selbst gehen NICHT ein**, sonst könnte
    /// niemand unterschreiben.
    #[test]
    fn die_signaturen_bewegen_die_botschaft_nicht() {
        let s = segment();
        let mut x = s.clone();
        x.signaturen = vec![BlsSignature([9u8; 96]); 2];
        assert_eq!(x.botschaft(), s.botschaft());
    }

    /// ⚑ **Zwei Pfade, die sich nur in der Aufteilung unterscheiden,
    /// sind zwei Botschaften.**
    ///
    /// Ohne die Längenangabe vor der Liste liessen sich verschiedene
    /// Pfade zu derselben Bytefolge zusammensetzen.
    #[test]
    fn die_laenge_des_pfades_geht_ein() {
        let mut a = segment();
        a.pod_pfad = vec![MinerId::new([1u8; 32]), MinerId::new([2u8; 32])];
        a.signaturen = vec![BlsSignature([0u8; 96]); 2];
        let mut b = segment();
        b.pod_pfad = vec![MinerId::new([1u8; 32])];
        b.signaturen = vec![BlsSignature([0u8; 96])];
        assert_ne!(a.botschaft(), b.botschaft());
    }

    /// Zweimal gerechnet ist zweimal dasselbe.
    #[test]
    fn die_botschaft_ist_deterministisch() {
        assert_eq!(segment().botschaft(), segment().botschaft());
    }

    /// Borsh hin und zurück.
    #[test]
    fn das_segment_ueberlebt_die_serialisierung() {
        let s = segment();
        let bytes = borsh::to_vec(&s).expect("serialisiert");
        let zurueck: Trainingssegment =
            borsh::from_slice(&bytes).expect("deserialisiert");
        assert_eq!(zurueck, s);
    }
}
