// W8A16 im Buendel auf einer Grafikkarte von NVIDIA oder AMD.
//
// Ein Dialekt fuer zwei Uebersetzer, beide zur Laufzeit: NVRTC (CUDA) und
// hiprtc (ROCm). Gemeinsam ist ihnen alles, was hier steht: `__global__`,
// `__shared__`, `__syncthreads`, `threadIdx`, `blockIdx`. Keine Kopfdatei,
// denn NVRTC bringt keine Standardbibliothek mit; die Typen sind eigene
// Namen. Dieselbe Quelle laeuft im Nachbau auf der CPU (`nachbau.rs`), dort
// mit einer Huelle fuer genau diese Woerter.
//
// Was hier gilt, steht im Determinismus-Vertrag (`backends/cuda.rs`):
//
// 1. Nur Ganzzahlen. Kein Gleitkommatyp, kein Gleitkommaliteral.
// 2. Keine Tensor Cores, kein Warp-Shuffle, keine Annahme ueber die
//    Warp-Breite (NVIDIA 32, AMD 64): Die Kachel teilt sich ueber den
//    gemeinsamen Speicher und `__syncthreads`, sonst nichts.
// 3. Saettigung genau einmal, ganz am Ende.
//
// Die Summen sind exakt: je Stueck in int32, dann in int64. Ein Stueck von
// 64 Spalten erreicht hoechstens 64 * 128 * 32 768 = 2^28, eine Gruppe von
// 128 ternaeren Gewichten 128 * 2 * 32 768 = 2^23. Damit ist jede
// Reihenfolge erlaubt, und die Kachelung aendert keine Zahl.

typedef signed char myl_i8;
typedef unsigned char myl_u8;
typedef short myl_i16;
typedef int myl_i32;
typedef long long myl_i64;
typedef unsigned long long myl_u64;

#ifndef MYL_KERN
#define MYL_KERN extern "C" __global__
#endif
#ifndef MYL_GERAET
#define MYL_GERAET __device__ static inline
#endif

// Kachelmass. Die Rust-Seite startet dieselben Masse (`treiber.rs`,
// `KACHEL_ZEILEN`, `KACHEL_EINGABEN`, `FAEDEN_X`, `FAEDEN_Y`).
#define KACHEL_ZEILEN 64
#define KACHEL_EINGABEN 16
#define FAEDEN_X 16
#define FAEDEN_Y 16
#define FAEDEN (FAEDEN_X * FAEDEN_Y)
#define ZEILEN_JE_FADEN (KACHEL_ZEILEN / FAEDEN_Y)
#define STUECK 64
#define GRUPPE 128

// Rechtsshift mit Runden zur naechsten geraden Zahl wie `rshift_round_i64`;
// ein negativer Abstand schiebt nach links wie `rescale_i64`, und zwar mit
// Umlauf der Bits wie der Linksshift in Rust (ueber `unsigned`, denn ein
// Linksshift einer negativen Zahl mit Vorzeichen ist in C++ vor 20 nicht
// festgelegt).
MYL_GERAET myl_i64 umskalieren(myl_i64 wert, int abstand) {
    if (abstand < 0) {
        return (myl_i64)((myl_u64)wert << (-abstand));
    }
    if (abstand == 0) {
        return wert;
    }
    myl_i64 maske = ((myl_i64)1 << abstand) - 1;
    myl_i64 halb = (myl_i64)1 << (abstand - 1);
    myl_i64 quotient = wert >> abstand;
    myl_i64 rest = wert & maske;
    if (rest > halb || (rest == halb && (quotient & 1) != 0)) {
        quotient += 1;
    }
    return quotient;
}

MYL_GERAET myl_i16 klemmen(myl_i64 y) {
    if (y < -32768) {
        return (myl_i16)(-32768);
    }
    if (y > 32767) {
        return (myl_i16)32767;
    }
    return (myl_i16)y;
}

