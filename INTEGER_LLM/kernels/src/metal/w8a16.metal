// W8A16 im Buendel auf der GPU von Apple-Silizium.
//
// Zwei Rechenschritte in einem Befehlspuffer:
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
