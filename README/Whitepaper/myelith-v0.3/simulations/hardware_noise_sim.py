#!/usr/bin/env python3
"""hardware_noise_sim.py — Rauschmodell realer Beschleuniger aus Spezifikationsdaten
(Whitepaper v0.2, Kap. 6.5 und Anhang B.5; Meilenstein M0, Kap. 10 Punkt 1).

WAS DIESES SKRIPT LEISTET — UND WAS NICHT
-----------------------------------------
Es MISST nichts. Es leitet aus den numerisch relevanten Spezifikationsparametern
gaengiger Beschleuniger ab, welche Rundungsabweichungen bei identischer Eingabe
zu ERWARTEN sind, und berechnet daraus:

  (1) den Rauschpegel s_hw je Hardware-Klasse,
  (2) die Heterogenitaet zwischen Klassen (paarweise Verhaeltnisse),
  (3) die daraus folgende Anforderung an die Trennung SEP,
  (4) die Wirkung der in Kap. 6.5 vorgeschriebenen fp32-Akkumulation.

Grundlage ist die klassische Fehleranalyse fuer Gleitkomma-Summation
(Higham, "Accuracy and Stability of Numerical Algorithms"): Bei einer Reduktion
ueber n Terme mit Maschinengenauigkeit u waechst der relative Fehler
  - im Worst Case linear:            ~ n * u
  - bei zufaelligen Rundungen:       ~ sqrt(n) * u   (stochastisches Modell)
  - bei paarweiser/Baum-Reduktion:   ~ log2(n) * u
Reale GEMM-Kernels nutzen Baum-/Split-K-Reduktion; wir verwenden daher das
Baum-Modell und geben das stochastische Modell als obere Schranke mit an.

ENTSCHEIDEND ist NICHT das Speicherformat der Gewichte, sondern das
AKKUMULATIONSFORMAT — genau die Unterscheidung aus Whitepaper Kap. 6.5.

Nur Standardbibliothek. Aufruf: python3 hardware_noise_sim.py
"""

import math

# ── Maschinengenauigkeit (halbes ULP) der gaengigen Akkumulationsformate ──────
U = {
    'fp32': 2 ** -24,     # ~5.96e-08  — IEEE binary32, 24 bit Mantisse
    'tf32': 2 ** -11,     # ~4.88e-04  — 10 bit Mantisse (NVIDIA TF32-Pfad)
    'fp16': 2 ** -11,     # ~4.88e-04  — IEEE binary16, 11 bit Mantisse
    'bf16': 2 ** -8,      # ~3.91e-03  — 8 bit Mantisse
}

# ── Hardware-Klassen nach Spezifikation ──────────────────────────────────────
# accum      : Akkumulationsformat der Tensor-/Matrix-Einheiten laut Spezifikation
# accum_alt  : abweichender, ebenfalls spezifikationskonformer Pfad (Kernel-Wahl!)
# tile       : typische K-Tile-Groesse der GEMM-Kernels (Laenge einer Teilreduktion)
# split_k    : uebliche Anzahl paralleler Teilreduktionen, die final addiert werden
HARDWARE = [
    # Name                              accum   accum_alt  tile  split_k
    ("NVIDIA Datacenter (Hopper-Klasse)", 'fp32', 'fp32',    64,   8),
    ("NVIDIA Consumer (Ada-Klasse)",      'fp32', 'tf32',    64,   4),
    ("AMD CDNA (MI-Klasse)",              'fp32', 'fp32',    32,   8),
    ("AMD RDNA (Consumer)",               'fp32', 'fp16',    32,   4),
    ("Apple Silicon (M-Klasse)",          'fp32', 'fp16',    32,   2),
    ("CPU AVX-512 (Referenzpfad)",        'fp32', 'fp32',    16,   1),
]

HIDDEN = 8192          # Reduktionslaenge je Matrixzeile (hidden dimension)
LAYERS_PER_SHARD = 10  # Layer je Shard (k=8 bei 80-Layer-Modell)
K_SHARDS = 8


