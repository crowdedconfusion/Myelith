// W8A16 im Buendel auf der GPU von Apple-Silizium.
//
// Fuer int8-Gewichte zwei Rechenschritte in einem Befehlspuffer (der Weg
// fuer ternaer gepackte Gewichte steht weiter unten):
//
// 1. `produkt`: C = X * W^T ueber `matmul2d` aus Metal Performance
//    Primitives, int8 mal int8 nach int32. X traegt die zerlegten
//    Aktivierungen (siehe `mod.rs`): zuerst die hohen Stellen aller
//    Eingaben, dann die niedrigen, zuletzt eine Zeile aus Einsen fuer
//    die Zeilensummen der Gewichte.
// 2. `nachlauf`: acc = 256 * hoch + niedrig + 128 * Zeilensumme in int64,
//    dann Rechtsshift mit Runden zur naechsten geraden Zahl und Klemmen
//    auf int16, Element fuer Element wie `rescale_i64` und
//    `clamp_i16_from_i64` in `fixed_point.rs`.
//
// Kein Gleitkommatyp, kein Gleitkommaliteral. `relaxed_precision` steht
// auf false.

#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

struct Produktform {
    int32_t zeilen;   // Zeilen der Gewichtsmatrix
    int32_t spalten;  // in_features
    int32_t eingaben; // Zeilen von X
};

// Kachelmass, gemessen am 2026-09-14 (M5 Pro, `down_proj` des 4B, 169
// Eingaben): 16 Eingaben mal 64 Gewichtszeilen je Fadengruppe, vier
// SIMD-Gruppen. Mit `execution_simdgroups` muessen beide Masse Vielfache
// von acht sein.
kernel void produkt(device int8_t  *w [[buffer(0)]],
                    device int8_t  *x [[buffer(1)]],
                    device int32_t *c [[buffer(2)]],
                    constant Produktform &f [[buffer(3)]],
                    uint2 gruppe [[threadgroup_position_in_grid]]) {
    auto X = tensor<device int8_t,  dextents<int32_t, 2>, tensor_inline>(x, dextents<int32_t, 2>(f.spalten, f.eingaben));
    auto W = tensor<device int8_t,  dextents<int32_t, 2>, tensor_inline>(w, dextents<int32_t, 2>(f.spalten, f.zeilen));
    auto C = tensor<device int32_t, dextents<int32_t, 2>, tensor_inline>(c, dextents<int32_t, 2>(f.zeilen, f.eingaben));
    constexpr auto d = matmul2d_descriptor(16, 64, static_cast<int>(dynamic_extent), false, true, false);
    matmul2d<d, execution_simdgroups<4>> op;
    auto teil_x = X.slice(0, int(gruppe.y) * 16);
    auto teil_w = W.slice(0, int(gruppe.x) * 64);
    auto teil_c = C.slice(int(gruppe.x) * 64, int(gruppe.y) * 16);
    op.run(teil_x, teil_w, teil_c);
}

struct Nachlaufform {
    int32_t zeilen;   // Zeilen der Gewichtsmatrix
    int32_t stapel;   // Eingaben
};

// Rechtsshift mit Runden zur naechsten geraden Zahl, wie
// `rshift_round_i64`; ein negativer Abstand schiebt nach links, wie
// `rescale_i64`.
static inline int64_t umskalieren(int64_t wert, int abstand) {
    if (abstand < 0) {
        return wert << (-abstand);
    }
    if (abstand == 0) {
        return wert;
    }
    int64_t maske = (int64_t(1) << abstand) - 1;
    int64_t halb = int64_t(1) << (abstand - 1);
    int64_t quotient = wert >> abstand;
    int64_t rest = wert & maske;
    if (rest > halb || (rest == halb && (quotient & 1) != 0)) {
        quotient += 1;
    }
    return quotient;
}

