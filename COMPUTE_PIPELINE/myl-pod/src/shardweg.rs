//! Wie der Koordinator einen Shard erreicht.
//!
//! # ⚑ Fund 186: Die Shards eines Pods liefen nie über einen Draht
//!
//! `PodMessage` ist seit Langem ein fertiges Format: borsh-serialisiert,
//! mit Rundlauftest. **Nur überquerte es nie eine Prozessgrenze.**
//! `Coordinator` rief `shard.process(&nachricht)` unmittelbar, und
//! `Pipelinewerk::laden` legte alle vier `ShardNode` in **einen**
//! Prozess.
//!
//! ⚑ **Der Beweis steht im Code selbst.** `Pipelinewerk::laden` leitet
//! vier BLS-Schlüssel aus `(s + 1) · 17` ab und schreibt dazu: „Für
//! einen echten Pod kommt der Schlüssel aus der Identität des Miners und
//! nicht von hier." **Ein Prozess, der vier Minerschlüssel hält, ist
//! kein Pod, sondern eine Maschine, die vier Rollen spielt.**
//!
//! Auch `zwei_prozesse.rs` widerlegt das nicht: Dort sind die zwei
//! Prozesse **Knoten und Poddienst**, und der Poddienst hält weiter alle
//! vier Shards. Der Draht lag zwischen Klient und Pod, nie zwischen den
//! Shards.
//!
//! ⚑ **Damit war die Kernaussage des Shardings unbelegt.** Die ganze
//! Konstruktion lebt davon, dass vier **verschiedene** Miner auf vier
//! **verschiedenen** Maschinen einen Pod bilden; was gemessen war, ist
//! eine Maschine, die dasselbe rechnet.
//!
//! # ⚑ Die Naht, und warum sie ein Merkmal ist und keine Verzweigung
//!
//! [`Shardweg`] hat zwei Umsetzungen und **eine** Bedeutung: „gib dieser
//! Nachricht den Shard *j* und bring mir, was herauskommt". Der
//! Koordinator kennt den Unterschied nicht.
//!
//! ⚑ **Deshalb bleibt jeder bestehende Test gültig.** Die Spurbildung,
//! die Unterschriften, die Reserveübernahme und die vTFE-Zuschreibung
//! rechnen unverändert; was sich ändert, ist allein, wo `process`
//! ausgeführt wird. Wäre es eine Verzweigung im Koordinator, gäbe es
//! zwei Wege durch dieselbe Logik und zwei Gelegenheiten,
//! auseinanderzulaufen.
//!
//! # ⚑ Stern und nicht Kette, und das ist eine Entscheidung
//!
//! Der Koordinator spricht **jeden** Shard selbst an, statt Shard *j*
//! an *j+1* weiterreichen zu lassen. Die Kette wäre sparsamer: halb so
//! viele Übertragungen.
//!
//! **Der Stern gewinnt trotzdem**, aus zwei Gründen:
//!
//! 1. **Die Ausfallbehandlung wohnt beim Koordinator.** Er kennt die
//!    Reserve, die Fristen und das Fenster (`standby.rs`, Kap. 6.8). In
//!    einer Kette müsste jeder Shard sie kennen, also läge dieselbe
//!    Regel viermal vor.
//! 2. ⚑ **Ein Shard, der weiterreicht, wählt seinen Nachfolger.** Damit
//!    könnte er die Pipeline umleiten, und die Spur bewiese nur noch,
//!    dass **irgendwer** gerechnet hat. Im Stern kennt kein Shard die
//!    Adresse eines anderen.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use crate::shard::{ShardNode, ShardOut};
use crate::wire::PodMessage;

/// Wie lange auf die Antwort eines Shards gewartet wird.
///
/// ⚑ **Ohne Frist hinge der ganze Pod an einem stillen Shard.** Der
/// Koordinator hat eine Reserve und ein Fenster; beide nützen nichts,
/// wenn er auf eine Antwort wartet, die nie kommt.
pub const ANTWORTFRIST: Duration = Duration::from_secs(30);

/// Grösste Nachricht, die angenommen wird.
///
/// ⚑ **Eine Längenangabe ohne Grenze ist eine Einladung**: Vier Bytes
/// mit `u32::MAX` liessen den Empfänger vier Gigabyte reservieren,
/// bevor er ein einziges Nutzbyte gesehen hat.
pub const MAX_NACHRICHT: usize = 64 * 1024 * 1024;

