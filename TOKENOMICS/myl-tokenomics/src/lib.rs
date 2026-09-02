//! `myl-tokenomics` — Tokenomik-Berechnungen (Whitepaper Kap. 5).
//!
//! Diese Komponente liefert die konkreten Formeln, die CONSENSUS'
//! generische Ledger-Zustandsübergänge (`burn`, `settle_epoch`,
//! `apply_verdict`) mit Zahlen füllt.
//!
//! Konsens-Grundregeln (Design-Entscheidung 2026-08-13):
//! - **Fixed-Point statt Gleitkomma:** Jede Formel hier ist Teil eines
//!   Ledger-Zustandsübergangs und muss auf jedem Node bitgleich
//!   nachrechenbar sein — derselbe Grund, aus dem INTEGER_LLM auf
//!   Gleitkomma verzichtet (Kap. 6.2). Ein `f64`-`exp()` kann bei
//!   Rundungsdifferenzen zu verschiedenen Ledger-Zuständen führen
//!   (Konsens-Bruch); deshalb: ganzzahlige Approximationen (LUT-basiert
//!   für Nichtlinearitäten, Brüche als Zähler/Nenner-Paare).
//! - **Skalierungen:** 1 MYL = 10⁶ Kleinstbeträge (`u64`);
//!   1 vTFE-Einheit = 10⁻⁶ Token-Forward-Äquivalent (`u64`).
//! - **Rundungsrichtung** wird je Formel dokumentiert (meist floor —
//!   „es wird niemals mehr verteilt/geprägt als gedeckt ist").
//! - **Überlaufsicherheit:** Zwischenrechnungen in `u128`/`i128`,
//!   Ergebnisse mit expliziter Bereichsprüfung.

#![deny(unsafe_code)]

pub mod anlauf;
pub mod ausschuettung;
pub mod boden;
pub mod distribute;
pub mod ema;
pub mod exp_approx;
pub mod genesis;
pub mod exp_lut_table;
pub mod mint;
pub mod sicherheit;
pub mod slashing;
pub mod speicherentgelt;
pub mod stake;
pub mod subventionsplan;
pub mod training;
pub mod utilization;
pub mod vtfe;
pub mod zuschreibung;

/// Anzahl der Kleinstbeträge je MYL (1 MYL = 10⁶ Kleinstbeträge).
pub const UNITS_PER_MYL: u64 = 1_000_000;

/// vTFE-Skalierung: eine vTFE-Einheit entspricht 10⁻⁶
/// Token-Forward-Äquivalenten (symmetrisch zu [`UNITS_PER_MYL`]).
pub const VTFE_UNITS_PER_TFE: u64 = 1_000_000;

pub use anlauf::{
    kleinste_ausreichende_rate, stufe as anlaufstufe, trainingsrate, Anlaufstufe,
    TRAININGSRATE_FAKTOR,
};
pub use boden::{
    auslastungsluecke, bodenbedarf, einkommen_traegt, liveness_verletzt, reichweite,
    Bodenbedarf, Deckel, Reichweite, AUSLASTUNGSBODEN,
};
pub use subventionsplan::{
    basis_halbierung_je_jahr, Erstjahresanteil, Planabschnitt, Planfehler, Subventionsplan,
};
pub use ausschuettung::{
    epochenausschuettung, Ausgelassen, Ausschuettung, Ausschuettungsfehler, Auslassungsgrund,
    Empfaengerklasse,
};
pub use distribute::{
    distribute_mint, redundancy_normalized_weight, split_proportional, Distribution,
    DistributeError, SHARE_CHECKERS_BPS, SHARE_COORDINATORS_BPS, SHARE_SHARD_MINERS_BPS,
    SHARE_TREASURY_BPS, SHARE_VALIDATORS_BPS, SHARES_TOTAL_BPS,
};
pub use ema::{ema_update, epochenabschluss_burn, Abschlussfehler, EMA_ALPHA_DEN, EMA_ALPHA_NUM};
pub use exp_approx::{
    exp_approx, update_price, update_price_mit_untergrenze, PREIS_UNTERGRENZE_VORGABE,
};
pub use genesis::{genesis_verteilung, Arbeitsnachweis, GenesisFehler, GenesisVerteilung};
pub use mint::{mint_amount, MintParams};
pub use sicherheit::{
    s_min, self_dealing_grenze, self_dealing_sicher, self_dealing_sicher_konservativ,
    stake_genuegt, SicherheitsFehler, KOSTENANTEIL_UNTEN_NENNER, KOSTENANTEIL_UNTEN_ZAEHLER,
};
pub use sicherheit::{burn_spielraum, BURN_DECKEL_AB, BURN_DECKEL_NENNER, BURN_DECKEL_ZAEHLER};
pub use slashing::{
    matrix as slashing_matrix, satz as slashing_satz, satz_aus_ledger, satz_gestaffelt,
    urteil_buchen_gestaffelt, Akteur, Grund, SlashBuchungFehler, Slashsatz,
    WIEDERHOLUNGSFENSTER,
};
pub use speicherentgelt::{
    byte_epochen, SPEICHERSATZ_VORGABE, SPEICHER_KOSTENBODEN,
};
pub use stake::{erforderlicher_stake, getragene_kapazitaet, StakeAnspruch, StakeFehler};
pub use training::{capped_training_reward, training_reward_cap, TRAINING_CAP_BPS};
pub use utilization::{
    calculate_utilization, utilization_from_burns, utilization_from_f64, utilization_to_f64,
    UTILIZATION_SCALE,
};
pub use vtfe::{vtfe_gutschrift, vtfe_voll, ModellProfil, ShardZuschnitt, VtfeError};

pub use zuschreibung::{
    zuschreiben, zuschreiben_aus_abrechnung, Abrechnungsfehler, Podabrechnung, Podleistung,
    Podposition, Zuschreibung, ZuschreibungFehler,
};
