//! Der Shard entsiegelt und prüft die Bindung (GATEWAY Stufe 4).
//!
//! # ⚑ Der Koordinator ist ein Fremder, und deshalb geht das überhaupt
//!
//! Der Prompt reist versiegelt. Wer ihn weiterleitet, sieht ihn nicht;
//! der Empfänger entsiegelt ihn und **kann die Bindung dann selbst
//! prüfen**, weil sie den Klartext bindet, den er nun in Händen hält.
//!
//! **Beides zusammen ist die Aussage:** Der Weg trägt keinen Klartext,
//! und der Empfänger kann trotzdem feststellen, ob er das bekommen hat,
//! was der Auftrag behauptet. Ohne die Prüfung rechnete er etwas, und
//! niemand könnte später zeigen, dass genau diese Anfrage es ausgelöst
//! hat.
//!
//! # ⚑ Drei Tore, und ihre Reihenfolge ist die Aussage
//!
//! 1. **Die Form** (schon in [`crate::ortsdienst`], vor dem Aufruf
//!    hierher): Ein Auftrag, der die Deckel verletzt, soll keine
//!    Entsiegelung kosten.
//! 2. **Das Entsiegeln**: kostet eine KEM-Dekapselung und eine
//!    AEAD-Öffnung, also Rechenzeit, aber beschränkte.
//! 3. **Die Bindung**: ein SHA-256 über den Klartext. **Erst danach
//!    sieht das Rechenwerk irgendetwas.**
//!
//! Wer das Rechnen vor die Bindung stellte, liesse jeden, der einen
//! Kanal aufbauen darf, den Pod beliebig rechnen lassen.
//!
//! # Was hier bewusst nicht steht
//!
//! **Woher die Gegenstelle kommt.** Der Shard muss wissen, wessen
//! Kapselpunkt zu dieser Sitzung gehört; das ist die Zuteilung, und sie
//! kommt aus der Kette. Hier steht ein [`Gegenstellen`]-Merkmal und
//! keine Nachbildung: Eine zweite Quelle für die Zuteilung wäre eine
//! zweite Wahrheit darüber, wer zu einem Pod gehört.

use std::sync::Mutex;

use myl_siegel::{Endpunkt, Gegenpunkte, Sitzungen, Umschlag};
use myl_types::hash::Hash;
use myl_types::ids::PodId;
use myl_types::inferenzauftrag::{Inferenzantwort, Inferenzauftrag};

use crate::ortsdienst::Rechenwerk;

/// Woher der Shard erfährt, wer die Gegenstelle einer Sitzung ist.
///
/// ⚑ **Ein Merkmal und keine Nachbildung.** Die Antwort steht in der
/// Kette, und `myl-pod` kennt die Kette nicht. Wer hier eine eigene
/// Tabelle führte, führte eine zweite Wahrheit darüber, wer zu einem
/// Pod gehört.
pub trait Gegenstellen: Send + Sync {
    /// Endpunkt und angekündigte Punkte der Gegenstelle dieser Sitzung.
    ///
    /// `None` heisst: Diese Sitzung gehört nicht hierher.
    fn nachschlagen(&self, sitzung: u64) -> Option<(Endpunkt, Gegenpunkte)>;
}

/// Was das Rechenwerk sieht, nachdem entsiegelt und geprüft wurde.
///
/// ⚑ **Es bekommt den Klartext und nie die Bytes von der Leitung.** Wer
/// beides reichte, machte es möglich, die Prüfung zu umgehen, ohne dass
/// es auffiele.
pub trait Klartextwerk: Send + Sync {
    /// Rechnet einen entsiegelten und gebundenen Auftrag.
    fn rechne(&self, auftrag: &Inferenzauftrag, prompt: &[u8]) -> Inferenzantwort;
    /// Für welchen Pipeline-Stand dieser Prozess geladen ist.
    fn pipeline(&self) -> Hash;
    /// Wie viele Shards die Pipeline hat.
    fn shards(&self) -> u32;
}

/// Der eigene Endpunkt und die angekündigten Punkte, als rohe Bytes.
///
/// Frei stehend, damit auch ein Test sie bilden kann, ohne
/// [`Entsiegelndes`] zu bauen.
pub fn eigene_punkte(sitzungen: &Sitzungen, ich: Endpunkt) -> ([u8; 32], [u8; 32], Vec<u8>) {
    (
        *ich.bytes(),
        *sitzungen.punkt().bytes(),
        sitzungen.kapselpunkt().bytes().to_vec(),
    )
}

