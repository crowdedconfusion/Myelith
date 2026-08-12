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

pub mod distribute;
pub mod ema;
pub mod mint;

/// Anzahl der Kleinstbeträge je MYL (1 MYL = 10⁶ Kleinstbeträge).
pub const UNITS_PER_MYL: u64 = 1_000_000;

/// vTFE-Skalierung: eine vTFE-Einheit entspricht 10⁻⁶
/// Token-Forward-Äquivalenten (symmetrisch zu [`UNITS_PER_MYL`]).
pub const VTFE_UNITS_PER_TFE: u64 = 1_000_000;

pub use distribute::{
    distribute_mint, redundancy_normalized_weight, split_proportional, Distribution,
    DistributeError, SHARE_CHECKERS_BPS, SHARE_COORDINATORS_BPS, SHARE_SHARD_MINERS_BPS,
    SHARE_TREASURY_BPS, SHARE_VALIDATORS_BPS, SHARES_TOTAL_BPS,
};
pub use ema::{ema_update, EMA_ALPHA_DEN, EMA_ALPHA_NUM};
pub use mint::{mint_amount, MintParams};
