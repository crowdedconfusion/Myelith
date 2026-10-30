//! **Die Prüfaufträge der Stufe 2** (Whitepaper 3.3, 6.4 und 6.8, Anhang
//! B.3; Festlegung des Projektinhabers vom 2026-10-29).
//!
//! # ⚑ Der Weg
//!
//! 1. Am Epochenwechsel legt der Zustand ab, wer prüfen darf: die Miner ohne
//!    Pod und die des Pods in Bereitschaft, dazu je Pod die Mitglieder der
//!    übrigen Zwillingsgruppen als Ersatz ([`crate::state::OffeneStichprobe`]).
//! 2. Mit der Ziehung entsteht je gezogenem Segment ein [`Pruefauftrag`] mit
//!    bis zu [`PRUEFER_JE_ZIEHUNG`] Checkern ([`auftraege_anlegen`]).
//! 3. Jeder Checker rechnet nach und **legt sich fest**, bevor er aufdeckt
//!    ([`festlegen`], [`aufdecken`]): Die Festlegung ist ein Hash über seinen
//!    Befund, ein Salz und seine Kennung ([`festlegung`]). Der Befund trägt
//!    den Hash der Logits an der letzten Position; den kennt nur, wer
//!    gerechnet hat, denn in der Spur des Pods steht er nicht.
//! 4. Nach der zweiten Frist schliesst die Kette ab ([`faellige_abschliessen`]):
//!    Zwei gleiche Befunde zählen als geprüftes Volumen (die Gewichte des
//!    Checker-Pools, Anhang B.3); jede aufgedeckte Abweichung wird ein
//!    Streitfall, auch wenn der andere Checker schweigt (6.8 (b): ein
//!    ehrlicher Checker genügt).
//!
//! # ⚑ Die Fristen sind fest
//!
//! Wie bei Arbitrum BoLD hängt das Fenster nicht daran, wer wann etwas
//! einreicht: Festlegen bis `frist_festlegen`, Aufdecken danach bis
//! `frist_aufdecken`, beides Blockhöhen, beim Anlegen gesetzt. Aufgedeckt
//! werden darf schon früher, sobald alle Checker festgelegt haben; vorher
//! nicht, sonst könnte der zweite den ersten abschreiben.

use borsh::{BorshDeserialize, BorshSerialize};
use myl_types::hash::Hash;
use myl_types::ids::{Address, MerkleRoot, MinerId, PodId};

use crate::state::{LedgerState, OffeneStichprobe};

/// Die Besetzung eines Pods: je Position Miner und Layerbereich `[von, bis)`.
pub type Besetzung = Vec<(MinerId, u32, u32)>;

/// So viele Checker bekommt eine Ziehung, wenn es so viele Kandidaten gibt.
pub const PRUEFER_JE_ZIEHUNG: usize = 2;

/// **Was ein Checker gefunden hat.**
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Pruefbefund {
    /// SHA-256 über die Logits an der letzten Position des Verlaufs, je
    /// Wert als `i32` in Little Endian.
    pub logits: Hash,
    /// Die erste Abweichung als (Position, Eintrag), Eintrag null ist der
    /// Eingang, `l + 1` der Ausgang der Layer `l`; `None` heisst gleich.
    pub abweichung: Option<(u32, u32)>,
    /// Wie viele Positionen nachgerechnet wurden: das geprüfte Volumen.
    pub positionen: u32,
}

/// **Ein Prüfauftrag**: ein gezogenes Segment und seine Checker.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Pruefauftrag {
    /// Die gezogene Epoche.
    pub epoche: u64,
    pub pod: PodId,
    pub segment: u32,
    /// Die zugeteilten Checker, höchstens [`PRUEFER_JE_ZIEHUNG`].
    pub pruefer: Vec<MinerId>,
    /// Je Checker seine Festlegung.
    pub festlegungen: Vec<(MinerId, Hash)>,
    /// Je Checker sein aufgedeckter Befund.
    pub aufdeckungen: Vec<(MinerId, Pruefbefund)>,
    /// Letzte Höhe, in der festgelegt werden darf.
    pub frist_festlegen: u64,
    /// Letzte Höhe, in der aufgedeckt werden darf.
    pub frist_aufdecken: u64,
    /// `segments_root` und Segmentzahl des Bündels: dagegen prüft die
    /// Schiedsrunde den Verlauf, auch wenn längst neu gezogen wurde.
    pub wurzel: MerkleRoot,
    pub segmente: u32,
    /// Die Besetzung des Pods in der gezogenen Epoche: je Position Miner
    /// und Layerbereich `[von, bis)`. Daraus folgt, wen eine Abweichung
    /// trifft.
    pub besetzung: Besetzung,
}

/// ⚑ **Ein Streitfall** (T6d, Whitepaper 6.6, Festlegung des
/// Projektinhabers vom 2026-10-29): Ein Checker deckte eine Abweichung auf.
/// Das Komitee der Validatoren rechnet **eine** Layer an **einer** Position
/// nach; die Eingänge der Layer bringt der Anfechter mit, jeder gegen den
/// Hash geprüft, den der Pod selbst bezeugt hat.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Streitfall {
    pub epoche: u64,
    pub pod: PodId,
    pub segment: u32,
    /// Die erste Abweichung: Position und Eintrag (Eintrag `l + 1` ist der
    /// Ausgang der Layer `l`).
    pub position: u32,
    pub eintrag: u32,
    pub anfechter: MinerId,
    /// Wessen Layerbereich die Layer enthält; `None`, wenn die Besetzung
    /// es nicht hergibt (dann kann nur der Anfechter verlieren).
    pub beschuldigt: Option<MinerId>,
    pub wurzel: MerkleRoot,
    pub segmente: u32,
    /// Letzte Höhe für Stimmen: das feste Fenster (Arbitrum BoLD).
    pub frist: u64,
    /// Je Validator seine Stimme: `true` heisst, der Pod rechnete falsch.
    pub stimmen: Vec<(MinerId, bool)>,
}