/// Warum ein Auftrag nicht gerechnet wurde.
///
/// ⚑ **Nur für das eigene Protokoll.** Nach aussen geht in jedem Fall
/// dieselbe Ablehnung ohne Grund: Ein Pod, der begründet, verrät seinen
/// Zustand und wird zum Auskunftsdienst über fremde Sitzungen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abweisungsgrund {
    /// Die Sitzung ist diesem Pod nicht zugeteilt.
    FremdeSitzung,
    /// Der Umschlag ging nicht auf.
    NichtEntsiegelbar,
    /// Der Klartext passt nicht zur Bindung des Auftrags.
    BindungPasstNicht,
    /// Der Auftrag gilt für einen anderen Pipeline-Stand.
    FremdePipeline,
}

/// Ein Rechenwerk, das vor dem Rechnen entsiegelt und prüft.
pub struct Entsiegelndes {
    pod: PodId,
    ich: Endpunkt,
    sitzungen: Mutex<Sitzungen>,
    gegenstellen: Box<dyn Gegenstellen>,
    werk: Box<dyn Klartextwerk>,
    /// Die Gründe der letzten Abweisungen, für das Betriebsprotokoll.
    letzter_grund: Mutex<Option<Abweisungsgrund>>,
}

impl Entsiegelndes {
    /// Setzt Zuteilung, Sitzungen und Rechenwerk zusammen.
    pub fn neu(
        pod: PodId,
        ich: Endpunkt,
        sitzungen: Sitzungen,
        gegenstellen: Box<dyn Gegenstellen>,
        werk: Box<dyn Klartextwerk>,
    ) -> Self {
        Self {
            pod,
            ich,
            sitzungen: Mutex::new(sitzungen),
            gegenstellen,
            werk,
            letzter_grund: Mutex::new(None),
        }
    }

    /// Warum der letzte Auftrag abgewiesen wurde, falls einer es wurde.
    pub fn letzter_grund(&self) -> Option<Abweisungsgrund> {
        self.letzter_grund.lock().ok().and_then(|g| *g)
    }

    fn abweisen(&self, grund: Abweisungsgrund, sitzung: u64) -> Inferenzantwort {
        if let Ok(mut g) = self.letzter_grund.lock() {
            *g = Some(grund);
        }
        Inferenzantwort::Abgelehnt { sitzung }
    }
}

impl Rechenwerk for Entsiegelndes {
    fn rechne(&self, auftrag: &Inferenzauftrag) -> Inferenzantwort {
        // ⚑ **Der billigste Vergleich zuerst.** Ein Auftrag für einen
        // fremden Pipeline-Stand kostet hier einen Hashvergleich; nach
        // dem Entsiegeln hätte er eine KEM-Dekapselung gekostet.
        if auftrag.pipeline != self.werk.pipeline() {
            return self.abweisen(Abweisungsgrund::FremdePipeline, auftrag.sitzung);
        }
        let Some((gegenstelle, gegenpunkte)) = self.gegenstellen.nachschlagen(auftrag.sitzung)
        else {
            return self.abweisen(Abweisungsgrund::FremdeSitzung, auftrag.sitzung);
        };
        let klartext = {
            // **Eine vergiftete Sperre weist ab und reisst den Prozess
            // nicht mit.** Dieselbe Haltung wie bei der Beobachtung im
            // Knoten (Fund 128).
            let Ok(mut sitzungen) = self.sitzungen.lock() else {
                return self.abweisen(Abweisungsgrund::NichtEntsiegelbar, auftrag.sitzung);
            };
            match Umschlag::oeffnen(
                &mut sitzungen,
                self.pod,
                gegenstelle,
                &gegenpunkte,
                &auftrag.prompt_versiegelt,
            ) {
                Ok(k) => k,
                Err(_) => {
                    return self.abweisen(Abweisungsgrund::NichtEntsiegelbar, auftrag.sitzung)
                }
            }
        };
        // ⚑ **Und erst jetzt ist die Bindung prüfbar.** Vorher band sie
        // etwas, das dieser Prozess nicht sehen konnte.
        if !auftrag.bindung.passt_zu_sitzung(auftrag.sitzung, &klartext) {
            return self.abweisen(Abweisungsgrund::BindungPasstNicht, auftrag.sitzung);
        }
        self.werk.rechne(auftrag, &klartext)
    }