/// Wie viele Positionen ein Trainings-Vorwaertspass hoechstens traegt.
///
/// # ⚑ Warum die Laengengrenze der Nachricht dafuer nicht reicht
///
/// [`MAX_NACHRICHT`] begrenzt, was **hereinkommt**. Ein Mitschnitt ist
/// aber ein Vielfaches davon: Er haelt je Position und je Ebene mehrere
/// Zwischenwerte. Bei 896 Kanaelen und sechs Ebenen je Shard kostet
/// eine Position rund hundert Kilobyte, und vierundsechzig Megabyte
/// Eingabe reichen fuer etwa siebenunddreissigtausend Positionen: **vier
/// Gigabyte Mitschnitt aus vierundsechzig Megabyte Nachricht.**
///
/// ⚑ **Das ist eine Verstaerkung und damit eine eigene Klasse.** Wer
/// nur die Eingabe begrenzt, begrenzt nicht den Speicher; die Grenze
/// gehoert dorthin, wo der Speicher entsteht.
///
/// Viertausendsechsundneunzig ist grosszuegig gegen jede Fenstergroesse,
/// die dieses Projekt misst, und klein gegen den Arbeitsspeicher eines
/// Shards.
pub const MAX_TRAINPOSITIONEN: usize = 4096;

/// Wie viele Trainingssitzungen ein Shard gleichzeitig haelt.
///
/// ⚑ **Ein Mitschnitt bleibt liegen, bis der Rueckwaertspass kommt.**
/// Wer nur Vorwaertspaesse schickt und nie zurueck, haeuft Mitschnitte
/// an, ohne je etwas zu rechnen. Ein Pod faehrt seine Folgen
/// nacheinander; mehr als eine Handvoll offener Sitzungen gibt es im
/// ehrlichen Betrieb nicht.
pub const MAX_TRAINSITZUNGEN: usize = 8;

/// Was ein Koordinator von einem Shard verlangen kann.
///
/// # ⚑ Warum es mehr als „rechne" sein muss
///
/// Der Koordinator baut ein PoI-Bündel, und darin steht eine
/// **Aggregatsignatur aller Mitglieder**. Die kann er nicht selbst
/// bilden: Sie verlangt die **privaten** Schlüssel der Shards. Solange
/// alle vier in einem Prozess lagen, fiel das nicht auf; über einen
/// Draht ist es die zweite Nachricht, die es braucht.
///
/// ⚑ **Und die Unterschrift ist keine Formsache.** Ein Shard prüft die
/// beanspruchten vTFE nach, bevor er unterschreibt (Fund 52); wer sie
/// beim Koordinator bildete, nähme dem Pod genau diese Prüfung.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum Shardanfrage {
    /// Rechne diese Nachricht.
    Rechne(PodMessage),
    /// Welchen Layerbereich hältst du?
    Zuschnitt,
    /// Wie lautet dein öffentlicher Schlüssel?
    Schluessel,
    /// Unterschreibe dieses Bündel, wenn du es nachrechnen kannst.
    Unterschreibe {
        /// Die Botschaft des Bündels.
        botschaft: Vec<u8>,
        /// Die beanspruchten vTFE.
        vtfe: u64,
        /// Über wie viele Segmente.
        segmente: u64,
        /// Der Zuschnitt aller Pod-Mitglieder.
        zuschnitte: Vec<myl_tokenomics::ShardZuschnitt>,
    },
    /// Vergiss diese Sitzung.
    SitzungVergessen(u64),
    /// Der Dekodier-Abdruck dieser Sitzung.
    DekodierDigest(u64),
    /// Das Modellprofil, für die vTFE-Rechnung.
    Profil,
    /// Wie viele Sitzungen der Shard gerade hält.
    GehalteneSitzungen,
    /// Rechne den Trainings-Vorwärtspass über deinen Layerbereich.
    ///
    /// ⚑ **Der Mitschnitt bleibt beim Shard.** Er ist gross, er wird nur
    /// von diesem Shard gebraucht, und er über den Draht zu schicken
    /// hiesse, ihn zweimal zu halten. Die Sitzungsnummer holt ihn beim
    /// Rückwärtspass wieder hervor.
    TrainVorwaerts {
        /// Wozu der Mitschnitt gehört.
        sitzung: u64,
        /// Der Residualstrom vor dem eigenen Bereich, je Position.
        hidden: Vec<Vec<i16>>,
        /// Der Schrittzähler für den Würfel.
        schritt: u64,
        /// Der Nenner der Lernrate.
        lr_nenner: i64,
    },
    /// Rechne den Rückwärtspass und **sammle**, ohne fortzuschreiben.
    ///
    /// ⚑ **Gesammelt und nicht angewandt**, aus dem Grund, der in
    /// `optimierer::sammle` steht: Über wenige Schritte streut das
    /// stochastische Runden stärker, als das Gradientensignal wiegt.
    TrainRueckwaerts {
        /// Welche Sitzung.
        sitzung: u64,
        /// Der Gradient am Ausgang des eigenen Bereichs.
        g_aus: Vec<Vec<i32>>,
        /// Der Nenner der Lernrate, **derselbe wie vorwärts**.
        ///
        /// ⚑ **Er steht hier, damit eine Abweichung ein Fehler ist und
        /// keine stille Fehlrechnung.** Ihn wegzulassen hiesse, ihn auf
        /// der Gegenseite zu raten.
        lr_nenner: i64,
    },
    /// Wende an, was gesammelt wurde: **ein** Wurf je Gewicht.
    TrainAnwenden {
        /// Der Schrittzähler des Segments.
        schritt: u64,
        /// Der Nenner der Lernrate.
        lr_nenner: i64,
    },
}