/// Wie ein Streitfall endete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schiedsausgang {
    /// Mehr als zwei Drittel: Der Pod rechnete falsch.
    PodFalsch,
    /// Mehr als zwei Drittel: Die Anfechtung war falsch.
    AnfechtungFalsch,
    /// Die Frist lief ohne Mehrheit ab; niemand wird geschlachtet.
    Verfallen,
}

/// Ein entschiedener Streitfall.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schiedsurteil {
    pub fall: Streitfall,
    pub ausgang: Schiedsausgang,
}

/// Wie ein Prüfauftrag endete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pruefausgang {
    /// Alle Checker deckten denselben Befund auf; vergütet wird jeder.
    Einig,
    /// Aufgedeckt, aber verschieden, ohne Abweichung: niemand vergütet.
    Uneinig,
    /// Nicht alle deckten auf: niemand vergütet.
    Unvollstaendig,
}

/// **Ein abgeschlossener Prüfauftrag**, für das Protokoll und die
/// Schiedsrunde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pruefabschluss {
    pub auftrag: Pruefauftrag,
    pub ausgang: Pruefausgang,
    /// Wer eine Abweichung aufdeckte, mit ihr: Daraus wird ein Streitfall.
    pub abweichungen: Vec<(MinerId, (u32, u32))>,
}

/// Warum ein Festlegen oder Aufdecken abgewiesen wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pruefungsfehler {
    /// Zu dieser Ziehung gibt es keinen offenen Auftrag.
    KeinAuftrag,
    /// Der Absender ist nicht als Checker zugeteilt.
    NichtZugeteilt,
    /// Die Frist ist vorbei.
    FristVorbei,
    /// Schon festgelegt oder schon aufgedeckt.
    Doppelt,
    /// Aufdecken ohne Festlegung, oder der Befund passt nicht zu ihr.
    PasstNichtZurFestlegung,
    /// Noch nicht alle haben festgelegt, und die Frist läuft noch.
    ZuFrueh,
    /// Zu diesem Streitfall darf der Absender nicht stimmen.
    NichtStimmberechtigt,
    /// Der Differenzbeweis trägt nicht.
    Differenz(myl_types::Differenzfehler),
}

/// **Die Checker einer Ziehung** (W6b, Frage 1 und 4 vom 2026-10-29): aus
/// den Kandidaten, geordnet nach `SHA-256("myelith-pruefer" ‖ Epoche ‖ Pod ‖
/// Index ‖ Kennung)`, die ersten `anzahl`. Pod und Index sind schon die
/// Ausgabe der Saat aus dem Zertifikat; es gibt keinen zweiten Spielraum.
/// Jeder Knoten kommt so auf dieselben, und keiner wird zweimal genommen.
pub fn pruefer_waehlen(kandidaten: &[MinerId], epoche: u64, pod: &PodId, segment: u32, anzahl: usize) -> Vec<MinerId> {
    let mut rang: Vec<(Hash, MinerId)> = kandidaten
        .iter()
        .map(|m| {
            let mut eingabe = b"myelith-pruefer".to_vec();
            eingabe.extend_from_slice(&epoche.to_le_bytes());
            eingabe.extend_from_slice(pod.as_ref());
            eingabe.extend_from_slice(&segment.to_le_bytes());
            eingabe.extend_from_slice(m.as_bytes());
            (Hash::sha256(&eingabe), *m)
        })
        .collect();
    rang.sort();
    rang.dedup_by(|a, b| a.1 == b.1);
    rang.into_iter().take(anzahl).map(|(_, m)| m).collect()
}

/// **Die Festlegung eines Befunds**: `SHA-256("myelith-festlegung" ‖ borsh(Befund)
/// ‖ Salz ‖ Kennung)`. Die Kennung steht darin, damit niemand die
/// Festlegung eines anderen als eigene einreicht.
pub fn festlegung(befund: &Pruefbefund, salz: &[u8; 32], pruefer: &MinerId) -> Hash {
    let mut eingabe = b"myelith-festlegung".to_vec();
    eingabe.extend_from_slice(&borsh::to_vec(befund).expect("ein Befund ist stets serialisierbar"));
    eingabe.extend_from_slice(salz);
    eingabe.extend_from_slice(pruefer.as_bytes());
    Hash::sha256(&eingabe)
}