    fn pipeline(&self) -> Hash {
        self.werk.pipeline()
    }

    fn shards(&self) -> u32 {
        self.werk.shards()
    }

    fn gegenstelle(&self) -> Option<([u8; 32], [u8; 32], Vec<u8>)> {
        let sitzungen = self.sitzungen.lock().ok()?;
        Some(eigene_punkte(&sitzungen, self.ich))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use myl_siegel::Epochenschluessel;
    use myl_types::ids::{EpochId, SegmentId};
    use myl_types::sitzung::Anfragebindung;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    const POD: [u8; 32] = [9u8; 32];

    fn endpunkt(n: u8) -> Endpunkt {
        Endpunkt::aus_bytes([n; 32])
    }

    /// Ein Verzeichnis mit genau einer Sitzung.
    struct EineSitzung {
        sitzung: u64,
        wer: Endpunkt,
        punkte: Gegenpunkte,
    }

    impl Gegenstellen for EineSitzung {
        fn nachschlagen(&self, sitzung: u64) -> Option<(Endpunkt, Gegenpunkte)> {
            (sitzung == self.sitzung).then(|| (self.wer, self.punkte.clone()))
        }
    }

    struct Zaehlwerk {
        gesehen: Arc<AtomicUsize>,
        klartext: Arc<Mutex<Vec<u8>>>,
    }

    impl Klartextwerk for Zaehlwerk {
        fn rechne(&self, auftrag: &Inferenzauftrag, prompt: &[u8]) -> Inferenzantwort {
            self.gesehen.fetch_add(1, Ordering::SeqCst);
            if let Ok(mut k) = self.klartext.lock() {
                *k = prompt.to_vec();
            }
            Inferenzantwort::Ergebnis {
                sitzung: auftrag.sitzung,
                token: vec![1, 2, 3],
                segment: SegmentId::new([4; 32]),
                prompt_token: prompt.len() as u64,
                text: String::from_utf8_lossy(prompt).to_string(),
            }
        }
        fn pipeline(&self) -> Hash {
            Hash::sha256(b"probe-pipeline")
        }
        fn shards(&self) -> u32 {
            4
        }
    }

    struct Aufbau {
        werk: Entsiegelndes,
        gesehen: Arc<AtomicUsize>,
        klartext: Arc<Mutex<Vec<u8>>>,
        fragender: Sitzungen,
        shard_punkte: Gegenpunkte,
    }

    fn aufbau(sitzung: u64) -> Aufbau {
        let fragender_schluessel = Epochenschluessel::probe(EpochId(1), [1u8; 32]);
        let shard_schluessel = Epochenschluessel::probe(EpochId(1), [2u8; 32]);
        let fragender_punkte = Gegenpunkte {
            punkt: fragender_schluessel.punkt(),
            kapselpunkt: fragender_schluessel.kapselpunkt(),
        };
        let shard_punkte = Gegenpunkte {
            punkt: shard_schluessel.punkt(),
            kapselpunkt: shard_schluessel.kapselpunkt(),
        };
        let gesehen = Arc::new(AtomicUsize::new(0));
        let klartext = Arc::new(Mutex::new(Vec::new()));
        Aufbau {
            werk: Entsiegelndes::neu(
                PodId::new(POD),
                endpunkt(2),
                Sitzungen::neu(endpunkt(2), shard_schluessel),
                Box::new(EineSitzung {
                    sitzung,
                    wer: endpunkt(1),
                    punkte: fragender_punkte,
                }),
                Box::new(Zaehlwerk {
                    gesehen: Arc::clone(&gesehen),
                    klartext: Arc::clone(&klartext),
                }),
            ),
            gesehen,
            klartext,
            fragender: Sitzungen::neu(endpunkt(1), fragender_schluessel),
            shard_punkte,
        }
    }

    /// Baut einen Auftrag, dessen Prompt für den Shard versiegelt ist.
    fn auftrag(a: &mut Aufbau, sitzung: u64, prompt: &[u8], gebunden_an: &[u8]) -> Inferenzauftrag {
        let kanal = a
            .fragender
            .kanal(PodId::new(POD), endpunkt(2), &a.shard_punkte)
            .expect("Kanal");
        let versiegelt = Umschlag::schliessen(kanal, prompt).expect("schliessen");
        Inferenzauftrag {
            sitzung,
            bindung: Anfragebindung::neu(sitzung, gebunden_an, EpochId(1)),
            prompt_versiegelt: versiegelt,
            max_token: 32,
            pipeline: Hash::sha256(b"probe-pipeline"),
        }
    }

    /// ⚑ **Der ganze Zweck: Der Weg trägt keinen Klartext, und der
    /// Shard sieht ihn trotzdem.**
    #[test]
    fn ein_versiegelter_prompt_erreicht_das_rechenwerk_im_klartext() {
        let mut a = aufbau(7);
        let auf = auftrag(&mut a, 7, b"was ist die hauptstadt von frankreich", b"was ist die hauptstadt von frankreich");
        assert!(
            !auf.prompt_versiegelt
                .windows(10)
                .any(|f| f == b"hauptstadt"),
            "der Klartext steht auf der Leitung"
        );
        let antwort = a.werk.rechne(&auf);
        assert!(matches!(antwort, Inferenzantwort::Ergebnis { .. }), "{antwort:?}");
        assert_eq!(a.gesehen.load(Ordering::SeqCst), 1);
        assert_eq!(
            &*a.klartext.lock().expect("Klartext"),
            b"was ist die hauptstadt von frankreich"
        );
        assert_eq!(a.werk.letzter_grund(), None);
    }

    /// ⚑ **Eine Bindung, die nicht zum Klartext passt, hält das
    /// Rechenwerk auf.** Ohne diese Prüfung könnte jeder, der einen
    /// Kanal aufbauen darf, den Pod beliebig rechnen lassen und die
    /// Arbeit einer fremden Anfrage zuschreiben.
    #[test]
    fn eine_falsche_bindung_haelt_das_rechenwerk_auf() {
        let mut a = aufbau(7);
        let auf = auftrag(&mut a, 7, b"der wirkliche prompt", b"eine ganz andere frage");
        assert_eq!(
            a.werk.rechne(&auf),
            Inferenzantwort::Abgelehnt { sitzung: 7 }
        );
        assert_eq!(a.werk.letzter_grund(), Some(Abweisungsgrund::BindungPasstNicht));
        assert_eq!(a.gesehen.load(Ordering::SeqCst), 0, "das Rechenwerk lief trotzdem");
    }

    /// Eine fremde Sitzung wird abgewiesen, bevor entsiegelt wird.
    #[test]
    fn eine_fremde_sitzung_kostet_keine_entsiegelung() {
        let mut a = aufbau(7);
        let mut auf = auftrag(&mut a, 7, b"prompt", b"prompt");
        auf.sitzung = 8;
        assert_eq!(
            a.werk.rechne(&auf),
            Inferenzantwort::Abgelehnt { sitzung: 8 }
        );
        assert_eq!(a.werk.letzter_grund(), Some(Abweisungsgrund::FremdeSitzung));
        assert_eq!(a.gesehen.load(Ordering::SeqCst), 0);
    }

    /// ⚑ **Ein fremder Pipeline-Stand fällt vor dem Entsiegeln**, denn
    /// ein Hashvergleich ist billiger als eine KEM-Dekapselung.
    #[test]
    fn ein_fremder_pipelinestand_faellt_vor_dem_entsiegeln() {
        let mut a = aufbau(7);
        let mut auf = auftrag(&mut a, 7, b"prompt", b"prompt");
        auf.pipeline = Hash::sha256(b"ein anderer stand");
        assert_eq!(
            a.werk.rechne(&auf),
            Inferenzantwort::Abgelehnt { sitzung: 7 }
        );
        assert_eq!(a.werk.letzter_grund(), Some(Abweisungsgrund::FremdePipeline));
        assert_eq!(a.gesehen.load(Ordering::SeqCst), 0);
    }

    /// Ein Umschlag, der nicht für diesen Shard versiegelt wurde, geht
    /// nicht auf.
    #[test]
    fn ein_fremd_versiegelter_prompt_geht_nicht_auf() {
        let mut a = aufbau(7);
        let mut auf = auftrag(&mut a, 7, b"prompt", b"prompt");
        // Ein Byte im Geheimtext kippen: das Tag traegt nicht mehr.
        let n = auf.prompt_versiegelt.len();
        auf.prompt_versiegelt[n - 1] ^= 1;
        assert_eq!(
            a.werk.rechne(&auf),
            Inferenzantwort::Abgelehnt { sitzung: 7 }
        );
        assert_eq!(a.werk.letzter_grund(), Some(Abweisungsgrund::NichtEntsiegelbar));
        assert_eq!(a.gesehen.load(Ordering::SeqCst), 0);
    }
}
