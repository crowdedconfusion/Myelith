// Der Nachbau als Programm: liest Faelle aus einer Datei, rechnet sie mit
// den Kerneln aus `kerne.cu` auf der CPU und schreibt die Ausgaben.
//
// Aufruf: nachbau <eingabe> <ausgabe>
//
// Je Fall, alle Zahlen little-endian:
//   int32 art (0 int8, 1 ternaer, 2 vier Bit), zeilen, spalten, stapel
//   art 0: int8 w[zeilen*spalten]
//   art 1: uint8 muster[zeilen*spalten/4], int16 betraege[zeilen*spalten/128]
//   art 2: uint8 codes[zeilen*spalten/2], uint8 gitter[zeilen*spalten/128*2]
//   int16 x[stapel*spalten], int8 abstaende[zeilen]
// Ausgabe je Fall: int16 aus[stapel*zeilen].

#include <cstdint>
#include <cstdio>
#include <vector>

#include "nachbau.hpp"
#include "kerne.cu"

template <class T>
static bool lesen(FILE* f, std::vector<T>& v, size_t n) {
    v.resize(n);
    return n == 0 || fread(v.data(), sizeof(T), n, f) == n;
}

int main(int argc, char** argv) {
    if (argc != 3) {
        fprintf(stderr, "Aufruf: nachbau <eingabe> <ausgabe>\n");
        return 2;
    }
    FILE* ein = fopen(argv[1], "rb");
    FILE* aus = fopen(argv[2], "wb");
    if (!ein || !aus) {
        fprintf(stderr, "Dateien nicht zu oeffnen\n");
        return 2;
    }
    for (;;) {
        int32_t kopf[4];
        if (fread(kopf, sizeof(int32_t), 4, ein) != 4) {
            break;
        }
        const int art = kopf[0], zeilen = kopf[1], spalten = kopf[2], stapel = kopf[3];
        std::vector<myl_i8> w;
        std::vector<myl_u8> muster, codes, gitter;
        std::vector<myl_i16> betraege, x, ergebnis((size_t)stapel * zeilen);
        std::vector<myl_i8> abstaende;
        bool gut = art == 0   ? lesen(ein, w, (size_t)zeilen * spalten)
                   : art == 1 ? lesen(ein, muster, (size_t)zeilen * spalten / 4) &&
                                    lesen(ein, betraege, (size_t)zeilen * (spalten / GRUPPE))
                              : lesen(ein, codes, (size_t)zeilen * spalten / 2) &&
                                    lesen(ein, gitter, (size_t)zeilen * (spalten / GRUPPE) * 2);
        gut = gut && lesen(ein, x, (size_t)stapel * spalten) && lesen(ein, abstaende, (size_t)zeilen);
        if (!gut) {
            fprintf(stderr, "Fall unvollstaendig\n");
            return 1;
        }
        MylDrei raster{(unsigned)((zeilen + KACHEL_ZEILEN - 1) / KACHEL_ZEILEN),
                       (unsigned)((stapel + KACHEL_EINGABEN - 1) / KACHEL_EINGABEN), 1};
        MylDrei gruppe{FAEDEN_X, FAEDEN_Y, 1};
        if (art == 0) {
            myl_starten(raster, gruppe, [&] {
                myl_w8a16(w.data(), x.data(), abstaende.data(), ergebnis.data(), zeilen, spalten, stapel);
            });
        } else if (art == 1) {
            myl_starten(raster, gruppe, [&] {
                myl_ternaer(muster.data(), betraege.data(), x.data(), abstaende.data(), ergebnis.data(), zeilen,
                            spalten, stapel);
            });
        } else {
            myl_starten(raster, gruppe, [&] {
                myl_vierbit(codes.data(), gitter.data(), x.data(), abstaende.data(), ergebnis.data(), zeilen,
                            spalten, stapel);
            });
        }
        fwrite(ergebnis.data(), sizeof(myl_i16), ergebnis.size(), aus);
    }
    fclose(aus);
    fclose(ein);
    return 0;
}