kernel void nachlauf(device int32_t *c         [[buffer(0)]],
                     device int8_t  *abstaende [[buffer(1)]],
                     device int16_t *aus       [[buffer(2)]],
                     constant Nachlaufform &f  [[buffer(3)]],
                     uint2 ort [[thread_position_in_grid]]) {
    int64_t z = int64_t(ort.x);
    int64_t b = int64_t(ort.y);
    int64_t zeilen = int64_t(f.zeilen);
    int64_t stapel = int64_t(f.stapel);
    if (z >= zeilen || b >= stapel) {
        return;
    }
    int64_t hoch = int64_t(c[b * zeilen + z]);
    int64_t niedrig = int64_t(c[(stapel + b) * zeilen + z]);
    int64_t zeilensumme = int64_t(c[2 * stapel * zeilen + z]);
    int64_t acc = hoch * 256 + niedrig + zeilensumme * 128;
    int64_t y = umskalieren(acc, int(abstaende[z]));
    aus[b * zeilen + z] = int16_t(clamp(y, int64_t(-32768), int64_t(32767)));
}

// ---------------------------------------------------------------------------
// Ternaere Gewichte im Buendel: 2-Bit-Codes und ein Betrag je 128 Eingaenge.
//
// Drei Rechenschritte in einem Befehlspuffer:
//
// 1. `entpacken`: aus jedem Musterbyte vier Werte t = c - 1 (also -1, 0, 1,
//    und 2 fuer den Code 3) als int8, in der Anordnung einer gewoehnlichen
//    Gewichtsmatrix. Das Format steht in `ternaer.rs`: Byte j eines Blocks
//    von 16 traegt in den Bits 2s und 2s+1 den Code des Gewichts 16s + j.
// 2. `gruppenprodukt`: `matmul2d` je 128er-Gruppe (K = 128) und Kachel,
//    int8 mal int8 nach int32, das Teilprodukt in einen Zwischenbereich der
//    Kachel; danach mal dem Betrag der Gruppe in int64 gesammelt. Dieselbe
//    ganze Zahl wie `sum_g m_g * (sum_{i in g} t_i * x_i)` der CPU, nur sind
//    die Eingaben wie oben in zwei Stellen zerlegt.
// 3. `nachlauf_breit`: wie `nachlauf`, aus den int64-Summen.
//
// Wertebereich: Ein Teilprodukt einer Gruppe ist hoechstens
// 128 * 2 * 128 = 32 768 dem Betrag nach und passt in int32. Mal dem Betrag
// (unter 2^15) und ueber alle Gruppen einer Zeile bleibt die Summe weit in
// int64; zusammengesetzt mit 256 ebenso.

struct Entpackform {
    int32_t zeilen;
    int32_t spalten;
};

kernel void entpacken(device uchar  *muster [[buffer(0)]],
                      device int8_t *w      [[buffer(1)]],
                      constant Entpackform &f [[buffer(2)]],
                      uint2 ort [[thread_position_in_grid]]) {
    // ort.x: Musterbyte der Zeile, ort.y: Zeile.
    int bytes_je_zeile = f.spalten / 4;
    if (int(ort.x) >= bytes_je_zeile || int(ort.y) >= f.zeilen) {
        return;
    }
    long z = long(ort.y);
    int p = int(ort.x);
    uint B = uint(muster[z * long(bytes_je_zeile) + p]);
    // 32 Byte je Gruppe, zwei Bloecke zu 16 Byte fuer je 64 Gewichte.
    int spalte = (p / 32) * 128 + ((p % 32) / 16) * 64 + (p % 16);
    device int8_t *ziel = w + z * long(f.spalten) + spalte;
    ziel[0]  = int8_t(int(B & 3u) - 1);
    ziel[16] = int8_t(int((B >> 2) & 3u) - 1);
    ziel[32] = int8_t(int((B >> 4) & 3u) - 1);
    ziel[48] = int8_t(int(B >> 6) - 1);
}

