// Huelle, mit der `kerne.cu` als gewoehnliches C++ auf der CPU laeuft.
//
// Wozu: Die Kernel lassen sich so ohne Grafikkarte gegen die CPU-Kerne
// pruefen, Bit fuer Bit, mit derselben Quelle, die NVRTC und hiprtc
// uebersetzen. Geprueft wird damit alles, was im Kernel steht: Indizes,
// Kachelraender, Entpacken, Runden, Klemmen, das Zusammenspiel ueber den
// gemeinsamen Speicher.
//
// Wie: Jede Fadengruppe laeuft fuer sich, ihre Faeden als echte Threads.
// `__shared__` wird `static`: Eine statische lokale Variable teilen alle
// Threads, und weil die Gruppen nacheinander laufen, sieht jede Gruppe
// genau ihren gemeinsamen Speicher. `__syncthreads` ist eine Schranke ueber
// alle Faeden der Gruppe.
//
// Nicht geprueft: was der Uebersetzer der Karte anders macht. Dafuer gibt
// es die Selbstpruefung auf der Karte selbst.

#include <barrier>
#include <thread>
#include <vector>

struct MylDrei {
    unsigned int x, y, z;
};

static thread_local MylDrei threadIdx;
static MylDrei blockIdx;
static MylDrei blockDim;
static MylDrei gridDim;
static std::barrier<>* myl_schranke = nullptr;

#define __global__
#define __device__
#define __shared__ static
#define __syncthreads() myl_schranke->arrive_and_wait()
#define MYL_KERN
#define MYL_GERAET static inline

template <class F>
static void myl_starten(MylDrei gitter, MylDrei gruppe, F kern) {
    gridDim = gitter;
    blockDim = gruppe;
    const unsigned int faeden = gruppe.x * gruppe.y * gruppe.z;
    for (unsigned int gz = 0; gz < gitter.z; gz++) {
        for (unsigned int gy = 0; gy < gitter.y; gy++) {
            for (unsigned int gx = 0; gx < gitter.x; gx++) {
                blockIdx = MylDrei{gx, gy, gz};
                std::barrier<> schranke((std::ptrdiff_t)faeden);
                myl_schranke = &schranke;
                std::vector<std::thread> alle;
                alle.reserve(faeden);
                for (unsigned int tz = 0; tz < gruppe.z; tz++) {
                    for (unsigned int ty = 0; ty < gruppe.y; ty++) {
                        for (unsigned int tx = 0; tx < gruppe.x; tx++) {
                            alle.emplace_back([=] {
                                threadIdx = MylDrei{tx, ty, tz};
                                kern();
                            });
                        }
                    }
                }
                for (auto& t : alle) {
                    t.join();
                }
            }
        }
    }
}