// Gewichte int8, `zeilen` x `spalten`, Zeile fuer Zeile; Eingaben int16,
// `stapel` x `spalten`; Ausgabe `stapel` x `zeilen`. Je Fadengruppe eine
// Kachel aus 64 Zeilen und 16 Eingaben; jeder Faden rechnet 4 Zeilen einer
// Eingabe.
MYL_KERN void myl_w8a16(const myl_i8* w, const myl_i16* x, const myl_i8* abstaende, myl_i16* aus,
                        int zeilen, int spalten, int stapel) {
    // Die Polster (+4, +2) verschieben jede Zeile um eine Bank.
    __shared__ myl_i8 kw[KACHEL_ZEILEN][STUECK + 4];
    __shared__ myl_i16 kx[KACHEL_EINGABEN][STUECK + 2];
    const int tx = (int)threadIdx.x;
    const int ty = (int)threadIdx.y;
    const int faden = ty * FAEDEN_X + tx;
    const myl_i64 zeile0 = (myl_i64)blockIdx.x * KACHEL_ZEILEN;
    const myl_i64 eingabe0 = (myl_i64)blockIdx.y * KACHEL_EINGABEN;
    myl_i64 summe[ZEILEN_JE_FADEN];
    for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
        summe[r] = 0;
    }
    for (int k0 = 0; k0 < spalten; k0 += STUECK) {
        for (int i = faden; i < KACHEL_ZEILEN * STUECK; i += FAEDEN) {
            int r = i / STUECK;
            int k = i % STUECK;
            myl_i64 z = zeile0 + r;
            int s = k0 + k;
            kw[r][k] = (z < zeilen && s < spalten) ? w[z * spalten + s] : (myl_i8)0;
        }
        for (int i = faden; i < KACHEL_EINGABEN * STUECK; i += FAEDEN) {
            int e = i / STUECK;
            int k = i % STUECK;
            myl_i64 b = eingabe0 + e;
            int s = k0 + k;
            kx[e][k] = (b < stapel && s < spalten) ? x[b * spalten + s] : (myl_i16)0;
        }
        __syncthreads();
        myl_i32 teil[ZEILEN_JE_FADEN];
        for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
            teil[r] = 0;
        }
        for (int k = 0; k < STUECK; k++) {
            myl_i32 xv = kx[tx][k];
            for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
                teil[r] += (myl_i32)kw[ty + FAEDEN_Y * r][k] * xv;
            }
        }
        for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
            summe[r] += (myl_i64)teil[r];
        }
        // Erst wenn alle gelesen haben, darf das naechste Stueck laden.
        __syncthreads();
    }
    const myl_i64 b = eingabe0 + tx;
    for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
        myl_i64 z = zeile0 + ty + FAEDEN_Y * r;
        if (z < zeilen && b < stapel) {
            aus[b * zeilen + z] = klemmen(umskalieren(summe[r], (int)abstaende[z]));
        }
    }
}