def noise_per_gemm(accum, tile, split_k, n=HIDDEN):
    """Erwartete relative Abweichung einer GEMM-Reduktion (Baum-Modell).

    Zwei Beitraege:
      - Teilreduktion ueber `tile` Elemente:      log2(tile) * u
      - finale Summation der split_k Teilergebnisse: log2(split_k) * u
    Der Gesamtbaum hat n/tile Blaetter; die Reduktionstiefe addiert sich.
    """
    u = U[accum]
    depth_tile = math.log2(max(2, tile))
    depth_leaves = math.log2(max(2, n / tile))
    depth_split = math.log2(max(2, split_k))
    return (depth_tile + depth_leaves + depth_split) * u


def noise_per_shard(accum, tile, split_k):
    """Abweichung nach LAYERS_PER_SHARD Layern (unabhaengige Beitraege)."""
    per = noise_per_gemm(accum, tile, split_k)
    return math.sqrt(LAYERS_PER_SHARD) * per


def main():
    print("=" * 78)
    print("RAUSCHMODELL AUS SPEZIFIKATIONSDATEN  —  Myelith v0.2, Kap. 6.5 / B.5 / M0")
    print("=" * 78)
    print("Modellrechnung nach Standard-Fehleranalyse der Gleitkomma-Summation,")
    print("NICHT gemessen. Grundlage: Akkumulationsformat, Tile-Groesse, Split-K.\n")

    # ── 1. Rauschpegel je Hardware bei vorgeschriebener fp32-Akkumulation ────
    print("[1] Rauschpegel s_hw bei PROTOKOLLKONFORMER fp32-Akkumulation (Kap. 6.5)")
    print(f"    {'Hardware':<38}{'rel. Abweichung/Shard':>24}")
    print("    " + "-" * 62)
    conform = {}
    for name, accum, _alt, tile, sk in HARDWARE:
        s = noise_per_shard('fp32', tile, sk)
        conform[name] = s
        print(f"    {name:<38}{s:>24.2e}")
    lo, hi = min(conform.values()), max(conform.values())
    print(f"\n    Spannweite ueber alle Anbieter: {hi/lo:.2f}x")
    print(f"    ==> Heterogenitaet sigma (log-Streuung): {math.log(hi/lo)/2:.2f}")
    print(f"    ==> Bei fp32-Akkumulation sind die Anbieter numerisch NAHEZU GLEICH.")
    print(f"        Die Unterschiede stammen nur aus Tile-/Split-K-Geometrie.\n")

    # ── 2. Was passiert ohne die fp32-Vorschrift? ───────────────────────────
    print("[2] Rauschpegel OHNE Protokollvorschrift (jeder Anbieter nutzt seinen")
    print("    schnellsten spezifikationskonformen Pfad)")
    print(f"    {'Hardware':<38}{'Akkum.':>8}{'rel. Abweichung':>18}")
    print("    " + "-" * 64)
    free = {}
    for name, _accum, alt, tile, sk in HARDWARE:
        s = noise_per_shard(alt, tile, sk)
        free[name] = s
        print(f"    {name:<38}{alt:>8}{s:>18.2e}")
    lo2, hi2 = min(free.values()), max(free.values())
    print(f"\n    Spannweite: {hi2/lo2:.0f}x   (gegenueber {hi/lo:.2f}x mit fp32-Pflicht)")
    print(f"    ==> Ohne die Vorschrift aus Kap. 6.5 driften die Rauschpegel um")
    print(f"        mehr als drei Groessenordnungen auseinander. Ein globales TAU")
    print(f"        waere dann nicht mehr definierbar — der langsamste ehrliche")
    print(f"        Knoten wuerde staendig ueber der Schwelle des schnellsten liegen.\n")

    # ── 3. Folge fuer die erforderliche Trennung SEP ────────────────────────
    print("[3] Folge fuer die Trennungsanforderung")
    print("    Heterogenitaet geht nach robustness_sim.py als sigma in die")
    print("    SEP-Anforderung ein (sigma=0.0 -> 5x, 0.3 -> 8x, 0.6 -> 20x).")
    sigma_conform = math.log(hi / lo) / 2
    sigma_free = math.log(hi2 / lo2) / 2
    def sep_for(sig):
        if sig <= 0.15: return "5x"
        if sig <= 0.45: return "8x"
        if sig <= 0.8:  return "20x"
        return ">35x"
    print(f"      mit fp32-Pflicht : sigma={sigma_conform:.2f}  ->  SEP {sep_for(sigma_conform)}")
    print(f"      ohne Vorschrift  : sigma={sigma_free:.2f}  ->  SEP {sep_for(sigma_free)}")
    print()

    # ── 4. Abgleich mit den Angriffsgroessenordnungen aus B.5 ───────────────
    print("[4] Trennbarkeit realer Angriffe (Verhaeltnis zum konformen Rauschen)")
    base = sum(conform.values()) / len(conform)
    attacks = [
        ("Layer uebersprungen",            3e-1),
        ("Falsche Gewichte / Modell",      2e-1),
        ("fp8 statt bf16 (heimlich)",      6e-2),
        ("int8-Quantisierung",             3e-2),
        ("Aktivierungs-Pruning 10 %",      5e-3),
    ]
    print(f"    Referenz-Rauschen (fp32-konform, Mittel): {base:.2e}")
    print(f"    {'Angriff':<34}{'rel. Abw.':>12}{'Verhaeltnis':>14}   Bewertung")
    print("    " + "-" * 76)
    for name, val in attacks:
        ratio = val / base
        if ratio >= 35:   verdict = "sicher trennbar (auch im Ungunstfall)"
        elif ratio >= 20: verdict = "trennbar bei realistischen Annahmen"
        elif ratio >= 5:  verdict = "nur unter Idealannahmen trennbar"
        else:             verdict = "NICHT trennbar — Kontrollsegmente noetig"
        print(f"    {name:<34}{val:>12.0e}{ratio:>13.0f}x   {verdict}")
    # ── 5. Schadensraum des Lenkungsangriffs ────────────────────────────────
    print("[5] Schadensraum des Lenkungsangriffs (Kap. 6.11)")
    print("    Dieser Angreifer waehlt seine Verzerrung definitionsgemaess knapp")
    print("    UNTER TAU — er hat also keine feste Groesse, sondern skaliert mit")
    print("    dem Rauschen. Entscheidend ist daher der ABSOLUTWERT von TAU:")
    for label, noise in [("fp32-Akkumulation (Protokollpflicht)", base),
                         ("bf16-Akkumulation (ohne Vorschrift)", noise_per_shard('bf16', 64, 4))]:
        tau_abs = 5 * noise * math.sqrt(K_SHARDS)   # TAU bei SEP=5 ueber die Pipeline
        print(f"      {label:<40} TAU ~ {tau_abs:.1e} relativ")
    print("    ==> Mit fp32-Pflicht liegt die maximal unentdeckte Verzerrung im")
    print("        Bereich 1e-5 relativ. Ob eine Stoerung dieser Groessenordnung")
    print("        die Token-Auswahl ueberhaupt kippen kann, ist eine offene")
    print("        Messfrage (M0) — die Quantisierung der Logits (Kap. 6.8) setzt")
    print("        hier eine zusaetzliche Schranke, da Aenderungen unterhalb der")
    print("        Rasterweite die Auswahl per Konstruktion nicht veraendern.")
    print()
    print("=" * 78)
    print("SCHLUSSFOLGERUNGEN")
    print("  1. Die fp32-Akkumulationspflicht (Kap. 6.5) ist nicht kosmetisch:")
    print("     Sie reduziert die Hardware-Heterogenitaet von >3 Groessenordnungen")
    print("     auf einen Faktor < 2 und macht ein globales TAU ueberhaupt erst")
    print("     definierbar (Kap. 6.7).")
    print("  2. Unter dieser Vorschrift liegen alle arbeitssparenden Angriffe")
    print("     mehrere Groessenordnungen ueber dem Rauschen — die Anforderung")
    print("     aus B.5 ist mit erheblicher Reserve erfuellt.")
    print("  3. Feine Eingriffe ohne Ersparnismotiv bleiben unterhalb der")
    print("     Trennschwelle. Fuer sie greifen Kontrollsegmente (Kap. 6.10)")
    print("     und die Vorzeichenstatistik (Kap. 6.9), nicht der Abstandsvergleich.")
    print("  4. ZU MESSEN IN M0: die tatsaechliche Verteilungsform in den extremen")
    print("     Quantilen (dominanter Faktor nach robustness_sim.py) sowie die")
    print("     Frage, ob reale Kernels die angenommene Baum-Reduktion einhalten.")
    print("=" * 78)


if __name__ == "__main__":
    main()