/// **Die Aufträge einer Ziehung anlegen.** Die Kandidaten sind die Miner
/// ohne Pod samt Bereitschaft; der Ersatz je Pod (die Mitglieder der
/// übrigen Zwillingsgruppen) wird nur genommen, wenn es keine Kandidaten
/// gibt (Frage 4 vom 2026-10-29). Ein Mitglied des gezogenen Pods prüft nie
/// sich selbst. Ohne jeden Checker entsteht kein Auftrag; das Ergebnis
/// nennt, wie viele Ziehungen ohne blieben.
pub fn auftraege_anlegen(
    state: &mut LedgerState,
    offen: &OffeneStichprobe,
    gezogen: &[(PodId, u32)],
    frist_festlegen: u64,
    frist_aufdecken: u64,
) -> usize {
    let mut ohne = 0;
    for (pod, segment) in gezogen {
        // 📌 `gezielt22`: Ein Differenzbeweis traf ein Segment, das schon
        //    gezogen war; der zweite Auftrag bekam nie eine Festlegung (sie
        //    geht an den ersten) und schloss unvollständig. Die Checker
        //    hängen nur am Segment, ein zweiter Auftrag wäre derselbe.
        if state.pruefauftraege.iter().any(|a| a.epoche == offen.epoche && a.pod == *pod && a.segment == *segment) {
            continue;
        }
        let liste = if offen.pruefkandidaten.is_empty() {
            offen.ersatz.iter().find(|(p, _)| p == pod).map(|(_, m)| m.as_slice()).unwrap_or(&[])
        } else {
            &offen.pruefkandidaten
        };
        let pruefer = pruefer_waehlen(liste, offen.epoche, pod, *segment, PRUEFER_JE_ZIEHUNG);
        let Some((_, wurzel, segmente)) = offen.wurzeln.iter().find(|(p, _, _)| p == pod) else {
            ohne += 1;
            continue;
        };
        if pruefer.is_empty() {
            ohne += 1;
            continue;
        }
        state.pruefauftraege.push(Pruefauftrag {
            epoche: offen.epoche,
            pod: *pod,
            segment: *segment,
            pruefer,
            festlegungen: Vec::new(),
            aufdeckungen: Vec::new(),
            frist_festlegen,
            frist_aufdecken,
            wurzel: *wurzel,
            segmente: *segmente,
            besetzung: offen.besetzung.iter().find(|(p, _)| p == pod).map(|(_, b)| b.clone()).unwrap_or_default(),
        });
    }
    ohne
}

/// ⚑ **Eine uneinige Zwillingsgruppe** (Whitepaper 6.4 und 6.6,
/// Festlegung des Projektinhabers vom 2026-10-29): Beide Pods reichten ein
/// Bündel ein, mit verschiedener Ausgabenwurzel. Am Wechsel abgelegt, bis
/// `bis_hoehe` darf ein Differenzbeweis gemeldet werden; jeder legt für
/// genau das abweichende Segment in beiden Pods einen Prüfauftrag an.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Uneinigkeit {
    /// Epoche, Wurzeln, Kandidaten, Ersatz und Besetzung der beiden Pods.
    pub stichprobe: OffeneStichprobe,
    /// Die Ausgabenwurzel je Pod.
    pub ausgabenwurzeln: Vec<(PodId, MerkleRoot)>,
    /// Schon gemeldete Segmente, höchstens [`DIFFERENZEN_JE_GRUPPE`].
    pub gemeldet: Vec<myl_types::ids::SegmentId>,
    /// Letzte Höhe für eine Meldung.
    pub bis_hoehe: u64,
}

/// Höchstens so viele Differenzen je uneiniger Gruppe: Eine genügt, um den
/// Falschen zu finden; mehr begrenzt nur die Prüflast.
pub const DIFFERENZEN_JE_GRUPPE: usize = 4;

/// **Einen Differenzbeweis melden** (2026-10-29): Er muss gegen die Wurzeln
/// beider Bündel der uneinigen Gruppe tragen
/// ([`myl_types::differenz_pruefen`]); dann je Pod ein Prüfauftrag für das
/// Segment an seiner Stelle, mit Checkern aus den Kandidaten der Epoche.
/// Wer meldet, ist gleich: Der Beweis trägt sich selbst.
#[allow(clippy::too_many_arguments)]
pub fn differenz_melden(
    state: &mut LedgerState,
    epoche: u64,
    pod_a: &PodId,
    pod_b: &PodId,
    beweis: &myl_types::Differenzbeweis,
    hoehe: u64,
    frist_festlegen: u64,
    frist_aufdecken: u64,
) -> Result<(), Pruefungsfehler> {
    let stelle = state
        .uneinigkeiten
        .iter()
        .position(|u| {
            u.stichprobe.epoche == epoche
                && u.ausgabenwurzeln.iter().any(|(p, _)| p == pod_a)
                && u.ausgabenwurzeln.iter().any(|(p, _)| p == pod_b)
                && pod_a != pod_b
        })
        .ok_or(Pruefungsfehler::KeinAuftrag)?;
    let u = &state.uneinigkeiten[stelle];
    if hoehe > u.bis_hoehe {
        return Err(Pruefungsfehler::FristVorbei);
    }
    if u.gemeldet.contains(&beweis.a.ausgabe.id) || u.gemeldet.len() >= DIFFERENZEN_JE_GRUPPE {
        return Err(Pruefungsfehler::Doppelt);
    }
    let wurzeln = |p: &PodId| {
        let aus = u.ausgabenwurzeln.iter().find(|(q, _)| q == p).map(|(_, w)| *w);
        let seg = u.stichprobe.wurzeln.iter().find(|(q, _, _)| q == p).map(|(_, w, n)| (*w, *n));
        aus.zip(seg)
    };
    let (Some((wa, (sa, na))), Some((wb, (sb, nb)))) = (wurzeln(pod_a), wurzeln(pod_b)) else {
        return Err(Pruefungsfehler::KeinAuftrag);
    };
    let (ia, ib) = myl_types::differenz_pruefen(beweis, (&wa, &sa, na), (&wb, &sb, nb)).map_err(Pruefungsfehler::Differenz)?;
    let offen = u.stichprobe.clone();
    state.uneinigkeiten[stelle].gemeldet.push(beweis.a.ausgabe.id);
    auftraege_anlegen(state, &offen, &[(*pod_a, ia), (*pod_b, ib)], frist_festlegen, frist_aufdecken);
    Ok(())
}