// Ternaere Gewichte: je Zeile `spalten / 4` Musterbytes und `spalten / 128`
// Betraege. Format wie in `ternaer.rs`: Je Gruppe 32 Byte in zwei Bloecken
// zu 16 fuer je 64 Gewichte; Byte j eines Blocks traegt in den Bits 2s und
// 2s+1 den Code c des Gewichts 16s + j, und das Gewicht ist c - 1 (Code 3
// heisst +2). Je Gruppe die Summe t * x in int32, mal dem Betrag in int64:
// dieselbe ganze Zahl wie `sum_g m_g * (sum_{i in g} t_i * x_i)` der CPU.
MYL_KERN void myl_ternaer(const myl_u8* muster, const myl_i16* betraege, const myl_i16* x,
                          const myl_i8* abstaende, myl_i16* aus, int zeilen, int spalten, int stapel) {
    __shared__ myl_i8 kt[KACHEL_ZEILEN][GRUPPE + 4];
    __shared__ myl_i16 kx[KACHEL_EINGABEN][GRUPPE + 2];
    const int tx = (int)threadIdx.x;
    const int ty = (int)threadIdx.y;
    const int faden = ty * FAEDEN_X + tx;
    const myl_i64 zeile0 = (myl_i64)blockIdx.x * KACHEL_ZEILEN;
    const myl_i64 eingabe0 = (myl_i64)blockIdx.y * KACHEL_EINGABEN;
    const int gruppen = spalten / GRUPPE;
    const myl_i64 bytes_je_zeile = spalten / 4;
    myl_i64 summe[ZEILEN_JE_FADEN];
    for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
        summe[r] = 0;
    }
    for (int g = 0; g < gruppen; g++) {
        // 64 Zeilen zu 32 Musterbytes, je Byte vier Gewichte.
        for (int i = faden; i < KACHEL_ZEILEN * (GRUPPE / 4); i += FAEDEN) {
            int r = i / (GRUPPE / 4);
            int p = i % (GRUPPE / 4);
            myl_i64 z = zeile0 + r;
            int spalte = (p / 16) * 64 + (p % 16);
            if (z < zeilen) {
                unsigned int B = (unsigned int)muster[z * bytes_je_zeile + (myl_i64)g * (GRUPPE / 4) + p];
                kt[r][spalte] = (myl_i8)((int)(B & 3u) - 1);
                kt[r][spalte + 16] = (myl_i8)((int)((B >> 2) & 3u) - 1);
                kt[r][spalte + 32] = (myl_i8)((int)((B >> 4) & 3u) - 1);
                kt[r][spalte + 48] = (myl_i8)((int)((B >> 6) & 3u) - 1);
            } else {
                kt[r][spalte] = 0;
                kt[r][spalte + 16] = 0;
                kt[r][spalte + 32] = 0;
                kt[r][spalte + 48] = 0;
            }
        }
        for (int i = faden; i < KACHEL_EINGABEN * GRUPPE; i += FAEDEN) {
            int e = i / GRUPPE;
            int k = i % GRUPPE;
            myl_i64 b = eingabe0 + e;
            kx[e][k] = (b < stapel) ? x[b * spalten + (myl_i64)g * GRUPPE + k] : (myl_i16)0;
        }
        __syncthreads();
        myl_i32 teil[ZEILEN_JE_FADEN];
        for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
            teil[r] = 0;
        }
        for (int k = 0; k < GRUPPE; k++) {
            myl_i32 xv = kx[tx][k];
            for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
                teil[r] += (myl_i32)kt[ty + FAEDEN_Y * r][k] * xv;
            }
        }
        for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
            myl_i64 z = zeile0 + ty + FAEDEN_Y * r;
            if (z < zeilen) {
                summe[r] += (myl_i64)teil[r] * (myl_i64)betraege[z * gruppen + g];
            }
        }
        __syncthreads();
    }
    const myl_i64 b = eingabe0 + tx;
    for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
        myl_i64 z = zeile0 + ty + FAEDEN_Y * r;
        if (z < zeilen && b < stapel) {
            aus[b * zeilen + z] = klemmen(umskalieren(summe[r], (int)abstaende[z]));
        }
    }
}