/// Was ein Shard darauf antwortet.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum Shardantwort {
    /// Das Ergebnis einer Rechnung.
    Gerechnet(ShardOut),
    /// Der Layerbereich.
    Zuschnitt(myl_tokenomics::ShardZuschnitt),
    /// Der öffentliche Schlüssel.
    Schluessel(myl_types::bls::BlsPublicKey),
    /// Die Unterschrift.
    Unterschrift(myl_types::bls::BlsSignature),
    /// Der Abdruck, falls es einen gibt.
    DekodierDigest(Option<(String, u64)>),
    /// Das Modellprofil.
    Profil(myl_tokenomics::vtfe::ModellProfil),
    /// Eine Zahl.
    Zahl(u64),
    /// Erledigt, ohne Rückgabe.
    Erledigt,
    /// Der Residualstrom nach dem eigenen Bereich.
    TrainAusgang(Vec<Vec<i16>>),
    /// Der Gradient am Eingang des eigenen Bereichs, mit seiner Skala.
    TrainEingang {
        /// Der Gradient je Position.
        eingang: Vec<Vec<i32>>,
        /// Die Bruchstellen des Gradienten je Kanal.
        eingang_frac: Vec<u8>,
    },
    /// Was der Sammelschritt bewegt hat.
    TrainAngewandt {
        /// Abdruck über die Deltas dieses Shards.
        delta_commitment: String,
        /// Wie viele Master sich bewegt haben.
        bewegte: u64,
        /// Wie viele Master der Shard hält.
        gesamt: u64,
        /// Welche globale Ebene die Übertragungsform verlassen hat.
        aus_der_form: Option<u64>,
    },
    /// Der Shard lehnt ab, mit Begründung.
    ///
    /// ⚑ **Ein Fehler ist eine Antwort und kein Abbruch.** Wer die
    /// Verbindung stattdessen fallen liesse, wäre von einem
    /// ausgefallenen Shard nicht zu unterscheiden, und der Koordinator
    /// zöge die Reserve, obwohl der Shard antwortet.
    Fehler(String),
}

/// Wie der Koordinator einen Shard erreicht.
pub trait Shardweg: Send + Sync {
    /// Wie viele Shards der Pod hat.
    fn shardzahl(&self) -> usize;

    /// Stellt Shard `nummer` eine Anfrage.
    fn frage(&self, nummer: usize, anfrage: &Shardanfrage) -> Result<Shardantwort, String>;

    /// Gibt Shard `nummer` die Nachricht und liefert, was herauskommt.
    fn rechne(&self, nummer: usize, nachricht: &PodMessage) -> Result<ShardOut, String> {
        match self.frage(nummer, &Shardanfrage::Rechne(nachricht.clone()))? {
            Shardantwort::Gerechnet(aus) => Ok(aus),
            Shardantwort::Fehler(e) => Err(e),
            andere => Err(format!("Shard {nummer} antwortete mit {andere:?} statt einer Rechnung")),
        }
    }
}

