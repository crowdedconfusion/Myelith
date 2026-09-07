//! Belege, mit denen ein Schuldspruch begründet wird.
//!
//! # ⚑ Warum diese Typen hier stehen und nicht in VERIFICATION
//!
//! Bis zum 2026-09-06 lagen sie in `myl-verifier`, und damit konnte der
//! Konsens sie nicht sehen: `myl-consensus` hängt nicht an VERIFICATION
//! und darf es auch nicht, denn dann hinge der Blockbau an der
//! Prüflogik.
//!
//! **Die Folge war Fund 192.** `Block::verdicts` trug einen Schuldspruch
//! **ohne Beleg**, weil der Beleg nicht auf den Draht passte. Ein
//! `Verdict` nennt Täter und Kopfgeldempfänger und sonst nichts; wer
//! ihn anwendete, gäbe dem Blockerzeuger ein Werkzeug, mit dem er jeden
//! schlachten kann.
//!
//! ⚑ **Hierher gehören sie, weil sie Wiretypen sind.** Sie bestehen
//! ausschliesslich aus Typen dieses Kistchens, und ihre Prüfung ist eine
//! Signaturprüfung, also genau die Art Regel, die `myl-types` ohnehin
//! trägt. VERIFICATION exportiert sie weiter; für seine Aufrufer ändert
//! sich nichts.

use borsh::{BorshDeserialize, BorshSerialize};

use crate::bls::{BlsPublicKey, BlsSignature};
use crate::challenge::Challenge;
use crate::ids::MinerId;
use crate::uebergang::{Rolle, TransitionSig};

/// Der Beleg, dass ein bestimmter Miner den strittigen Schritt selbst
/// gerechnet hat.
///
/// ⚑ **Er belegt die Urheberschaft, nicht die Schuld.** Dass die
/// Rechnung falsch war, entscheidet die Bisektion; dieser Beleg sagt
/// nur, **wen** es trifft. Ohne ihn konnte jeder benannt werden.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Schuldbeleg {
    /// Der unterschriebene Übergang.
    pub uebergang: TransitionSig,
    /// Der öffentliche Schlüssel des Unterzeichners.
    pub schluessel: BlsPublicKey,
    /// Seine Signatur über den Übergang in der Rolle [`Rolle::Shard`].
    pub signatur: BlsSignature,
}

impl Schuldbeleg {
    /// Wen der Beleg belastet: die aus dem Schlüssel abgeleitete Kennung.
    pub fn unterzeichner(&self) -> MinerId {
        MinerId::aus_schluessel(&self.schluessel)
    }

    /// Prüft die Signatur, und zwar ausdrücklich in der Rolle
    /// [`Rolle::Shard`].
    ///
    /// ⚑ **Die Rolle mitzuprüfen ist der Sinn der Rollenbindung.** Eine
    /// Unterschrift, die derselbe Miner als Pod-Mitglied oder Validator
    /// abgegeben hat, gilt hier nicht.
    pub fn ist_gueltig(&self) -> bool {
        self.uebergang
            .verify_mit_rolle(&self.schluessel, &self.signatur, Rolle::Shard)
    }
}

/// Der Beleg, dass ein bestimmter Miner die Anfechtung eingereicht hat.
///
/// ⚑ **Die andere Seite derselben Frage.** Verliert der Herausforderer,
/// wird er dafür geschlachtet, dass er falsch beschuldigt hat; auch das
/// muss belegt sein, sonst bestimmt der Schlachtende, wen es trifft.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Anfechtungsbeleg {
    /// Die unterschriebene Anfechtung.
    pub anfechtung: Challenge,
    /// Der öffentliche Schlüssel des Herausforderers.
    pub schluessel: BlsPublicKey,
}

impl Anfechtungsbeleg {
    /// Prüft Unterschrift und Zuordnung in einem.
    pub fn ist_gueltig(&self) -> bool {
        self.anfechtung.ist_vom_herausforderer(&self.schluessel)
    }
}

/// Der Nachweis, auf den ein Schuldspruch gestützt wird.
///
/// ⚑ **Der Nachweis trägt den Ausgang in sich.** Es gibt keinen Weg,
/// nach einem Schuldspruch zu fragen, ohne den Beleg mitzubringen, der
/// zu ihm gehört, und keinen, bei dem Ausgang und Beleg
/// auseinanderfallen. Nicht abgewiesen, sondern nicht hinschreibbar.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub enum Belegart {
    /// Der primäre Pod verliert: Er hat den strittigen Schritt selbst
    /// unterschrieben.
    PrimaerHatGerechnet(Schuldbeleg),
    /// Der Herausforderer verliert: Er hat die Anfechtung selbst
    /// unterschrieben.
    HerausfordererHatAngefochten(Anfechtungsbeleg),
}

impl Belegart {
    /// Trägt der Beleg sich selbst?
    pub fn ist_gueltig(&self) -> bool {
        match self {
            Self::PrimaerHatGerechnet(b) => b.ist_gueltig(),
            Self::HerausfordererHatAngefochten(b) => b.ist_gueltig(),
        }
    }
}