// Vier Bit je Gewicht: je Zeile `spalten / 2` Codebytes und `spalten / 128`
// Gitter zu zwei Byte (Schritt a als u8, Versatz b als i8). Format wie in
// `vierbit.rs`: Je Gruppe 64 Byte in vier Bloecken zu 16 fuer je 32
// Gewichte; Byte j eines Blocks traegt im unteren Halbbyte den Code q des
// Gewichts j und im oberen den des Gewichts 16 + j, und das Gewicht ist
// a * q + b. Je Gruppe die Summe q * x und die Summe x in int32, zusammen
// in int64 als a * (q * x) + b * x: dieselbe ganze Zahl wie
// `sum_g sum_{i in g} (a_g q_i + b_g) x_i` der CPU, auch fuer Schritte und
// Versaetze, deren Werte nicht in int8 passen.
//
// Wertebereich: q * x je Gruppe hoechstens 128 * 15 * 32 768 < 2^26, die
// Summe x hoechstens 128 * 32 768 = 2^22, beide in int32.
MYL_KERN void myl_vierbit(const myl_u8* codes, const myl_u8* gitter, const myl_i16* x,
                          const myl_i8* abstaende, myl_i16* aus, int zeilen, int spalten, int stapel) {
    __shared__ myl_i8 kq[KACHEL_ZEILEN][GRUPPE + 4];
    __shared__ myl_i16 kx[KACHEL_EINGABEN][GRUPPE + 2];
    const int tx = (int)threadIdx.x;
    const int ty = (int)threadIdx.y;
    const int faden = ty * FAEDEN_X + tx;
    const myl_i64 zeile0 = (myl_i64)blockIdx.x * KACHEL_ZEILEN;
    const myl_i64 eingabe0 = (myl_i64)blockIdx.y * KACHEL_EINGABEN;
    const int gruppen = spalten / GRUPPE;
    const myl_i64 bytes_je_zeile = spalten / 2;
    myl_i64 summe[ZEILEN_JE_FADEN];
    for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
        summe[r] = 0;
    }
    for (int g = 0; g < gruppen; g++) {
        // 64 Zeilen zu 64 Codebytes, je Byte zwei Gewichte.
        for (int i = faden; i < KACHEL_ZEILEN * (GRUPPE / 2); i += FAEDEN) {
            int r = i / (GRUPPE / 2);
            int p = i % (GRUPPE / 2);
            myl_i64 z = zeile0 + r;
            int spalte = (p / 16) * 32 + (p % 16);
            if (z < zeilen) {
                unsigned int B = (unsigned int)codes[z * bytes_je_zeile + (myl_i64)g * (GRUPPE / 2) + p];
                kq[r][spalte] = (myl_i8)(B & 15u);
                kq[r][spalte + 16] = (myl_i8)(B >> 4);
            } else {
                kq[r][spalte] = 0;
                kq[r][spalte + 16] = 0;
            }
        }
        for (int i = faden; i < KACHEL_EINGABEN * GRUPPE; i += FAEDEN) {
            int e = i / GRUPPE;
            int k = i % GRUPPE;
            myl_i64 b = eingabe0 + e;
            kx[e][k] = (b < stapel) ? x[b * spalten + (myl_i64)g * GRUPPE + k] : (myl_i16)0;
        }
        __syncthreads();
        myl_i32 teil[ZEILEN_JE_FADEN];
        for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
            teil[r] = 0;
        }
        myl_i32 xsumme = 0;
        for (int k = 0; k < GRUPPE; k++) {
            myl_i32 xv = kx[tx][k];
            xsumme += xv;
            for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
                teil[r] += (myl_i32)kq[ty + FAEDEN_Y * r][k] * xv;
            }
        }
        for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
            myl_i64 z = zeile0 + ty + FAEDEN_Y * r;
            if (z < zeilen) {
                myl_i64 stelle = (z * gruppen + g) * 2;
                myl_i64 schritt = (myl_i64)gitter[stelle];
                // Das Byte als i8 von Hand: Die Umwandlung eines Werts ueber
                // 127 nach `signed char` ist vor C++20 nicht festgelegt.
                myl_i64 versatz = (myl_i64)gitter[stelle + 1];
                if (versatz > 127) {
                    versatz -= 256;
                }
                summe[r] += (myl_i64)teil[r] * schritt + (myl_i64)xsumme * versatz;
            }
        }
        __syncthreads();
    }
    const myl_i64 b = eingabe0 + tx;
    for (int r = 0; r < ZEILEN_JE_FADEN; r++) {
        myl_i64 z = zeile0 + ty + FAEDEN_Y * r;
        if (z < zeilen && b < stapel) {
            aus[b * zeilen + z] = klemmen(umskalieren(summe[r], (int)abstaende[z]));
        }
    }
}