struct Gruppenform {
    int32_t zeilen;    // Zeilen der Gewichtsmatrix
    int32_t spalten;   // in_features, ein Vielfaches von 128
    int32_t eingaben;  // Zeilen von X
    int32_t kacheln_x; // Kacheln je Eingabekachel, also ceil(zeilen / 64)
    int32_t breite;    // Faeden je Fadengruppe
};

kernel void gruppenprodukt(device int8_t  *w        [[buffer(0)]],
                           device int8_t  *x        [[buffer(1)]],
                           device int32_t *zwischen [[buffer(2)]],
                           constant Gruppenform &f  [[buffer(3)]],
                           device int16_t *betraege [[buffer(4)]],
                           device int64_t *summen   [[buffer(5)]],
                           uint2 gruppe [[threadgroup_position_in_grid]],
                           uint faden [[thread_index_in_threadgroup]]) {
    auto X = tensor<device int8_t,  dextents<int32_t, 2>, tensor_inline>(x, dextents<int32_t, 2>(f.spalten, f.eingaben));
    auto W = tensor<device int8_t,  dextents<int32_t, 2>, tensor_inline>(w, dextents<int32_t, 2>(f.spalten, f.zeilen));
    long kachel = long(gruppe.y) * long(f.kacheln_x) + long(gruppe.x);
    device int32_t *teil = zwischen + kachel * 1024;
    auto C = tensor<device int32_t, dextents<int32_t, 2>, tensor_inline>(teil, dextents<int32_t, 2>(64, 16));
    constexpr auto d = matmul2d_descriptor(16, 64, 128, false, true, false);
    matmul2d<d, execution_simdgroups<4>> op;
    int gruppen = f.spalten / 128;
    for (int g = 0; g < gruppen; g++) {
        auto teil_x = X.slice(g * 128, int(gruppe.y) * 16);
        auto teil_w = W.slice(g * 128, int(gruppe.x) * 64);
        op.run(teil_x, teil_w, C);
        threadgroup_barrier(mem_flags::mem_device);
        // Jedes Element der Kachel gehoert genau einem Faden.
        for (int i = int(faden); i < 1024; i += f.breite) {
            long z = long(gruppe.x) * 64 + long(i % 64);
            long e = long(gruppe.y) * 16 + long(i / 64);
            if (z < long(f.zeilen) && e < long(f.eingaben)) {
                int64_t beitrag = int64_t(teil[i]) * int64_t(betraege[z * long(gruppen) + g]);
                long ziel = e * long(f.zeilen) + z;
                // Die erste Gruppe setzt, die folgenden addieren: Der
                // Summenpuffer wird wiederverwendet und nicht geloescht.
                summen[ziel] = (g == 0) ? beitrag : summen[ziel] + beitrag;
            }
        }
        threadgroup_barrier(mem_flags::mem_device);
    }
}

kernel void nachlauf_breit(device int64_t *summen    [[buffer(0)]],
                           device int8_t  *abstaende [[buffer(1)]],
                           device int16_t *aus       [[buffer(2)]],
                           constant Nachlaufform &f  [[buffer(3)]],
                           uint2 ort [[thread_position_in_grid]]) {
    int64_t z = int64_t(ort.x);
    int64_t b = int64_t(ort.y);
    int64_t zeilen = int64_t(f.zeilen);
    int64_t stapel = int64_t(f.stapel);
    if (z >= zeilen || b >= stapel) {
        return;
    }
    int64_t hoch = summen[b * zeilen + z];
    int64_t niedrig = summen[(stapel + b) * zeilen + z];
    int64_t zeilensumme = summen[2 * stapel * zeilen + z];
    int64_t acc = hoch * 256 + niedrig + zeilensumme * 128;
    int64_t y = umskalieren(acc, int(abstaende[z]));
    aus[b * zeilen + z] = int16_t(clamp(y, int64_t(-32768), int64_t(32767)));
}