/// Die Trainingshälfte eines Shards.
///
/// # ⚑ Warum getrennt vom [`ShardNode`]
///
/// Der Inferenzknoten hält das Modell und einen KV-Cache je Sitzung; er
/// braucht keine Master und keinen Mitschnitt. Ein Shard, der nur
/// rechnet, soll auch nur so viel Speicher belegen: Die Master eines
/// Bereichs sind viermal so gross wie seine Gewichte, und bei einer
/// Gemischebene kämen die gewählten Experten dazu.
///
/// **Getrennt heisst deshalb: wer nicht trainiert, zahlt nicht dafür.**
pub struct Shardtrainer {
    modell: Arc<integer_llm_runtime::model::IntegerModel>,
    von: usize,
    bis: usize,
    gewichte: integer_llm_runtime::shardtraining::Shardgewichte,
    /// Der Mitschnitt je Sitzung **mit der Lernrate**, unter der er
    /// entstanden ist.
    ///
    /// ⚑ **Die Rate gehört dazu, und das hat ein Test gelehrt.** Der
    /// erste Entwurf gab dem Rückwärtspass `lr_nenner = 1`, weil die
    /// Anfrage sie nicht trug. Über vier Prozesse bewegten sich dann
    /// 351 statt 237 Millionen Gewichte, und ohne den Vergleich gegen
    /// den Einprozesslauf wäre das nicht aufgefallen: Beide Zahlen
    /// sehen plausibel aus.
    mitschnitte: std::collections::HashMap<
        u64,
        (integer_llm_runtime::shardtraining::Shardmitschnitt, i64),
    >,
    /// Was bisher gesammelt wurde.
    sammlung: integer_llm_runtime::shardtraining::Sammlung,
}

impl Shardtrainer {
    /// Legt die Trainingshälfte für einen Layerbereich an.
    pub fn neu(
        modell: Arc<integer_llm_runtime::model::IntegerModel>,
        von: usize,
        bis: usize,
    ) -> Result<Self, String> {
        let gewichte =
            integer_llm_runtime::shardtraining::Shardgewichte::aus_modell(&modell, von, bis)
                .map_err(|e| format!("Shardgewichte {von}..{bis}: {e}"))?;
        Ok(Self {
            modell,
            von,
            bis,
            gewichte,
            mitschnitte: std::collections::HashMap::new(),
            // ⚑ **Der Pod sammelt normiert, und das ist die
            // Protokollrechnung** (Fund 194, geschlossen 2026-09-06).
            // Ohne Normierung bedeutet eine Lernrate auf jedem Modell
            // etwas anderes: Qwen3-4B bewegte bei jeder zulässigen Rate
            // **kein einziges Gewicht**, während Qwen2.5-0,5B bei
            // derselben Zahl lernte.
            sammlung: integer_llm_runtime::shardtraining::Sammlung::normiert(),
        })
    }

    fn vorgaben(&self, schritt: u64, lr_nenner: i64) -> integer_llm_runtime::shardtraining::Shardvorgaben {
        integer_llm_runtime::shardtraining::Shardvorgaben {
            von: self.von,
            bis: self.bis,
            schritt,
            lr_zaehler: 1,
            lr_nenner,
        }
    }
}

