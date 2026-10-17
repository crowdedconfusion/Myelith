# Belegungssimulation: myelith-35b-a3b

Erzeugt von `belegungssimulation.py`. Lernspur 2035 Token, Prüfspur 1977 Token, Profil aus 2035 Token. Dicht 2.34 GiB, 40 × 256 Experten zu 3.15 MB. Rechenzeit 43.5 ms je Token, Platte 3.41 GB/s, System 6 GiB. Zellen: Token/s (Treffer im Speicher, MB je Token von der Platte).

## Kontext und Platz je Strategie

| Arbeitsspeicher | `alles_laden` | `seitencache_kurz` | `seitencache_voll` | `seitencache_arbeit` | `plan` |
|---|---|---|---|---|---|
| 16 GiB | läuft nicht | Kontext 4096, 6.9 GiB Experten | Kontext 262144, 1.9 GiB Experten | Kontext 65536, 5.7 GiB Experten | Kontext 8192, 6.8 GiB Experten (1836 heiß vorgewärmt) |
| 24 GiB | läuft nicht | Kontext 4096, 14.9 GiB Experten | Kontext 262144, 9.9 GiB Experten | Kontext 65536, 13.7 GiB Experten | Kontext 65536, 13.7 GiB Experten (2878 heiß vorgewärmt) |
| 32 GiB | läuft nicht | Kontext 4096, 22.9 GiB Experten | Kontext 262144, 17.9 GiB Experten | Kontext 65536, 21.7 GiB Experten | Kontext 65536, 21.7 GiB Experten (2878 heiß vorgewärmt) |
| 48 GiB | Kontext 65536, 30.0 GiB Experten | Kontext 4096, 30.0 GiB Experten | Kontext 262144, 30.0 GiB Experten | Kontext 65536, 30.0 GiB Experten | Kontext 65536, 30.0 GiB Experten (2878 heiß vorgewärmt) |
| 64 GiB | Kontext 65536, 30.0 GiB Experten | Kontext 4096, 30.0 GiB Experten | Kontext 262144, 30.0 GiB Experten | Kontext 65536, 30.0 GiB Experten | Kontext 65536, 30.0 GiB Experten (2878 heiß vorgewärmt) |

## Dauerbetrieb (nach der Lernspur angewärmt)

| Arbeitsspeicher | `alles_laden` | `seitencache_kurz` | `seitencache_voll` | `seitencache_arbeit` | `plan` | `plan_festgehalten` | `optimum` |
|---|---|---|---|---|---|---|---|
| 16 GiB | läuft nicht | 9.5 (79.1 %, 211 MB) | 5.2 (49.3 %, 510 MB) | 8.4 (74.4 %, 258 MB) | 9.4 (78.8 %, 214 MB) | 9.2 (78.1 %, 221 MB) | 13.9 (90.3 %, 98 MB) |
| 24 GiB | läuft nicht | 16.7 (94.5 %, 56 MB) | 12.4 (87.4 %, 127 MB) | 15.7 (93.2 %, 68 MB) | 15.7 (93.2 %, 68 MB) | 16.0 (93.6 %, 64 MB) | 19.5 (97.4 %, 26 MB) |
| 32 GiB | läuft nicht | 21.7 (99.2 %, 8 MB) | 19.1 (97.0 %, 30 MB) | 21.3 (98.8 %, 12 MB) | 21.3 (98.8 %, 12 MB) | 21.3 (98.8 %, 12 MB) | 22.4 (99.6 %, 4 MB) |
| 48 GiB | 23.0 (100.0 %, 0 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) |
| 64 GiB | 23.0 (100.0 %, 0 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) |

## Kaltstart (die ersten 500 Token nach dem Laden, Lesen vorab mitgezählt)

