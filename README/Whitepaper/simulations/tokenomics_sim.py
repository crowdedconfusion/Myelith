#!/usr/bin/env python3
"""Burn/Mint-Gleichgewichts-Simulation (Whitepaper Kap. 5.2, 5.6, Anhang B.4).

Simuliert Epochen mit stochastischer Nachfrage und prüft:
  1. konvergiert M_e gegen B̄_e (Netto-Neutralität)?
  2. ist Self-Dealing verlustbringend (Anhang B.4)?
Kernbefund v0.1: OHNE Rechenkosten wäre Self-Dealing in der Subventionsphase
profitabel (Prägung = Burn·(1+s)). Die Sicherheit beruht darauf, dass Prägung
nur gegen verifizierte Arbeit fließt, deren Kosten den Subventionsaufschlag
übersteigen: Sicherheitsbedingung  s < COMPUTE_COST_RATIO / (1 − COMPUTE_COST_RATIO)
— bei Kosten von 70 % des Rewards also s < 2,33. Governance-Regel!
Nur Standardbibliothek — läuft überall: python3 tokenomics_sim.py
"""
import math
import random

EMA_WINDOW = 30          # Epochen (Kap. 5.2)
SUBSIDY_START = 0.5      # s zu Beginn
SUBSIDY_HALFLIFE = 500   # Epochen bis Halbierung von s
M_MAX = 1_000_000        # harter Deckel pro Epoche
K_PRICE = 0.3            # Dämpfung Credit-Preis (Kap. 5.4)
TARGET_UTIL = 0.8
# Rechenkosten ehrlicher Kapazität als Anteil des Miner-Rewards (Hardware+Strom).
# Zentral für Anhang B.4: Prägung erfordert ECHTE Arbeit — Self-Dealing kauft
# nicht nur Credits, sondern muss die Kapazität auch betreiben.
COMPUTE_COST_RATIO = 0.7


def run(epochs: int = 2000, seed: int = 42, self_deal_share: float = 0.0):
    rng = random.Random(seed)
    ema_burn, price, minted_total, burned_total = 10_000.0, 1.0, 0.0, 0.0
    attacker_burned, attacker_minted = 0.0, 0.0

    for e in range(epochs):
        subsidy = SUBSIDY_START * 0.5 ** (e / SUBSIDY_HALFLIFE)
        # organische Nachfrage: log-normal um wachsenden Trend
        organic = 10_000 * (1 + e / 1000) * rng.lognormvariate(0, 0.25)
        attack = organic * self_deal_share / max(1e-9, 1 - self_deal_share)
        burn = organic + attack

        ema_burn += (burn - ema_burn) / EMA_WINDOW
        minted = min(ema_burn * (1 + subsidy), M_MAX)

        util = min(1.5, burn / (minted * 1.1 + 1))  # grobe Kapazitäts-Proxy
        price *= math.exp(K_PRICE * (util - TARGET_UTIL))

        minted_total += minted
        burned_total += burn
        # Angreifer erntet Prägung nur proportional zu seinem Kapazitätsanteil α;
        # konservativ: α = self_deal_share (er stellt so viel Kapazität wie er kauft)
        attacker_burned += attack
        # Prägung fließt nur an tatsächlich geleistete, verifizierte Arbeit;
        # der Angreifer trägt dafür reale Rechenkosten wie jeder Miner.
        reward = minted * self_deal_share
        attacker_minted += reward - reward * COMPUTE_COST_RATIO

    return {
        "minted_total": minted_total,
        "burned_total": burned_total,
        "net_inflation": (minted_total - burned_total) / burned_total,
        "attacker_pnl": attacker_minted - attacker_burned,
    }


if __name__ == "__main__":
    base = run()
    print(f"[Basis]      Netto-Inflation über 2000 Epochen: {base['net_inflation']:+.2%}"
          f"  (Erwartung: klein positiv wg. Subvention, fallend)")

    for share in (0.05, 0.20, 0.40):
        r = run(self_deal_share=share)
        verdict = "verlustbringend ✅" if r["attacker_pnl"] < 0 else "PROFITABEL ⚠️ Parameter prüfen!"
        print(f"[Self-Deal α={share:.0%}] Angreifer-PnL: {r['attacker_pnl']:,.0f} MYL → {verdict}")