/// Bedient eine Trainingsanfrage.
///
/// ⚑ **Getrennte Funktion und kein zweiter Weg.** Sie tut dasselbe wie
/// ein Direktaufruf von `shardtraining`; die Alternative wäre gewesen,
/// die Schritte hier noch einmal zu schreiben, und dann wäre eine
/// Abweichung zwischen Draht und Direktaufruf nicht von einem
/// Rechenfehler zu unterscheiden.
pub fn bedienen_training(
    trainer: &mut Shardtrainer,
    anfrage: &Shardanfrage,
) -> Option<Shardantwort> {
    use integer_llm_runtime::shardtraining::{
        rueckwaerts_mit, sammlung_anwenden, vorwaerts, Fortschreibung,
    };
    match anfrage {
        Shardanfrage::TrainVorwaerts { sitzung, hidden, schritt, lr_nenner } => {
            // ⛑ **Zwei Grenzen, bevor irgendetwas belegt wird.**
            if hidden.len() > MAX_TRAINPOSITIONEN {
                return Some(Shardantwort::Fehler(format!(
                    "{} Positionen ueberschreiten die Grenze von {MAX_TRAINPOSITIONEN}",
                    hidden.len()
                )));
            }
            if !trainer.mitschnitte.contains_key(sitzung)
                && trainer.mitschnitte.len() >= MAX_TRAINSITZUNGEN
            {
                return Some(Shardantwort::Fehler(format!(
                    "{MAX_TRAINSITZUNGEN} offene Trainingssitzungen: erst zurueckrechnen"
                )));
            }
            let v = trainer.vorgaben(*schritt, *lr_nenner);
            match vorwaerts(&trainer.modell.clone(), &mut trainer.gewichte, &v, hidden) {
                Ok(ms) => {
                    let ausgang = ms.ausgang.clone();
                    trainer.mitschnitte.insert(*sitzung, (ms, *lr_nenner));
                    Some(Shardantwort::TrainAusgang(ausgang))
                }
                Err(e) => Some(Shardantwort::Fehler(format!("Vorwaerts: {e}"))),
            }
        }
        Shardanfrage::TrainRueckwaerts { sitzung, g_aus, lr_nenner } => {
            // ⚑ **Der Mitschnitt wird entnommen und nicht kopiert.** Ein
            // zweiter Rückwärtspass über dieselbe Sitzung wäre ein
            // zweiter Beitrag zur Sammlung aus **einer** Rechnung; wer
            // ihn zuliesse, könnte seinen eigenen Gradienten
            // vervielfachen.
            let Some((ms, rate)) = trainer.mitschnitte.remove(sitzung) else {
                return Some(Shardantwort::Fehler(format!(
                    "keine Sitzung {sitzung}: erst TrainVorwaerts, dann TrainRueckwaerts"
                )));
            };
            // ⚑ **Dieselbe Rate wie vorwärts, sonst gar keine.** Eine
            // abweichende Rate ergibt einen anderen Gewichtsstand, und
            // der Redundanzvergleich meldete zwei ehrliche Pods als
            // uneinig.
            if rate != *lr_nenner {
                return Some(Shardantwort::Fehler(format!(
                    "Sitzung {sitzung} lief vorwaerts mit Nenner {rate}, rueckwaerts \
                     kommt {lr_nenner}"
                )));
            }
            let v = trainer.vorgaben(0, rate);
            let modell = trainer.modell.clone();
            let mut sammlung =
                std::mem::take(&mut trainer.sammlung);
            let erg = rueckwaerts_mit(
                &modell,
                &mut trainer.gewichte,
                &v,
                &ms,
                g_aus,
                &mut Fortschreibung::Sammeln(&mut sammlung),
            );
            sammlung.folge_fertig();
            trainer.sammlung = sammlung;
            match erg {
                Ok(e) => Some(Shardantwort::TrainEingang {
                    eingang: e.eingang,
                    eingang_frac: e.eingang_frac,
                }),
                Err(e) => Some(Shardantwort::Fehler(format!("Rueckwaerts: {e}"))),
            }
        }
        Shardanfrage::TrainAnwenden { schritt, lr_nenner } => {
            let v = trainer.vorgaben(*schritt, *lr_nenner);
            let erg = sammlung_anwenden(&trainer.sammlung, &mut trainer.gewichte, &v);
            trainer.sammlung = integer_llm_runtime::shardtraining::Sammlung::normiert();
            Some(Shardantwort::TrainAngewandt {
                delta_commitment: erg.delta_commitment,
                bewegte: erg.bewegte_gewichte as u64,
                gesamt: erg.gewichte_gesamt as u64,
                aus_der_form: erg.aus_der_form.map(|e| e as u64),
            })
        }
        _ => None,
    }
}

/// Bedient eine Anfrage mit einem geladenen Shard.
///
/// ⚑ **Eine Stelle für beide Wege.** Der Prozessweg und der
/// Direktaufruf müssen dasselbe tun; täte es jeder für sich, wäre eine
/// Abweichung zwischen ihnen nicht von einem Rechenfehler zu
/// unterscheiden.
pub fn bedienen(shard: &ShardNode, anfrage: &Shardanfrage) -> Shardantwort {
    match anfrage {
        Shardanfrage::Rechne(m) => {
            if !m.is_valid_frame() {
                return Shardantwort::Fehler(
                    "die Nachricht traegt nicht die Magic-Bytes".to_string(),
                );
            }
            match shard.process(m) {
                Ok(a) => Shardantwort::Gerechnet(a),
                Err(e) => Shardantwort::Fehler(e),
            }
        }
        Shardanfrage::Zuschnitt => Shardantwort::Zuschnitt(shard.zuschnitt()),
        Shardanfrage::Schluessel => Shardantwort::Schluessel(shard.public_key()),
        Shardanfrage::Unterschreibe { botschaft, vtfe, segmente, zuschnitte } => {
            match shard.signiere_buendel(botschaft, *vtfe, *segmente, zuschnitte) {
                Ok(s) => Shardantwort::Unterschrift(s),
                Err(e) => Shardantwort::Fehler(e),
            }
        }
        Shardanfrage::SitzungVergessen(id) => {
            shard.sitzung_vergessen(*id);
            Shardantwort::Erledigt
        }
        Shardanfrage::DekodierDigest(id) => match shard.dekodier_digest(*id) {
            Ok(d) => Shardantwort::DekodierDigest(d.map(|(h, s)| (h, s as u64))),
            Err(e) => Shardantwort::Fehler(e),
        },
        Shardanfrage::Profil => Shardantwort::Profil(shard.modell_profil()),
        Shardanfrage::GehalteneSitzungen => {
            Shardantwort::Zahl(shard.gehaltene_sitzungen() as u64)
        }
        // ⚑ **Ein Shard ohne Trainingshälfte lehnt ab, statt zu
        // schweigen.** Ein Koordinator, der keine Antwort bekommt, kann
        // „kann nicht" nicht von „ist ausgefallen" unterscheiden und
        // zöge die Reserve.
        Shardanfrage::TrainVorwaerts { .. }
        | Shardanfrage::TrainRueckwaerts { .. }
        | Shardanfrage::TrainAnwenden { .. } => Shardantwort::Fehler(
            "dieser Shard haelt keine Trainingsgewichte (ohne --training gestartet)".to_string(),
        ),
    }
}

