"""
Berechnet Zweierpotenz-Skalen (Shifts) aus den gesammelten Statistiken.
"""

import math

# Siehe calibrate/src/quantize.py::MAX_FRAC_BITS fuer die Begruendung.
MAX_FRAC_BITS = 20

# Aktivierungen sind seit dem Numerik-Realitaetsabgleich (v0.12.20) int16
# mit Per-Layer-Skalen; Gewichte bleiben int8 (siehe quantize.py). Die
# Aktivierungsskalen muessen daher den int16-Wertebereich abdecken.
ACTIVATION_MAX_INT = 32767


def choose_pow2_shift(absmax: float, max_int: int = ACTIVATION_MAX_INT) -> int:
    """
    Bestimmt frac_bits (Laufzeit-Konvention, arithmetischer Rechtsshift bei
    der Dequantisierung: real ≈ quantized >> shift), sodass
    absmax * 2^shift in [-max_int, max_int] passt.

    Vorherige Fassung (Rechtsshift bereits bei der Quantisierung gedacht)
    lieferte fuer absmax < max_int - der Regelfall bei realen
    Aktivierungen - immer shift=0 und verschenkte damit fast die gesamte
    int8-Aufloesung. floor() statt ceil() ist hier zwingend: ceil() koennte
    absmax * 2^shift > max_int ergeben und den Wertebereich verletzen.
    """
    if absmax <= 1e-9:
        return 0
    shift = math.floor(math.log2(max_int / absmax))
    return max(0, min(shift, MAX_FRAC_BITS))


# Sicherheitsabstand der Per-Kanal-Skalen in Bits (Fund 21, 2026-08-19).
#
# Fund 20 waehlt je Kanal die ENGSTE Skala, die seinen KALIBRIERTEN
# Maximalwert traegt — darin liegt der Aufloesungsgewinn, aber auch das
# Problem: solche Skalen haben null Headroom. Kalibriert wird auf 64
# WikiText-Sequenzen, gemessen auf 4 bewusst ausgesparten anderen
# (damit die Kalibrierung nicht auf den Benchmark ueberpasst, siehe
# main.py::_wikitext_calibration_texts). Uebersteigt ein Kanal zur
# Laufzeit sein kalibriertes Maximum, clippt er an der int16-Grenze —
# mit der engen Per-Kanal-Skala viel leichter als mit der grosszuegigen
# Per-Tensor-Skala davor.
#
# Gemessen (tests/diag/per_channel_headroom.py, 2026-08-19):
#
#            clippende Kanaele   schlimmster Faktor   Perplexitaet
#     0,5B         0,77 %              1,61x          15,59 -> 15,29  (besser)
#     7B           6,24 %              4,53x          16,26 -> 40,48  (schlechter)
#
# Dieselbe Logik, derselbe Code — nur andere Aktivierungsstatistik. Bei
# 0,5B ueberwiegt der Aufloesungsgewinn knapp, bei 7B ueberwiegt das
# Clipping massiv. Das ist die Wiederholung von Fund 14 Kandidat (i)
# (v0.12.26) auf der Kanal-Ebene: der damals durch einen breiteren
# Kalibrierkorpus gewonnene Headroom wird von der engeren Skala wieder
# aufgezehrt.
#
# **GEMESSENES NEGATIV-ERGEBNIS (2026-08-19): auf 0 gesetzt.**
#
# Ein Sicherheitsabstand von 2 Bit beseitigte das Clipping wie geplant
# (7B: 6,24 % -> 0,02 % clippende Kanaele, schlimmster Faktor 4,53x ->
# 1,13x), verschlechterte die Perplexitaet aber BEIDE Modelle deutlich:
#
#                      ohne Headroom    mit 2 Bit Headroom
#     0,5B                  15,29             20,98
#     7B                    40,68          19365,03
#
# Der Auflösungsverlust wiegt also schwerer als der Clipping-Gewinn - bei
# 7B um Groessenordnungen. Damit ist Clipping als dominante Fehlerquelle
# ausgeschlossen und gleichzeitig gezeigt, dass 7B extrem empfindlich auf
# Aufloesungsverlust im Residualstrom reagiert (2 Bit weniger = Faktor 476
# schlechter).
#
# Die Konstante bleibt als dokumentierter, messbarer Schalter stehen: Wer
# die Messung wiederholen oder einen anderen Wert pruefen will, aendert
# hier eine Zahl. Belege: tests/diag/per_channel_headroom.py (Clipping),
# eval/results/decision_12-21_qwen25-7b.md (Perplexitaet).
PER_CHANNEL_HEADROOM_BITS = 0


def compute_scales_from_stats(stats: dict, max_int: int = ACTIVATION_MAX_INT) -> dict:
    """
    Fund 20 (2026-08-18): Eintraege mit "channel_absmax" (die drei
    Residualstrom-Segmente, siehe stats.py-Modulkopf) bekommen zusaetzlich
    ein "shifts"-Array - eine Zweierpotenz-Skala je Kanal statt einer
    einzigen fuer den ganzen Tensor. "shift"/"scale"/"absmax_observed"
    bleiben als Gesamt-Zusammenfassung erhalten (informativ); der
    Runtime-Loader liest bei Vorhandensein von "shifts" NUR dieses Array
    (`ScaleEntry::shifts`, `runtime/src/loader.rs`).

    Fund 21 (2026-08-19): Die Per-Kanal-Shifts tragen einen
    Sicherheitsabstand von PER_CHANNEL_HEADROOM_BITS gegen Clipping auf
    ungesehenen Sequenzen — siehe Konstante oben fuer die Messung, die
    dazu gefuehrt hat.
    """
    scales = {}
    for name, s in stats.items():
        shift = choose_pow2_shift(s["absmax"], max_int)
        entry = {
            "shift": shift,
            "scale": 2.0 ** (-shift),
            "absmax_observed": s["absmax"],
        }
        if "channel_absmax" in s:
            entry["shifts"] = [
                max(0, choose_pow2_shift(v, max_int) - PER_CHANNEL_HEADROOM_BITS)
                for v in s["channel_absmax"]
            ]
            entry["headroom_bits"] = PER_CHANNEL_HEADROOM_BITS
        scales[name] = entry
    return scales
