#!/usr/bin/env python3
"""**Speicherballon**: belegt N GiB Arbeitsspeicher, bis er beendet wird.

# ⚑ Wozu

Um auf einer grossen Maschine zu messen, wie ein Modell auf einer kleineren
laeuft. Der Ballon fuellt den Speicher mit Zufallsbytes (die das System
nicht komprimieren kann) und versucht, ihn mit `mlock` festzuhalten. Ist er
bereit, schreibt er eine Zeile, die mit `bereit` beginnt; ein Messskript
wartet darauf und beendet ihn danach.

⚠️ **Ob er gehalten hat, sagt das System, nicht der Ballon.** Unter macOS
meldete `mlock` Erfolg, ohne dass die Seiten als festgelegt zaehlten; der
Ballon blieb trotzdem im Speicher. Wer misst, vergleicht die Swapouts
(`vm_stat`) vor und nach der Reihe: Stiegen sie, war der Ballon teils
ausgelagert und die Messung zaehlt nicht.

Aufruf: `python3 BENCHMARKS/Inferenz/ballon.py <gib>`
"""

import ctypes
import mmap
import os
import sys
import time


def main() -> int:
    gib = float(sys.argv[1]) if len(sys.argv) > 1 else 0.0
    n = int(gib * (1 << 30))
    if n <= 0:
        print("bereit 0", flush=True)
    else:
        m = mmap.mmap(-1, n)
        block = os.urandom(64 << 20)
        for i in range(0, n, len(block)):
            m[i : i + len(block)] = block[: min(len(block), n - i)]
        libc = ctypes.CDLL(None)
        anfang = ctypes.addressof(ctypes.c_char.from_buffer(m))
        r = libc.mlock(ctypes.c_void_p(anfang), ctypes.c_size_t(n))
        print(f"bereit {gib} mlock={r}", flush=True)
    while True:
        time.sleep(3600)


if __name__ == "__main__":
    raise SystemExit(main())