/// Die **festen** Angaben eines Pods, einmal erhoben.
///
/// # ⚑ Einmal fragen und nicht bei jedem Auftrag
///
/// Zuschnitt und Modellprofil ändern sich nicht. Sie über den Draht
/// wiederholt zu erfragen kostete nicht nur Zeit: **Ein Shard könnte
/// beim zweiten Mal etwas anderes sagen**, und der Koordinator rechnete
/// die vTFE eines Auftrags gegen einen anderen Zuschnitt als den, mit
/// dem er ihn zugeteilt hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Podbesetzung {
    /// Je Shard sein Layerbereich, in Shardreihenfolge.
    pub zuschnitte: Vec<myl_tokenomics::ShardZuschnitt>,
    /// Das Modellprofil, von allen Shards dasselbe.
    pub profil: myl_tokenomics::vtfe::ModellProfil,
}

impl Podbesetzung {
    /// Erhebt die festen Angaben über den gegebenen Weg.
    ///
    /// ⚑ **Alle Shards müssen dasselbe Profil nennen.** Zwei Profile
    /// heissen zwei Modelle, und ein Pod mit zwei Modellen rechnet
    /// nichts, was sich nachrechnen liesse.
    pub fn erheben(weg: &dyn Shardweg) -> Result<Self, String> {
        let mut zuschnitte = Vec::with_capacity(weg.shardzahl());
        let mut profil: Option<myl_tokenomics::vtfe::ModellProfil> = None;
        for j in 0..weg.shardzahl() {
            match weg.frage(j, &Shardanfrage::Zuschnitt)? {
                Shardantwort::Zuschnitt(z) => zuschnitte.push(z),
                andere => return Err(format!("Shard {j} nannte keinen Zuschnitt: {andere:?}")),
            }
            match weg.frage(j, &Shardanfrage::Profil)? {
                Shardantwort::Profil(p) => match profil {
                    None => profil = Some(p),
                    Some(erstes) if erstes == p => {}
                    Some(_) => {
                        return Err(format!(
                            "Shard {j} nennt ein anderes Modellprofil als Shard 0"
                        ))
                    }
                },
                andere => return Err(format!("Shard {j} nannte kein Profil: {andere:?}")),
            }
        }
        let profil = profil.ok_or_else(|| "ein Pod ohne Shards hat kein Profil".to_string())?;
        Ok(Self { zuschnitte, profil })
    }
}

/// Alle Shards in diesem Prozess.
///
/// ⚑ **Für Prüfläufe und den Einzelbetrieb, nicht für ein Netz.** Wer so
/// fährt, hält alle Schlüssel des Pods; das ist kein Pod, sondern eine
/// Maschine, die mehrere Rollen spielt. Der Name sagt es, damit niemand
/// eine Messung damit für eine Aussage über verteilte Pods hält.
pub struct ImProzess {
    shards: Vec<Arc<ShardNode>>,
}

impl ImProzess {
    /// Neu aus den geladenen Shards.
    pub fn neu(shards: Vec<Arc<ShardNode>>) -> Self {
        Self { shards }
    }

    /// Die Shards, für Aufrufer, die mehr als das Rechnen brauchen
    /// (Signaturen, Zuschnitt, Sitzungsverwaltung).
    pub fn shards(&self) -> &[Arc<ShardNode>] {
        &self.shards
    }
}

impl Shardweg for ImProzess {
    fn shardzahl(&self) -> usize {
        self.shards.len()
    }

    fn frage(&self, nummer: usize, anfrage: &Shardanfrage) -> Result<Shardantwort, String> {
        let shard = self
            .shards
            .get(nummer)
            .ok_or_else(|| format!("Shard {nummer} gibt es in diesem Pod nicht"))?;
        Ok(bedienen(shard, anfrage))
    }
}

/// Jeder Shard ein eigener Prozess, erreichbar über TCP.
///
/// ⚑ **Die Adressen kommen von aussen und werden nicht gefunden.** Wer
/// sie sucht, sucht sie irgendwo; ein Pod, dessen Besetzung aus einer
/// Suche entsteht, ist nicht die Besetzung, die der Scheduler zugeteilt
/// hat. Sie stammen aus `MinerRegistration::netzadresse` der zugeteilten
/// Mitglieder, in Shardreihenfolge.
pub struct UeberDenDraht {
    adressen: Vec<SocketAddr>,
    frist: Duration,
}