| Arbeitsspeicher | `alles_laden` | `seitencache_kurz` | `seitencache_voll` | `seitencache_arbeit` | `plan` | `plan_festgehalten` | `plan`, Vorwärmen mitgezählt |
|---|---|---|---|---|---|---|---|
| 16 GiB | läuft nicht | 10.1 (81.3 %, 189 MB) | 5.3 (51.2 %, 492 MB) | 8.9 (76.6 %, 235 MB) | 10.1 (81.4 %, 188 MB) | 9.8 (80.2 %, 199 MB) | 9.8 (80.2 %, 199 MB) |
| 24 GiB | läuft nicht | 15.9 (93.4 %, 66 MB) | 12.7 (88.0 %, 121 MB) | 15.3 (92.6 %, 75 MB) | 16.1 (93.7 %, 63 MB) | 16.5 (94.3 %, 58 MB) | 14.8 (91.9 %, 81 MB) |
| 32 GiB | läuft nicht | 17.4 (95.3 %, 48 MB) | 16.9 (94.7 %, 53 MB) | 17.4 (95.3 %, 48 MB) | 19.1 (97.0 %, 30 MB) | 19.1 (97.0 %, 30 MB) | 17.4 (95.2 %, 48 MB) |
| 48 GiB | 16.0 (93.6 %, 64 MB) | 17.4 (95.3 %, 48 MB) | 17.4 (95.3 %, 48 MB) | 17.4 (95.3 %, 48 MB) | 19.1 (97.0 %, 30 MB) | 19.1 (97.0 %, 30 MB) | 17.4 (95.2 %, 48 MB) |
| 64 GiB | 16.0 (93.6 %, 64 MB) | 17.4 (95.3 %, 48 MB) | 17.4 (95.3 %, 48 MB) | 17.4 (95.3 %, 48 MB) | 19.1 (97.0 %, 30 MB) | 19.1 (97.0 %, 30 MB) | 17.4 (95.2 %, 48 MB) |

## Ein Bild mitten in der Arbeit (begrenzter Kontext, 3,65 GiB): die Token danach

| Arbeitsspeicher | `seitencache_kurz` | `seitencache_voll` | `seitencache_arbeit` | `plan` |
|---|---|---|---|---|
| 16 GiB | 9.1 (77.5 %, 227 MB) | 5.1 (48.4 %, 520 MB), 1.7 GiB Auslagerung | 8.1 (72.9 %, 273 MB) | 9.0 (77.2 %, 230 MB) |
| 24 GiB | 16.4 (94.1 %, 60 MB) | 12.0 (86.4 %, 137 MB) | 15.4 (92.7 %, 73 MB) | 15.4 (92.7 %, 73 MB) |
| 32 GiB | 21.6 (99.1 %, 10 MB) | 19.0 (96.9 %, 32 MB) | 21.1 (98.7 %, 13 MB) | 21.1 (98.7 %, 13 MB) |
| 48 GiB | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) |
| 64 GiB | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) |

## Ein Bild mitten in der Arbeit (Vorgabekontext der Laufzeit, 18,25 GiB): die Token danach

| Arbeitsspeicher | `seitencache_kurz` | `seitencache_voll` | `seitencache_arbeit` | `plan` |
|---|---|---|---|---|
| 16 GiB | 9.0 (77.3 %, 229 MB), 11.4 GiB Auslagerung | 5.1 (48.4 %, 520 MB), 16.3 GiB Auslagerung | 8.1 (72.8 %, 274 MB), 12.6 GiB Auslagerung | 9.4 (78.8 %, 214 MB), Sehen abgelehnt |
| 24 GiB | 15.7 (93.2 %, 69 MB), 3.4 GiB Auslagerung | 11.8 (86.0 %, 141 MB), 8.3 GiB Auslagerung | 14.8 (91.9 %, 81 MB), 4.6 GiB Auslagerung | 15.7 (93.2 %, 68 MB), Sehen abgelehnt |
| 32 GiB | 19.8 (97.7 %, 24 MB) | 17.7 (95.6 %, 44 MB), 0.3 GiB Auslagerung | 19.5 (97.3 %, 27 MB) | 19.5 (97.3 %, 27 MB) |
| 48 GiB | 21.9 (99.2 %, 8 MB) | 21.2 (98.7 %, 13 MB) | 21.7 (99.1 %, 9 MB) | 21.7 (99.1 %, 9 MB) |
| 64 GiB | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) | 22.9 (99.9 %, 1 MB) |