/// ⚑ **Ein Vorbehalt** (Whitepaper 6.4: „Rückbuchung der
/// vTFE-Gutschrift“, Festlegung des Projektinhabers vom 2026-10-29): Was
/// ein Pod einer uneinigen Gruppe verdient hätte, je Miner, zurückgehalten
/// bis zur Entscheidung. Verurteilt die Schiedsrunde seinen Zwilling, wird
/// er frei und am nächsten Wechsel ausgezahlt; wird er selbst verurteilt,
/// verfällt er; ohne Urteil bis `bis_epoche` verfällt er ebenso. So
/// bekommt ein ehrlicher Pod seinen Lohn, sobald der andere überführt ist,
/// und hat allen Grund, den Differenzbeweis zu melden.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Vorbehalt {
    pub epoche: u64,
    pub pod: PodId,
    pub zwilling: PodId,
    pub je_miner: Vec<(MinerId, u64)>,
    pub frei: bool,
    pub bis_epoche: u64,
}

/// **Ein Pod ist überführt** (Schiedsurteil „Pod falsch“): sein Vorbehalt
/// der Epoche verfällt, der seines Zwillings wird frei.
pub fn vorbehalt_entscheiden(state: &mut LedgerState, epoche: u64, falsch: &PodId) {
    state.vorbehalte.retain(|v| !(v.epoche == epoche && v.pod == *falsch));
    for v in state.vorbehalte.iter_mut().filter(|v| v.epoche == epoche && v.zwilling == *falsch) {
        v.frei = true;
    }
}

/// **Am Wechsel**: die freien Vorbehalte als Gutschrift je Miner (sie
/// fallen weg und zählen in `rueckbuchungen`), abgelaufene unfreie
/// verfallen.
pub fn vorbehalte_faellig(state: &mut LedgerState, alte_epoche: u64) -> Vec<(MinerId, u64)> {
    let mut aus = Vec::new();
    let mut bleiben = Vec::new();
    for v in std::mem::take(&mut state.vorbehalte) {
        if v.frei {
            aus.extend(v.je_miner);
            state.rueckbuchungen = state.rueckbuchungen.saturating_add(1);
        } else if v.bis_epoche > alte_epoche {
            bleiben.push(v);
        }
    }
    state.vorbehalte = bleiben;
    aus
}

/// Uneinigkeiten nach ihrer Frist fallen weg.
pub fn uneinigkeiten_aufraeumen(state: &mut LedgerState, hoehe: u64) {
    state.uneinigkeiten.retain(|u| hoehe <= u.bis_hoehe);
}

fn auftrag_mut<'a>(
    state: &'a mut LedgerState,
    absender: &Address,
    epoche: u64,
    pod: &PodId,
    segment: u32,
) -> Result<(&'a mut Pruefauftrag, MinerId), Pruefungsfehler> {
    let ich = MinerId::new(*absender.as_bytes());
    let a = state
        .pruefauftraege
        .iter_mut()
        .find(|a| a.epoche == epoche && a.pod == *pod && a.segment == segment)
        .ok_or(Pruefungsfehler::KeinAuftrag)?;
    if !a.pruefer.contains(&ich) {
        return Err(Pruefungsfehler::NichtZugeteilt);
    }
    Ok((a, ich))
}

/// **Festlegen**: bis einschliesslich `frist_festlegen`, einmal je Checker.
pub fn festlegen(
    state: &mut LedgerState,
    absender: &Address,
    epoche: u64,
    pod: &PodId,
    segment: u32,
    wert: Hash,
    hoehe: u64,
) -> Result<(), Pruefungsfehler> {
    let (a, ich) = auftrag_mut(state, absender, epoche, pod, segment)?;
    if hoehe > a.frist_festlegen {
        return Err(Pruefungsfehler::FristVorbei);
    }
    if a.festlegungen.iter().any(|(m, _)| *m == ich) {
        return Err(Pruefungsfehler::Doppelt);
    }
    a.festlegungen.push((ich, wert));
    Ok(())
}

/// **Aufdecken**: sobald alle festgelegt haben oder die erste Frist vorbei
/// ist, bis einschliesslich `frist_aufdecken`; der Befund muss zur eigenen
/// Festlegung passen.
#[allow(clippy::too_many_arguments)]
pub fn aufdecken(
    state: &mut LedgerState,
    absender: &Address,
    epoche: u64,
    pod: &PodId,
    segment: u32,
    befund: Pruefbefund,
    salz: &[u8; 32],
    hoehe: u64,
) -> Result<(), Pruefungsfehler> {
    let (a, ich) = auftrag_mut(state, absender, epoche, pod, segment)?;
    if hoehe > a.frist_aufdecken {
        return Err(Pruefungsfehler::FristVorbei);
    }
    if hoehe <= a.frist_festlegen && a.festlegungen.len() < a.pruefer.len() {
        return Err(Pruefungsfehler::ZuFrueh);
    }
    if a.aufdeckungen.iter().any(|(m, _)| *m == ich) {
        return Err(Pruefungsfehler::Doppelt);
    }
    let Some((_, wert)) = a.festlegungen.iter().find(|(m, _)| *m == ich) else {
        return Err(Pruefungsfehler::PasstNichtZurFestlegung);
    };
    if *wert != festlegung(&befund, salz, &ich) {
        return Err(Pruefungsfehler::PasstNichtZurFestlegung);
    }
    a.aufdeckungen.push((ich, befund));
    Ok(())
}