impl UeberDenDraht {
    /// Neu, mit einer Adresse je Shard in Shardreihenfolge.
    pub fn neu(adressen: Vec<SocketAddr>) -> Self {
        Self { adressen, frist: ANTWORTFRIST }
    }

    /// Mit abweichender Frist, für Prüfläufe.
    pub fn mit_frist(mut self, frist: Duration) -> Self {
        self.frist = frist;
        self
    }
}

impl Shardweg for UeberDenDraht {
    fn shardzahl(&self) -> usize {
        self.adressen.len()
    }

    fn frage(&self, nummer: usize, anfrage: &Shardanfrage) -> Result<Shardantwort, String> {
        let adresse = self
            .adressen
            .get(nummer)
            .ok_or_else(|| format!("Shard {nummer} gibt es in diesem Pod nicht"))?;
        let mut strom = TcpStream::connect_timeout(adresse, self.frist)
            .map_err(|e| format!("Shard {nummer} unter {adresse} nicht erreichbar: {e}"))?;
        strom.set_read_timeout(Some(self.frist)).map_err(|e| e.to_string())?;
        strom.set_write_timeout(Some(self.frist)).map_err(|e| e.to_string())?;
        senden(&mut strom, &borsh::to_vec(anfrage).map_err(|e| e.to_string())?)?;
        let roh = empfangen(&mut strom)?;
        borsh::from_slice(&roh).map_err(|e| format!("Shard {nummer} antwortete unlesbar: {e}"))
    }
}

/// Ein Shard als Dienst: **einer**, mit **einem** Schlüssel.
///
/// # ⚑ Was dieser Typ zusichert, und es ist der Punkt der ganzen Sache
///
/// Er hält **genau einen** [`ShardNode`]. Wer ihn betreibt, hat den
/// Schlüssel dieses einen Miners und die Gewichte dieses einen
/// Layerbereichs, und sonst nichts vom Pod. **Das ist der Unterschied zu
/// `Pipelinewerk`**, das vier Schlüssel in einem Prozess hält.
///
/// ⚑ **Er kennt keine anderen Shards.** Kein Feld nennt eine Adresse,
/// also kann er nicht weiterreichen und nicht umleiten. Er antwortet dem,
/// der fragt, und das ist der Koordinator.
pub struct Shardstelle {
    shard: Arc<ShardNode>,
    horcher: std::net::TcpListener,
    frist: Duration,
    /// Die Trainingshälfte, falls dieser Shard eine hält.
    ///
    /// ⚑ **`Option` und nicht immer.** Die Master eines Bereichs sind
    /// viermal so gross wie seine Gewichte; ein Shard, der nur Inferenz
    /// fährt, soll sie nicht halten.
    trainer: Option<std::sync::Mutex<Shardtrainer>>,
}

impl Shardstelle {
    /// Öffnet die Tür dieses Shards.
    ///
    /// `adresse` darf Port null nennen; die vergebene Adresse steht dann
    /// in [`Shardstelle::adresse`].
    pub fn oeffnen(shard: Arc<ShardNode>, adresse: &str) -> std::io::Result<Self> {
        let horcher = std::net::TcpListener::bind(adresse)?;
        Ok(Self { shard, horcher, frist: ANTWORTFRIST, trainer: None })
    }

    /// Öffnet die Tür **mit** Trainingshälfte.
    ///
    /// ⚑ **Der Layerbereich kommt aus dem Shard und nicht als Argument.**
    /// Zwei Angaben desselben Bereichs liefen auseinander, und dann
    /// trainierte ein Shard Ebenen, die er nicht rechnet.
    pub fn oeffnen_mit_training(
        shard: Arc<ShardNode>,
        modell: Arc<integer_llm_runtime::model::IntegerModel>,
        adresse: &str,
    ) -> Result<Self, String> {
        let horcher = std::net::TcpListener::bind(adresse).map_err(|e| e.to_string())?;
        let trainer = Shardtrainer::neu(modell, shard.layer_start, shard.layer_end)?;
        Ok(Self {
            shard,
            horcher,
            frist: ANTWORTFRIST,
            trainer: Some(std::sync::Mutex::new(trainer)),
        })
    }

    /// Unter welcher Adresse sie erreichbar ist.
    pub fn adresse(&self) -> std::io::Result<SocketAddr> {
        self.horcher.local_addr()
    }

