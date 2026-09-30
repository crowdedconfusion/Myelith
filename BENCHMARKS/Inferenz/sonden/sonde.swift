import Metal
import Foundation

let args = CommandLine.arguments
let zeilen = args.count > 1 ? Int(args[1])! : 17408
let spalten = args.count > 2 ? Int(args[2])! : 5120
let eingaben = args.count > 3 ? Int(args[3])! : 64
let kern = args.count > 4 ? args[4] : "ternaer"
let gx = args.count > 5 ? Int(args[5])! : 32
let gy = args.count > 6 ? Int(args[6])! : 8
let gruppen = spalten / 128

let geraet = MTLCreateSystemDefaultDevice()!
let quelle = try! String(contentsOfFile: "sonde.metal", encoding: .utf8)
let opt = MTLCompileOptions(); opt.languageVersion = .version4_0
let bib = try! geraet.makeLibrary(source: quelle, options: opt)
let schlange = geraet.makeCommandQueue()!
var saat: UInt64 = 0x9e3779b97f4a7c15
func zufall() -> UInt64 { saat ^= saat << 13; saat ^= saat >> 7; saat ^= saat << 17; return saat }
struct Form { var zeilen: Int32; var spalten: Int32; var eingaben: Int32 }

func messen(_ name: String, _ lauf: () -> MTLCommandBuffer, produkte: Double) {
    var beste = Double.infinity
    for _ in 0..<4 { let t0 = Date(); let p = lauf(); p.commit(); p.waitUntilCompleted(); if let e = p.error { print("FEHLER", e); exit(1) }
        beste = min(beste, Date().timeIntervalSince(t0)) }
    print(String(format: "[sonde] %@ %d x %d, %d Eingaben: %.2f ms, %.0f G Gewichte*Eingaben/s", name, zeilen, spalten, eingaben, beste * 1e3, produkte / beste / 1e9))
}