/// **Abschliessen, was fällig ist**: jeder Auftrag, dessen zweite Frist vor
/// `hoehe` liegt. Einige Checker bekommen ihr geprüftes Volumen auf
/// [`LedgerState::pruefvolumen`] gutgeschrieben; das zahlt der Checker-Pool
/// am nächsten Epochenwechsel aus.
///
/// ⚑ **Mit nur einem zugeteilten Checker** (es gab nur einen Kandidaten)
/// zählt seine Aufdeckung allein: Es gibt niemanden, mit dem sie einig sein
/// könnte, und ohne Vergütung fände sich für solche Ziehungen keiner.
///
/// ⚑ **Jede aufgedeckte Abweichung öffnet einen [`Streitfall`]**, einen je
/// Auftrag, mit der frühesten behaupteten Stelle (Position, dann Eintrag);
/// Stimmen bis `hoehe + schied_fenster`.
pub fn faellige_abschliessen(state: &mut LedgerState, hoehe: u64, schied_fenster: u64) -> Vec<Pruefabschluss> {
    let (faellig, offen): (Vec<_>, Vec<_>) =
        std::mem::take(&mut state.pruefauftraege).into_iter().partition(|a| a.frist_aufdecken < hoehe);
    state.pruefauftraege = offen;
    let mut aus = Vec::with_capacity(faellig.len());
    for auftrag in faellig {
        let abweichungen: Vec<(MinerId, (u32, u32))> =
            auftrag.aufdeckungen.iter().filter_map(|(m, b)| b.abweichung.map(|x| (*m, x))).collect();
        let alle = auftrag.aufdeckungen.len() == auftrag.pruefer.len();
        let gleich = auftrag.aufdeckungen.windows(2).all(|w| w[0].1 == w[1].1);
        let ausgang = match (alle, gleich) {
            (true, true) => Pruefausgang::Einig,
            (true, false) => Pruefausgang::Uneinig,
            (false, _) => Pruefausgang::Unvollstaendig,
        };
        if ausgang == Pruefausgang::Einig {
            for (m, b) in &auftrag.aufdeckungen {
                let v = state.pruefvolumen.entry(*m).or_insert(0);
                *v = v.saturating_add(u64::from(b.positionen.max(1)));
            }
        }
        if let Some((anfechter, (position, eintrag))) = abweichungen.iter().min_by_key(|(_, x)| *x).copied() {
            let layer = eintrag.saturating_sub(1);
            let beschuldigt = auftrag
                .besetzung
                .iter()
                .find(|(_, von, bis)| *von <= layer && layer < *bis)
                .or_else(|| auftrag.besetzung.iter().max_by_key(|(_, _, bis)| *bis).filter(|(_, _, bis)| layer >= *bis))
                .map(|(m, _, _)| *m);
            state.streitfaelle.push(Streitfall {
                epoche: auftrag.epoche,
                pod: auftrag.pod,
                segment: auftrag.segment,
                position,
                eintrag,
                anfechter,
                beschuldigt,
                wurzel: auftrag.wurzel,
                segmente: auftrag.segmente,
                frist: hoehe.saturating_add(schied_fenster),
                stimmen: Vec::new(),
            });
        }
        aus.push(Pruefabschluss { auftrag, ausgang, abweichungen });
    }
    aus
}

/// **Eine Schiedsstimme**: einmal je Validator und Streitfall, bis zur
/// Frist. Ob der Absender im Stimmsatz steht, weiss die Kette, nicht der
/// Ledger: `stimmberechtigt`.
#[allow(clippy::too_many_arguments)]
pub fn schiedsstimme(
    state: &mut LedgerState,
    absender: &Address,
    epoche: u64,
    pod: &PodId,
    segment: u32,
    pod_falsch: bool,
    hoehe: u64,
    stimmberechtigt: bool,
) -> Result<(), Pruefungsfehler> {
    if !stimmberechtigt {
        return Err(Pruefungsfehler::NichtStimmberechtigt);
    }
    let ich = MinerId::new(*absender.as_bytes());
    let fall = state
        .streitfaelle
        .iter_mut()
        .find(|f| f.epoche == epoche && f.pod == *pod && f.segment == segment)
        .ok_or(Pruefungsfehler::KeinAuftrag)?;
    if hoehe > fall.frist {
        return Err(Pruefungsfehler::FristVorbei);
    }
    if fall.stimmen.iter().any(|(m, _)| *m == ich) {
        return Err(Pruefungsfehler::Doppelt);
    }
    fall.stimmen.push((ich, pod_falsch));
    Ok(())
}