    /// Bedient **eine** Anfrage.
    ///
    /// ⚑ **Nacheinander und nicht nebenläufig.** Ein Shard hält einen
    /// KV-Cache je Sitzung; zwei gleichzeitige Aufträge stritten um
    /// denselben und wären langsamer als nacheinander. Dasselbe Argument
    /// wie beim `Ortsdienst`.
    pub fn bediene_eine(&self) -> Result<(), String> {
        let (mut strom, _) = self.horcher.accept().map_err(|e| e.to_string())?;
        strom.set_read_timeout(Some(self.frist)).map_err(|e| e.to_string())?;
        strom.set_write_timeout(Some(self.frist)).map_err(|e| e.to_string())?;
        let roh = empfangen(&mut strom)?;
        let anfrage: Shardanfrage =
            borsh::from_slice(&roh).map_err(|e| format!("unlesbare Anfrage: {e}"))?;
        // ⚑ **Erst das Training, dann die Inferenz.** `bedienen_training`
        // gibt `None` für alles, was es nicht betrifft, und dann geht die
        // Anfrage den gewohnten Weg. Eine Tür, zwei Hälften.
        let antwort = match self.trainer.as_ref() {
            Some(sperre) => {
                let mut t = sperre.lock().unwrap_or_else(|v| v.into_inner());
                bedienen_training(&mut t, &anfrage)
                    .unwrap_or_else(|| bedienen(&self.shard, &anfrage))
            }
            None => bedienen(&self.shard, &anfrage),
        };
        senden(&mut strom, &borsh::to_vec(&antwort).map_err(|e| e.to_string())?)?;
        Ok(())
    }

    /// Bedient, bis der Prozess endet.
    pub fn laufen(&self) {
        loop {
            if let Err(e) = self.bediene_eine() {
                eprintln!("[myl-shard] {e}");
            }
        }
    }
}

/// Längenpräfix und Nutzlast.
fn senden(strom: &mut TcpStream, nutzlast: &[u8]) -> Result<(), String> {
    if nutzlast.len() > MAX_NACHRICHT {
        return Err(format!("Nachricht mit {} Bytes ist zu gross", nutzlast.len()));
    }
    let laenge = (nutzlast.len() as u32).to_le_bytes();
    strom.write_all(&laenge).map_err(|e| e.to_string())?;
    strom.write_all(nutzlast).map_err(|e| e.to_string())?;
    strom.flush().map_err(|e| e.to_string())
}

/// Liest ein Längenpräfix und danach genau so viele Bytes.
fn empfangen(strom: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut kopf = [0u8; 4];
    strom.read_exact(&mut kopf).map_err(|e| e.to_string())?;
    let laenge = u32::from_le_bytes(kopf) as usize;
    // ⚑ **Vor dem Reservieren prüfen**, siehe [`MAX_NACHRICHT`].
    if laenge > MAX_NACHRICHT {
        return Err(format!("angekuendigte Laenge {laenge} ueberschreitet die Grenze"));
    }
    let mut roh = vec![0u8; laenge];
    strom.read_exact(&mut roh).map_err(|e| e.to_string())?;
    Ok(roh)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⚑ **Eine angekündigte Länge über der Grenze wird abgewiesen,
    /// bevor reserviert wird.**
    #[test]
    fn eine_zu_grosse_laengenangabe_wird_abgewiesen() {
        let horcher = std::net::TcpListener::bind("127.0.0.1:0").expect("binden");
        let adresse = horcher.local_addr().expect("Adresse");
        let faden = std::thread::spawn(move || {
            let (mut strom, _) = horcher.accept().expect("annehmen");
            empfangen(&mut strom)
        });
        let mut strom = TcpStream::connect(adresse).expect("verbinden");
        strom.write_all(&u32::MAX.to_le_bytes()).expect("Kopf");
        let _ = strom.flush();
        let ergebnis = faden.join().expect("Faden");
        assert!(ergebnis.is_err(), "vier Gigabyte wurden reserviert");
    }

    /// Rundlauf: was gesendet wird, kommt an.
    #[test]
    fn der_rahmen_traegt_die_nutzlast() {
        let horcher = std::net::TcpListener::bind("127.0.0.1:0").expect("binden");
        let adresse = horcher.local_addr().expect("Adresse");
        let faden = std::thread::spawn(move || {
            let (mut strom, _) = horcher.accept().expect("annehmen");
            empfangen(&mut strom).expect("empfangen")
        });
        let mut strom = TcpStream::connect(adresse).expect("verbinden");
        senden(&mut strom, b"myelith").expect("senden");
        assert_eq!(faden.join().expect("Faden"), b"myelith");
    }
}