if kern == "ternaer" {
    let pipe = try! geraet.makeComputePipelineState(function: bib.makeFunction(name: "ternaer")!)
    let muster = geraet.makeBuffer(length: zeilen * gruppen * 32, options: .storageModeShared)!
    let betr = geraet.makeBuffer(length: zeilen * gruppen * 2, options: .storageModeShared)!
    let x = geraet.makeBuffer(length: eingaben * spalten * 2, options: .storageModeShared)!
    let aus = geraet.makeBuffer(length: eingaben * zeilen * 8, options: .storageModeShared)!
    let mp = muster.contents().bindMemory(to: UInt8.self, capacity: zeilen * gruppen * 32)
    for i in 0..<(zeilen * gruppen * 32) { var b: UInt8 = 0; let r = zufall(); for s in 0..<4 { b |= UInt8((r >> (8 * UInt64(s))) % 3) << UInt8(2 * s) }; mp[i] = b }
    let bp = betr.contents().bindMemory(to: Int16.self, capacity: zeilen * gruppen)
    for i in 0..<(zeilen * gruppen) { bp[i] = Int16(1 + zufall() % 16000) }
    let xp = x.contents().bindMemory(to: Int16.self, capacity: eingaben * spalten)
    for i in 0..<(eingaben * spalten) { xp[i] = Int16(truncatingIfNeeded: zufall()) }
    var form = Form(zeilen: Int32(zeilen), spalten: Int32(spalten), eingaben: Int32(eingaben))
    messen("eigener Kern (\(gx)x\(gy))", {
        let p = schlange.makeCommandBuffer()!; let k = p.makeComputeCommandEncoder()!
        k.setComputePipelineState(pipe)
        k.setBuffer(muster, offset: 0, index: 0); k.setBuffer(betr, offset: 0, index: 1); k.setBuffer(x, offset: 0, index: 2); k.setBuffer(aus, offset: 0, index: 3)
        k.setBytes(&form, length: MemoryLayout<Form>.size, index: 4)
        k.dispatchThreads(MTLSize(width: zeilen, height: eingaben, depth: 1), threadsPerThreadgroup: MTLSize(width: gx, height: gy, depth: 1))
        k.endEncoding(); return p }, produkte: Double(zeilen) * Double(spalten) * Double(eingaben))
    // Variante mit vier Eingaben je Faden.
    let viertel = (eingaben + 3) / 4
    let x4 = geraet.makeBuffer(length: viertel * spalten * 8, options: .storageModeShared)!
    let su = geraet.makeBuffer(length: viertel * gruppen * 16, options: .storageModeShared)!
    let x4p = x4.contents().bindMemory(to: Int16.self, capacity: viertel * spalten * 4)
    let sup = su.contents().bindMemory(to: Int32.self, capacity: viertel * gruppen * 4)
    for i in 0..<(viertel * spalten * 4) { x4p[i] = 0 }
    for i in 0..<(viertel * gruppen * 4) { sup[i] = 0 }
    for b in 0..<eingaben { for i in 0..<spalten { let w = xp[b * spalten + i]; x4p[((b / 4) * spalten + i) * 4 + b % 4] = w; sup[((b / 4) * gruppen + i / 128) * 4 + b % 4] += Int32(w) } }
    let pipe4 = try! geraet.makeComputePipelineState(function: bib.makeFunction(name: "ternaer4")!)
    if kern == "ternaer" && args.count > 7 {
        let ap0 = aus.contents().bindMemory(to: Int64.self, capacity: eingaben * zeilen)
        let soll = Array(UnsafeBufferPointer(start: ap0, count: eingaben * zeilen))
        messen("vier Eingaben je Faden (\(gx)x\(gy))", {
            let p = schlange.makeCommandBuffer()!; let k = p.makeComputeCommandEncoder()!
            k.setComputePipelineState(pipe4)
            k.setBuffer(muster, offset: 0, index: 0); k.setBuffer(betr, offset: 0, index: 1); k.setBuffer(x4, offset: 0, index: 2); k.setBuffer(aus, offset: 0, index: 3)
            k.setBytes(&form, length: MemoryLayout<Form>.size, index: 4); k.setBuffer(su, offset: 0, index: 5)
            k.dispatchThreads(MTLSize(width: zeilen, height: viertel, depth: 1), threadsPerThreadgroup: MTLSize(width: gx, height: gy, depth: 1))
            k.endEncoding(); return p }, produkte: Double(zeilen) * Double(spalten) * Double(eingaben))
        var ab = 0; for i in 0..<(eingaben * zeilen) { if ap0[i] != soll[i] { ab += 1 } }
        print(ab == 0 ? "[sonde] alle \(eingaben * zeilen) Summen gleich dem einfachen Kern" : "[sonde] ABWEICHUNG in \(ab) Summen")
    }
    // Gegenprobe an Stichproben gegen die skalare Rechnung.
    let ap = aus.contents().bindMemory(to: Int64.self, capacity: eingaben * zeilen)
    var fehler = 0
    for (z, b) in [(0, 0), (zeilen - 1, eingaben - 1), (zeilen / 2, eingaben / 3), (7, 5)] {
        var acc: Int64 = 0
        for g in 0..<gruppen { var s: Int64 = 0
            for i in 0..<128 { let blk = i / 64, rest = i % 64; let byte = blk * 16 + rest % 16, vers = 2 * (rest / 16)
                let c = Int64((mp[(z * gruppen + g) * 32 + byte] >> UInt8(vers)) & 3) - 1
                s += c * Int64(xp[b * spalten + g * 128 + i]) }
            acc += s * Int64(bp[z * gruppen + g]) }
        if acc != ap[b * zeilen + z] { fehler += 1 } }
    print(fehler == 0 ? "[sonde] Stichproben stimmen mit der skalaren Rechnung" : "[sonde] ABWEICHUNG in \(fehler) Stichproben")
} else {
    let pipe = try! geraet.makeComputePipelineState(function: bib.makeFunction(name: "produkt")!)
    let w = geraet.makeBuffer(length: zeilen * spalten, options: .storageModeShared)!
    let ein = 2 * eingaben + 1
    let x = geraet.makeBuffer(length: ein * spalten, options: .storageModeShared)!
    let c = geraet.makeBuffer(length: ein * zeilen * 4, options: .storageModeShared)!
    let wp = w.contents().bindMemory(to: UInt64.self, capacity: zeilen * spalten / 8); for i in 0..<(zeilen * spalten / 8) { wp[i] = zufall() }
    let xp = x.contents().bindMemory(to: UInt8.self, capacity: ein * spalten); for i in 0..<(ein * spalten) { xp[i] = UInt8(truncatingIfNeeded: zufall()) }
    var form = Form(zeilen: Int32(zeilen), spalten: Int32(spalten), eingaben: Int32(ein))
    let breite = pipe.threadExecutionWidth * 4
    if args.count > 5 {
        struct GForm { var zeilen: Int32; var spalten: Int32; var eingaben: Int32; var kx: Int32; var breite: Int32 }
        let pg = try! geraet.makeComputePipelineState(function: bib.makeFunction(name: "gruppenweise")!)
        let kx = (zeilen + 63) / 64, ky = (ein + 15) / 16
        let zw = geraet.makeBuffer(length: kx * ky * 1024 * 4, options: .storageModeShared)!
        let acc = geraet.makeBuffer(length: ein * zeilen * 8, options: .storageModeShared)!
        let bt = geraet.makeBuffer(length: zeilen * gruppen * 2, options: .storageModeShared)!
        let btp = bt.contents().bindMemory(to: Int16.self, capacity: zeilen * gruppen)
        for i in 0..<(zeilen * gruppen) { btp[i] = Int16(1 + zufall() % 16000) }
        // Gewichte auf -1, 0, 1 bringen, wie entpackte Codes.
        let w8 = w.contents().bindMemory(to: Int8.self, capacity: zeilen * spalten)
        for i in 0..<(zeilen * spalten) { w8[i] = Int8(Int(UInt8(bitPattern: w8[i]) % 3) - 1) }
        var gf = GForm(zeilen: Int32(zeilen), spalten: Int32(spalten), eingaben: Int32(ein), kx: Int32(kx), breite: Int32(breite))
        let accp = acc.contents().bindMemory(to: Int64.self, capacity: ein * zeilen)
        messen("matmul2d je Gruppe", {
            memset(acc.contents(), 0, ein * zeilen * 8)
            let p = schlange.makeCommandBuffer()!; let k = p.makeComputeCommandEncoder()!
            k.setComputePipelineState(pg)
            k.setBuffer(w, offset: 0, index: 0); k.setBuffer(x, offset: 0, index: 1); k.setBuffer(zw, offset: 0, index: 2)
            k.setBytes(&gf, length: MemoryLayout<GForm>.size, index: 3); k.setBuffer(bt, offset: 0, index: 4); k.setBuffer(acc, offset: 0, index: 5)
            k.dispatchThreadgroups(MTLSize(width: kx, height: ky, depth: 1), threadsPerThreadgroup: MTLSize(width: breite, height: 1, depth: 1))
            k.endEncoding(); return p }, produkte: Double(zeilen) * Double(spalten) * Double(eingaben))
        let x8 = x.contents().bindMemory(to: Int8.self, capacity: ein * spalten)
        var fehler = 0
        for (z, e) in [(0, 0), (zeilen - 1, ein - 1), (zeilen / 2, ein / 3), (77, 5), (64, 16), (63, 15)] {
            var soll: Int64 = 0
            for g in 0..<gruppen { var s: Int64 = 0; for i in 0..<128 { s += Int64(w8[z * spalten + g * 128 + i]) * Int64(x8[e * spalten + g * 128 + i]) }; soll += s * Int64(btp[z * gruppen + g]) }
            if soll != accp[e * zeilen + z] { fehler += 1; print("  soll", soll, "ist", accp[e * zeilen + z], "bei", z, e) } }
        print(fehler == 0 ? "[sonde] Stichproben stimmen" : "[sonde] ABWEICHUNG in \(fehler) Stichproben")
    }
    messen("matmul2d int8", {
        let p = schlange.makeCommandBuffer()!; let k = p.makeComputeCommandEncoder()!
        k.setComputePipelineState(pipe)
        k.setBuffer(w, offset: 0, index: 0); k.setBuffer(x, offset: 0, index: 1); k.setBuffer(c, offset: 0, index: 2)
        k.setBytes(&form, length: MemoryLayout<Form>.size, index: 3)
        k.dispatchThreadgroups(MTLSize(width: (zeilen + 63) / 64, height: (ein + 15) / 16, depth: 1), threadsPerThreadgroup: MTLSize(width: breite, height: 1, depth: 1))
        k.endEncoding(); return p }, produkte: Double(zeilen) * Double(spalten) * Double(eingaben))
}