/// **Streitfälle entscheiden**: Sobald eine Seite die Schwelle erreicht
/// (mehr als zwei Drittel des Gewichts, `schwelle`), ist der Fall
/// entschieden; läuft die Frist ohne Mehrheit ab, verfällt er. Geschlachtet
/// wird nicht hier: Die Kette bucht das Urteil nach der Matrix.
pub fn streitfaelle_entscheiden(
    state: &mut LedgerState,
    hoehe: u64,
    gewicht: impl Fn(&MinerId) -> u64,
    schwelle: u64,
) -> Vec<Schiedsurteil> {
    let mut aus = Vec::new();
    let mut bleiben = Vec::new();
    for fall in std::mem::take(&mut state.streitfaelle) {
        let summe = |seite: bool| -> u64 {
            fall.stimmen.iter().filter(|(_, s)| *s == seite).map(|(m, _)| gewicht(m)).fold(0u64, u64::saturating_add)
        };
        let ausgang = if schwelle > 0 && summe(true) >= schwelle {
            Some(Schiedsausgang::PodFalsch)
        } else if schwelle > 0 && summe(false) >= schwelle {
            Some(Schiedsausgang::AnfechtungFalsch)
        } else if hoehe > fall.frist {
            Some(Schiedsausgang::Verfallen)
        } else {
            None
        };
        match ausgang {
            Some(ausgang) => aus.push(Schiedsurteil { fall, ausgang }),
            None => bleiben.push(fall),
        }
    }
    state.streitfaelle = bleiben;
    aus
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(b: u8) -> MinerId {
        MinerId::new([b; 32])
    }
    fn a(b: u8) -> Address {
        Address::new([b; 32])
    }
    fn pod() -> PodId {
        PodId::new([9; 32])
    }
    fn befund(abweichung: Option<(u32, u32)>) -> Pruefbefund {
        Pruefbefund { logits: Hash::sha256(b"logits"), abweichung, positionen: 7 }
    }

    #[test]
    fn die_wahl_ist_fest_und_ohne_doppelte() {
        let k = [m(1), m(2), m(3), m(4), m(2)];
        let w = pruefer_waehlen(&k, 3, &pod(), 5, 2);
        assert_eq!(w.len(), 2);
        assert_ne!(w[0], w[1]);
        assert!(w.iter().all(|x| k.contains(x)));
        // Dieselbe Eingabe, in anderer Reihenfolge: dieselben Checker.
        let mut r = k.to_vec();
        r.reverse();
        assert_eq!(pruefer_waehlen(&r, 3, &pod(), 5, 2), w);
        // Ein Kandidat allein ist der einzige Checker.
        assert_eq!(pruefer_waehlen(&[m(7)], 3, &pod(), 5, 2), vec![m(7)]);
        assert!(pruefer_waehlen(&[], 3, &pod(), 5, 2).is_empty());
    }

    fn offen(kandidaten: Vec<MinerId>, ersatz: Vec<(PodId, Vec<MinerId>)>) -> OffeneStichprobe {
        let andere = PodId::new([8; 32]);
        OffeneStichprobe {
            epoche: 3,
            anteile: Vec::new(),
            wurzeln: vec![(pod(), MerkleRoot::new([1; 32]), 10), (andere, MerkleRoot::new([2; 32]), 10)],
            voll: Vec::new(),
            pruefkandidaten: kandidaten,
            ersatz,
            // Vier Positionen über 28 Layer.
            besetzung: vec![(pod(), vec![(m(20), 0, 7), (m(21), 7, 14), (m(22), 14, 21), (m(23), 21, 28)])],
        }
    }

    #[test]
    fn ohne_kandidaten_kommt_der_ersatz_aus_der_anderen_gruppe() {
        let mut s = LedgerState::genesis(1);
        let ersatz = vec![(pod(), vec![m(5), m(6)])];
        let ohne = auftraege_anlegen(&mut s, &offen(Vec::new(), ersatz.clone()), &[(pod(), 1), (PodId::new([8; 32]), 2)], 10, 20);
        assert_eq!(ohne, 1, "der zweite Pod hat keinen Ersatz");
        assert_eq!(s.pruefauftraege.len(), 1);
        assert!(s.pruefauftraege[0].pruefer.iter().all(|x| [m(5), m(6)].contains(x)));
        assert_eq!(s.pruefauftraege[0].wurzel, MerkleRoot::new([1; 32]));
        assert_eq!(s.pruefauftraege[0].besetzung.len(), 4);
        // Mit Kandidaten bleibt der Ersatz unberührt.
        let mut s = LedgerState::genesis(1);
        auftraege_anlegen(&mut s, &offen(vec![m(1)], ersatz), &[(pod(), 1)], 10, 20);
        assert_eq!(s.pruefauftraege[0].pruefer, vec![m(1)]);
    }

    fn mit_auftrag() -> (LedgerState, Vec<MinerId>) {
        let mut s = LedgerState::genesis(1);
        auftraege_anlegen(&mut s, &offen(vec![m(1), m(2)], Vec::new()), &[(pod(), 1)], 10, 20);
        let p = s.pruefauftraege[0].pruefer.clone();
        (s, p)
    }

    #[test]
    fn festlegen_dann_aufdecken_und_einig_gibt_volumen() {
        let (mut s, p) = mit_auftrag();
        let b = befund(None);
        let salz = [3u8; 32];
        let (x, y) = (a(p[0].as_bytes()[0]), a(p[1].as_bytes()[0]));
        assert_eq!(festlegen(&mut s, &a(9), 3, &pod(), 1, Hash::sha256(b"x"), 5), Err(Pruefungsfehler::NichtZugeteilt));
        festlegen(&mut s, &x, 3, &pod(), 1, festlegung(&b, &salz, &p[0]), 5).unwrap();
        // ⛔️ Vor der Festlegung des zweiten darf niemand aufdecken.
        assert_eq!(aufdecken(&mut s, &x, 3, &pod(), 1, b.clone(), &salz, 6), Err(Pruefungsfehler::ZuFrueh));
        assert_eq!(festlegen(&mut s, &x, 3, &pod(), 1, Hash::sha256(b"x"), 6), Err(Pruefungsfehler::Doppelt));
        festlegen(&mut s, &y, 3, &pod(), 1, festlegung(&b, &salz, &p[1]), 6).unwrap();
        // Ein anderer Befund als festgelegt wird abgewiesen.
        assert_eq!(
            aufdecken(&mut s, &x, 3, &pod(), 1, befund(Some((1, 2))), &salz, 7),
            Err(Pruefungsfehler::PasstNichtZurFestlegung)
        );
        aufdecken(&mut s, &x, 3, &pod(), 1, b.clone(), &salz, 7).unwrap();
        aufdecken(&mut s, &y, 3, &pod(), 1, b.clone(), &salz, 8).unwrap();
        assert!(faellige_abschliessen(&mut s, 20, 30).is_empty(), "die Frist läuft noch");
        let ab = faellige_abschliessen(&mut s, 21, 30);
        assert_eq!(ab.len(), 1);
        assert_eq!(ab[0].ausgang, Pruefausgang::Einig);
        assert!(ab[0].abweichungen.is_empty());
        assert_eq!(s.pruefvolumen.get(&p[0]), Some(&7));
        assert_eq!(s.pruefvolumen.get(&p[1]), Some(&7));
        assert!(s.pruefauftraege.is_empty());
    }

    #[test]
    fn eine_abweichung_allein_wird_ein_streitfall_aber_nicht_bezahlt() {
        let (mut s, p) = mit_auftrag();
        let salz = [4u8; 32];
        let x = a(p[0].as_bytes()[0]);
        let b = befund(Some((2, 6)));
        festlegen(&mut s, &x, 3, &pod(), 1, festlegung(&b, &salz, &p[0]), 5).unwrap();
        // Der zweite schweigt; nach der ersten Frist darf aufgedeckt werden.
        assert_eq!(aufdecken(&mut s, &x, 3, &pod(), 1, b.clone(), &salz, 10), Err(Pruefungsfehler::ZuFrueh));
        aufdecken(&mut s, &x, 3, &pod(), 1, b, &salz, 11).unwrap();
        assert_eq!(aufdecken(&mut s, &x, 3, &pod(), 1, befund(None), &salz, 21), Err(Pruefungsfehler::FristVorbei));
        let ab = faellige_abschliessen(&mut s, 21, 30);
        assert_eq!(ab[0].ausgang, Pruefausgang::Unvollstaendig);
        assert_eq!(ab[0].abweichungen, vec![(p[0], (2, 6))]);
        assert!(s.pruefvolumen.is_empty());
        // ⚑ Der Streitfall: Eintrag 6 ist der Ausgang der Layer 5, und die
        //   liegt im Bereich [0, 7) des ersten Mitglieds.
        assert_eq!(s.streitfaelle.len(), 1);
        let f = &s.streitfaelle[0];
        assert_eq!((f.position, f.eintrag, f.anfechter, f.beschuldigt, f.frist), (2, 6, p[0], Some(m(20)), 51));
    }

    #[test]
    fn das_komitee_entscheidet_mit_mehr_als_zwei_dritteln_oder_der_fall_verfaellt() {
        let fall = |stimmen: Vec<(MinerId, bool)>| Streitfall {
            epoche: 3,
            pod: pod(),
            segment: 1,
            position: 2,
            eintrag: 6,
            anfechter: m(1),
            beschuldigt: Some(m(20)),
            wurzel: MerkleRoot::new([1; 32]),
            segmente: 10,
            frist: 50,
            stimmen,
        };
        let mut s = LedgerState::genesis(1);
        s.streitfaelle.push(fall(Vec::new()));
        // Nicht im Stimmsatz, nach der Frist, doppelt: abgewiesen.
        assert_eq!(schiedsstimme(&mut s, &a(30), 3, &pod(), 1, true, 10, false), Err(Pruefungsfehler::NichtStimmberechtigt));
        assert_eq!(schiedsstimme(&mut s, &a(30), 3, &pod(), 1, true, 51, true), Err(Pruefungsfehler::FristVorbei));
        schiedsstimme(&mut s, &a(30), 3, &pod(), 1, true, 10, true).unwrap();
        assert_eq!(schiedsstimme(&mut s, &a(30), 3, &pod(), 1, false, 11, true), Err(Pruefungsfehler::Doppelt));
        schiedsstimme(&mut s, &a(31), 3, &pod(), 1, true, 12, true).unwrap();
        // Vier Validatoren zu je 1000, Schwelle 2667: zwei Stimmen reichen nicht.
        assert!(streitfaelle_entscheiden(&mut s, 13, |_| 1_000, 2_667).is_empty());
        schiedsstimme(&mut s, &a(32), 3, &pod(), 1, true, 14, true).unwrap();
        let u = streitfaelle_entscheiden(&mut s, 15, |_| 1_000, 2_667);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].ausgang, Schiedsausgang::PodFalsch);
        assert!(s.streitfaelle.is_empty());
        // Die Gegenseite ebenso.
        s.streitfaelle.push(fall(vec![(m(30), false), (m(31), false), (m(32), false)]));
        assert_eq!(streitfaelle_entscheiden(&mut s, 15, |_| 1_000, 2_667)[0].ausgang, Schiedsausgang::AnfechtungFalsch);
        // Ohne Mehrheit bis zur Frist: verfallen, nicht vorher.
        s.streitfaelle.push(fall(vec![(m(30), true), (m(31), false)]));
        assert!(streitfaelle_entscheiden(&mut s, 50, |_| 1_000, 2_667).is_empty());
        assert_eq!(streitfaelle_entscheiden(&mut s, 51, |_| 1_000, 2_667)[0].ausgang, Schiedsausgang::Verfallen);
    }

    #[test]
    fn verschiedene_befunde_ohne_abweichung_sind_uneinig() {
        let (mut s, p) = mit_auftrag();
        let salz = [5u8; 32];
        let b0 = befund(None);
        let b1 = Pruefbefund { logits: Hash::sha256(b"geraten"), ..befund(None) };
        for (i, b) in [(0usize, &b0), (1, &b1)] {
            festlegen(&mut s, &a(p[i].as_bytes()[0]), 3, &pod(), 1, festlegung(b, &salz, &p[i]), 5).unwrap();
        }
        for (i, b) in [(0usize, b0), (1, b1)] {
            aufdecken(&mut s, &a(p[i].as_bytes()[0]), 3, &pod(), 1, b, &salz, 6).unwrap();
        }
        let ab = faellige_abschliessen(&mut s, 21, 30);
        assert_eq!(ab[0].ausgang, Pruefausgang::Uneinig);
        assert!(s.pruefvolumen.is_empty());
    }

    /// ⚑ **Ein Differenzbeweis legt je Pod einen Auftrag an** (2026-10-29):
    /// für das abweichende Segment an seiner Stelle in jedem Bündel. Ein
    /// zweiter Beweis für dasselbe Segment, ein Beweis, der nicht trägt, und
    /// einer nach der Frist werden abgewiesen.
    #[test]
    fn ein_differenzbeweis_legt_je_pod_einen_auftrag_an() {
        use myl_types::core_types::{Ausgabenzeugnis, Segmentzeugnis};
        use myl_types::ids::SegmentId;
        let id = |b: u8| SegmentId::new([b; 32]);
        let zeugnis = |b: u8, aus: u8| Ausgabenzeugnis { id: id(b), eingang: [b; 32], ausgang: [aus; 32] };
        let seg = |b: u8| Segmentzeugnis { id: id(b), spurwurzel: MerkleRoot::new([b + 100; 32]) };
        let (pa, pb) = (PodId::new([10; 32]), PodId::new([11; 32]));
        let aus_a = vec![zeugnis(1, 1), zeugnis(2, 2)];
        let aus_b = vec![zeugnis(1, 1), zeugnis(2, 9)];
        let seg_a = vec![seg(1), seg(2)];
        let seg_b = vec![seg(2), seg(1)];
        let wurzel = |x: Result<MerkleRoot, _>| x.expect("Wurzel");
        let mut st = LedgerState::genesis(1);
        st.uneinigkeiten.push(Uneinigkeit {
            stichprobe: OffeneStichprobe {
                epoche: 3,
                anteile: Vec::new(),
                wurzeln: vec![
                    (pa, wurzel(myl_types::segments_root(&seg_a)), 2),
                    (pb, wurzel(myl_types::segments_root(&seg_b)), 2),
                ],
                voll: Vec::new(),
                pruefkandidaten: vec![m(1), m(2), m(3)],
                ersatz: Vec::new(),
                besetzung: Vec::new(),
            },
            ausgabenwurzeln: vec![(pa, wurzel(myl_types::ausgabenwurzel(&aus_a))), (pb, wurzel(myl_types::ausgabenwurzel(&aus_b)))],
            gemeldet: Vec::new(),
            bis_hoehe: 50,
        });
        let beweis = myl_types::Differenzbeweis {
            a: myl_types::differenzseite(&aus_a, &seg_a, &id(2)).expect("Seite a"),
            b: myl_types::differenzseite(&aus_b, &seg_b, &id(2)).expect("Seite b"),
        };
        let mut schief = beweis.clone();
        schief.a.ausgabe.ausgang = [5; 32];
        assert!(matches!(differenz_melden(&mut st, 3, &pa, &pb, &schief, 10, 20, 30), Err(Pruefungsfehler::Differenz(_))));
        differenz_melden(&mut st, 3, &pa, &pb, &beweis, 10, 20, 30).expect("trägt");
        let stellen: Vec<(PodId, u32)> = st.pruefauftraege.iter().map(|a| (a.pod, a.segment)).collect();
        assert_eq!(stellen, vec![(pa, 1), (pb, 0)], "je Pod die Stelle des Segments");
        assert!(st.pruefauftraege.iter().all(|a| a.pruefer.len() == 2 && a.epoche == 3));
        assert_eq!(differenz_melden(&mut st, 3, &pa, &pb, &beweis, 11, 20, 30), Err(Pruefungsfehler::Doppelt));
        // Ein schon gezogenes Segment bekommt keinen zweiten Auftrag.
        let offen = st.uneinigkeiten[0].stichprobe.clone();
        auftraege_anlegen(&mut st, &offen, &[(pa, 1), (pb, 0)], 20, 30);
        assert_eq!(st.pruefauftraege.len(), 2, "kein doppelter Auftrag");
        uneinigkeiten_aufraeumen(&mut st, 51);
        assert!(st.uneinigkeiten.is_empty());
        assert_eq!(differenz_melden(&mut st, 3, &pa, &pb, &beweis, 52, 60, 70), Err(Pruefungsfehler::KeinAuftrag));
    }

    /// ⚑ **Die Rückbuchung** (Whitepaper 6.4): Wird ein Pod überführt,
    /// verfällt sein Vorbehalt und der seines Zwillings wird am nächsten
    /// Wechsel ausgezahlt; ohne Urteil verfallen beide nach der Frist.
    #[test]
    fn der_ehrliche_zwilling_bekommt_seinen_lohn_nach_dem_urteil() {
        let (pa, pb) = (PodId::new([10; 32]), PodId::new([11; 32]));
        let vorbehalt = |pod: PodId, zwilling: PodId, wer: u8| Vorbehalt {
            epoche: 3,
            pod,
            zwilling,
            je_miner: vec![(m(wer), 100)],
            frei: false,
            bis_epoche: 6,
        };
        let mut st = LedgerState::genesis(1);
        st.vorbehalte = vec![vorbehalt(pa, pb, 1), vorbehalt(pb, pa, 2)];
        assert!(vorbehalte_faellig(&mut st, 4).is_empty(), "vor dem Urteil nichts");
        vorbehalt_entscheiden(&mut st, 3, &pb);
        assert_eq!(st.vorbehalte.len(), 1);
        assert_eq!(vorbehalte_faellig(&mut st, 4), vec![(m(1), 100)], "der Zwilling des Überführten");
        assert!(st.vorbehalte.is_empty());
        assert_eq!(st.rueckbuchungen, 1);
        // Ohne Urteil: nach der Frist verfallen beide, vorher bleiben sie.
        st.vorbehalte = vec![vorbehalt(pa, pb, 1), vorbehalt(pb, pa, 2)];
        assert!(vorbehalte_faellig(&mut st, 5).is_empty());
        assert_eq!(st.vorbehalte.len(), 2);
        assert!(vorbehalte_faellig(&mut st, 6).is_empty());
        assert!(st.vorbehalte.is_empty(), "verfallen");
    }
}
